use std::sync::atomic::Ordering;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use anyhow::Result;

use super::engine::{CaptureLoopState, counters_delta, now_seconds};
use crate::live::{
    CaptureAttemptSummary, CaptureBackend, CaptureOptions, CaptureResult, CaptureStrategy,
    CaptureStrategyKind, CaptureStrategyReason, CaptureTarget,
};
use crate::net;
use crate::raw::{PacketKind, RawCaptureTarget, RawPacketSource, RawWriter, parse_packet_bytes};

pub(super) fn capture_live_windivert(
    options: CaptureOptions,
    stop: Arc<AtomicBool>,
) -> Result<CaptureResult> {
    let ports = net::limited_filter_ports(&options.ports);
    let pppoe_detection = options
        .pppoe_detection
        .clone()
        .unwrap_or_else(net::detect_pppoe);
    let strategy = CaptureStrategy::no_filter(CaptureStrategyReason::WinDivertBackend);
    let mut target = CaptureTarget {
        pid: options.pid,
        exe: options.exe.clone(),
        interface: CaptureBackend::WinDivert.as_str().to_string(),
        ports: ports.clone(),
        bpf: "ip".to_string(),
        capture_strategy: strategy.kind.as_str().to_string(),
        strategy_reason: strategy.reason.as_str().to_string(),
        pppoe_detection: pppoe_detection.clone(),
        attempts: Vec::new(),
    };
    let raw_writer = match options.raw_out.as_ref() {
        Some(path) => Some(RawWriter::open_with_target(
            path,
            options.pid,
            &ports,
            strategy,
            &pppoe_detection,
            options.raw_append,
            RawCaptureTarget::new(CaptureBackend::WinDivert.as_str(), "ip"),
        )?),
        None => None,
    };
    let mut loop_state = CaptureLoopState::new(raw_writer);
    let attempt_started_at = now_seconds();
    let attempt_start_counters = loop_state.counters().clone();
    loop_state.emit_initial_progress(&options, &target);

    let capture =
        crate::windivert::WinDivertHandle::open_ip_sniff(options.windivert_dir.as_deref())
            .map_err(|message| anyhow::anyhow!(message))?;
    let watcher = spawn_recv_shutdown_watcher(capture.clone(), Arc::clone(&stop));
    let mut buffer = vec![0_u8; 65_575];
    let mut fatal_error = None;

    while !stop.load(Ordering::SeqCst) {
        if max_reached(
            loop_state.counters(),
            options.max_packets,
            options.max_decoded,
        ) {
            stop.store(true, Ordering::SeqCst);
            break;
        }

        let packet_len = match capture.recv(&mut buffer) {
            Ok(packet_len) => packet_len,
            Err(crate::windivert::WinDivertRecvError::Shutdown) => break,
            Err(error) => {
                fatal_error = Some(crate::windivert::recv_error_message(error));
                break;
            }
        };
        if packet_len == 0 {
            continue;
        }
        loop_state.counters_mut().packets_seen += 1;
        let Some(parsed_packet) = parse_packet_bytes(
            &buffer[..packet_len],
            PacketKind::Ip,
            RawPacketSource::WinDivert,
        ) else {
            loop_state.record_parse_drop();
            continue;
        };
        loop_state.handle_packet(
            &options,
            &target,
            parsed_packet,
            &ports,
            CaptureStrategyKind::NoFilter,
            now_seconds(),
        )?;
    }

    stop.store(true, Ordering::SeqCst);
    capture.shutdown_recv();
    capture.close();
    let _ = watcher.join();
    let attempts = vec![CaptureAttemptSummary {
        attempt_index: 0,
        capture_strategy: target.capture_strategy.clone(),
        strategy_reason: target.strategy_reason.clone(),
        started_at: attempt_started_at,
        ended_at: now_seconds(),
        counters: counters_delta(&attempt_start_counters, loop_state.counters()),
    }];
    target.attempts = attempts.clone();
    if let Some(error) = fatal_error {
        anyhow::bail!(error);
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

fn spawn_recv_shutdown_watcher(
    capture: crate::windivert::WinDivertHandle,
    stop: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        while !stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(100));
        }
        capture.shutdown_recv();
    })
}

fn max_reached(
    counters: &crate::live::CaptureCounters,
    max_packets: u64,
    max_decoded: u64,
) -> bool {
    (max_packets > 0 && counters.packets_seen >= max_packets)
        || (max_decoded > 0 && counters.decoded_packets >= max_decoded)
}
