use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use anyhow::Result;
use pktmon::filter::{PktMonFilter, TransportProtocol};

use super::engine::{CaptureLoopState, counters_delta, now_seconds};
use crate::live::{
    CaptureAttemptSummary, CaptureOptions, CaptureResult, CaptureStrategy, CaptureStrategyKind,
    CaptureTarget,
};
use crate::net;
use crate::raw::{PacketKind, RawPacketSource, RawWriter, parse_packet_bytes};

pub(super) fn capture_live_pktmon(
    options: CaptureOptions,
    stop: Arc<AtomicBool>,
) -> Result<CaptureResult> {
    let mut ports = net::limited_filter_ports(&options.ports);
    let pppoe_detection = options
        .pppoe_detection
        .clone()
        .unwrap_or_else(net::detect_pppoe);
    let strategy = options
        .strategy
        .unwrap_or_else(|| CaptureStrategy::for_pppoe_detection(&pppoe_detection));
    if ports.is_empty() && strategy.kind == CaptureStrategyKind::PortFiltered {
        anyhow::bail!("no candidate ports found for pid={}", options.pid);
    }

    let mut target = CaptureTarget {
        pid: options.pid,
        exe: options.exe.clone(),
        interface: "pktmon".to_string(),
        ports: ports.clone(),
        bpf: bpf(strategy.kind, &ports),
        capture_strategy: strategy.kind.as_str().to_string(),
        strategy_reason: strategy.reason.as_str().to_string(),
        pppoe_detection: pppoe_detection.clone(),
        attempts: Vec::new(),
    };
    let raw_writer = match options.raw_out.as_ref() {
        Some(path) => Some(RawWriter::open(
            path,
            options.pid,
            &ports,
            strategy,
            &pppoe_detection,
            options.raw_append,
        )?),
        None => None,
    };
    let mut loop_state = CaptureLoopState::new(raw_writer);
    let mut attempts = Vec::new();
    loop_state.emit_initial_progress(&options, &target);

    while !stop.load(Ordering::SeqCst) {
        target.capture_strategy = strategy.kind.as_str().to_string();
        target.strategy_reason = strategy.reason.as_str().to_string();
        target.bpf = bpf(strategy.kind, &ports);
        let attempt_started_at = now_seconds();
        let attempt_start_counters = loop_state.counters().clone();
        let mut capture = pktmon::Capture::new()?;
        if strategy.kind == CaptureStrategyKind::PortFiltered {
            add_port_filters(&mut capture, &ports)?;
        }
        capture.start()?;

        let mut restart_for_ports = false;
        let mut idle_ticks = 0_u32;
        loop {
            if stop.load(Ordering::SeqCst) {
                break;
            }
            if max_reached(
                loop_state.counters(),
                options.max_packets,
                options.max_decoded,
            ) {
                stop.store(true, Ordering::SeqCst);
                break;
            }

            match capture.next_packet_timeout(Duration::from_secs(1)) {
                Ok(packet) => {
                    idle_ticks = 0;
                    loop_state.counters_mut().packets_seen += 1;
                    let kind = packet_kind(&packet.payload);
                    let bytes = packet.payload.to_vec();
                    let Some(parsed_packet) =
                        parse_packet_bytes(bytes, kind, RawPacketSource::Pktmon)
                    else {
                        loop_state.record_parse_drop();
                        continue;
                    };
                    loop_state.handle_packet(
                        &options,
                        &target,
                        parsed_packet,
                        &ports,
                        strategy.kind,
                        now_seconds(),
                    )?;
                }
                Err(error) if is_timeout(&error) => {
                    idle_ticks += 1;
                    if idle_ticks >= 3 {
                        idle_ticks = 0;
                        let latest = net::limited_filter_ports(&net::candidate_ports(options.pid)?);
                        if latest.iter().any(|port| !ports.contains(port)) {
                            ports = latest;
                            target.ports = ports.clone();
                            target.bpf = bpf(strategy.kind, &ports);
                            if strategy.kind == CaptureStrategyKind::PortFiltered {
                                loop_state.counters_mut().filter_restarts += 1;
                                restart_for_ports = true;
                                break;
                            }
                        }
                    }
                }
                Err(error) => return Err(error.into()),
            }
        }
        let _ = capture.stop();
        let _ = capture.unload();
        attempts.push(CaptureAttemptSummary {
            attempt_index: attempts.len() as u32,
            capture_strategy: target.capture_strategy.clone(),
            strategy_reason: target.strategy_reason.clone(),
            started_at: attempt_started_at,
            ended_at: now_seconds(),
            counters: counters_delta(&attempt_start_counters, loop_state.counters()),
        });
        target.attempts = attempts.clone();
        if !restart_for_ports {
            break;
        }
    }

    let finish = loop_state.finish(&options, &target)?;
    Ok(CaptureResult {
        target,
        counters: finish.counters,
        attempts,
        rows: finish.rows,
        warnings: finish.warnings,
    })
}

fn add_port_filters(capture: &mut pktmon::Capture, ports: &[u16]) -> Result<()> {
    for port in ports {
        capture.add_filter(PktMonFilter {
            name: format!("NTE UDP {port}"),
            transport_protocol: Some(TransportProtocol::UDP),
            port: (*port).into(),
            ..Default::default()
        })?;
        capture.add_filter(PktMonFilter {
            name: format!("NTE TCP {port}"),
            transport_protocol: Some(TransportProtocol::TCP),
            port: (*port).into(),
            ..Default::default()
        })?;
    }
    Ok(())
}

pub(crate) fn bpf(strategy: CaptureStrategyKind, ports: &[u16]) -> String {
    match strategy {
        CaptureStrategyKind::PortFiltered => ports
            .iter()
            .map(|port| format!("port {port}"))
            .collect::<Vec<_>>()
            .join(" or "),
        CaptureStrategyKind::NoFilter => "none".to_string(),
    }
}

fn packet_kind(payload: &pktmon::PacketPayload) -> PacketKind {
    match payload {
        pktmon::PacketPayload::Ethernet(_) | pktmon::PacketPayload::WiFi(_) => PacketKind::Ethernet,
        pktmon::PacketPayload::IP(_) => PacketKind::Ip,
        pktmon::PacketPayload::TCP(_) => PacketKind::Tcp,
        pktmon::PacketPayload::UDP(_) => PacketKind::Udp,
        pktmon::PacketPayload::L4Payload(_) => PacketKind::L4Payload,
        _ => PacketKind::Unknown,
    }
}

fn is_timeout(error: &impl std::fmt::Display) -> bool {
    format!("{error}")
        .to_ascii_lowercase()
        .contains("timed out")
}

fn max_reached(
    counters: &crate::live::CaptureCounters,
    max_packets: u64,
    max_decoded: u64,
) -> bool {
    (max_packets > 0 && counters.packets_seen >= max_packets)
        || (max_decoded > 0 && counters.decoded_packets >= max_decoded)
}
