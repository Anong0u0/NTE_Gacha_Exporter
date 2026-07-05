#[cfg(not(windows))]
pub fn run_diagnostic_capture(
    _options: DiagnosticCaptureOptions,
    _stop: Arc<AtomicBool>,
) -> Result<DiagnosticCaptureResult> {
    anyhow::bail!("pktmon diagnostic capture requires Windows")
}

#[cfg(windows)]
pub fn run_diagnostic_capture(
    options: DiagnosticCaptureOptions,
    stop: Arc<AtomicBool>,
) -> Result<DiagnosticCaptureResult> {
    use pktmon::filter::{PktMonFilter, TransportProtocol};

    let started_at = Instant::now();
    let duration = if options.duration.is_zero() {
        Duration::from_secs(30)
    } else {
        options.duration
    };
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
    let mut raw_writer = match options.raw_out.as_ref() {
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
    let mut dropped_writer = match options.dropped_samples_out.as_ref() {
        Some(path) => Some(DroppedSampleWriter::open(path, options.pid, &ports)?),
        None => None,
    };
    let mut assembler = ProtocolAssembler::default();
    let mut warnings = Vec::new();
    let mut counters = DiagnosticCaptureCounters::default();
    let mut summary = DiagnosticCaptureSummary::default();
    let mut last_packet: Option<DiagnosticRecentPacket> = None;
    let mut last_progress_seen = 0_u64;
    let mut attempts = Vec::new();
    emit_progress(
        &options,
        &target,
        &counters,
        started_at,
        assembler.rows().len() as u64,
        warnings.len() as u64,
    );

    while !stop.load(Ordering::SeqCst) && started_at.elapsed() < duration {
        target.capture_strategy = strategy.kind.as_str().to_string();
        target.strategy_reason = strategy.reason.as_str().to_string();
        target.bpf = bpf(strategy.kind, &ports);
        let attempt_started_at = now_seconds();
        let attempt_start_counters = counters.clone();
        let mut capture = pktmon::Capture::new()?;
        if strategy.kind == CaptureStrategyKind::PortFiltered {
            for port in &ports {
                capture.add_filter(PktMonFilter {
                    name: format!("NTE diagnostic UDP {port}"),
                    transport_protocol: Some(TransportProtocol::UDP),
                    port: (*port).into(),
                    ..Default::default()
                })?;
                capture.add_filter(PktMonFilter {
                    name: format!("NTE diagnostic TCP {port}"),
                    transport_protocol: Some(TransportProtocol::TCP),
                    port: (*port).into(),
                    ..Default::default()
                })?;
            }
        }
        capture.start()?;

        let mut restart_for_ports = false;
        let mut idle_ticks = 0_u32;
        loop {
            if stop.load(Ordering::SeqCst) || started_at.elapsed() >= duration {
                stop.store(true, Ordering::SeqCst);
                break;
            }

            match capture.next_packet_timeout(Duration::from_secs(1)) {
                Ok(packet) => {
                    idle_ticks = 0;
                    counters.packets_seen += 1;
                    let kind = packet_kind(&packet.payload);
                    increment(&mut summary.packet_kind_counts, packet_kind_name(kind));
                    let bytes = packet.payload.to_vec();
                    let Some(parsed_packet) = parse_packet_bytes(bytes, kind) else {
                        counters.dropped_packets += 1;
                        increment(
                            &mut summary.dropped_packet_size_buckets,
                            size_bucket(bytes.len()),
                        );
                        let analysis = analyze_dropped_packet(bytes, kind);
                        add_dropped_evidence_summary(
                            &mut summary,
                            &analysis,
                            counters.packets_seen,
                            bytes.len(),
                        );
                        if counters.dropped_samples_written < options.max_dropped_samples as u64 {
                            if let Some(writer) = dropped_writer.as_mut() {
                                let include_full_payload = should_include_full_dropped_sample(
                                    &counters,
                                    options.max_full_dropped_samples,
                                );
                                writer.write_sample(&DroppedPacketSample {
                                    typ: "dropped_packet_sample",
                                    schema_version: 1,
                                    captured_at: now_seconds(),
                                    capture_index: counters.packets_seen,
                                    packet_kind: packet_kind_name(kind).to_string(),
                                    size: bytes.len(),
                                    analysis,
                                    payload_prefix_b64: payload_prefix_b64(bytes),
                                    payload_truncated: bytes.len() > DROPPED_SAMPLE_PREFIX_BYTES,
                                    payload_full_included: include_full_payload,
                                    payload_b64: include_full_payload.then(|| payload_b64(bytes)),
                                })?;
                                counters.dropped_samples_written += 1;
                                if include_full_payload {
                                    counters.dropped_full_samples_written += 1;
                                }
                            }
                        }
                        continue;
                    };

                    let captured_at = now_seconds();
                    if last_packet
                        .as_ref()
                        .is_some_and(|last| last.matches(&parsed_packet, captured_at))
                    {
                        counters.duplicate_packets += 1;
                        maybe_emit_progress(
                            &options,
                            &target,
                            &counters,
                            started_at,
                            &mut assembler,
                            &warnings,
                            &mut last_progress_seen,
                        );
                        continue;
                    }
                    last_packet = Some(DiagnosticRecentPacket {
                        signature: DiagnosticRawSignature::from_packet(&parsed_packet),
                        captured_at,
                    });
                    add_parsed_summary(&mut summary, &parsed_packet);
                    let (blocks, found_warnings) = parse_payload_blocks(
                        &parsed_packet.payload,
                        0,
                        counters.packets_seen,
                        counters.packets_seen - 1,
                    );
                    add_block_summary(&mut summary, &blocks);
                    add_warning_summary(&mut summary, &found_warnings);
                    warnings.extend(found_warnings);
                    if should_write_raw_packet(
                        &parsed_packet,
                        &ports,
                        !blocks.is_empty(),
                        strategy.kind,
                    ) {
                        let record = raw_record_from_parsed_packet(
                            &parsed_packet,
                            counters.packets_seen,
                            captured_at,
                        );
                        if let Some(writer) = raw_writer.as_mut() {
                            writer.write_packet(&record)?;
                            counters.raw_packets_written += 1;
                        }
                    }
                    if blocks.is_empty() {
                        maybe_emit_progress(
                            &options,
                            &target,
                            &counters,
                            started_at,
                            &mut assembler,
                            &warnings,
                            &mut last_progress_seen,
                        );
                        continue;
                    }
                    counters.decoded_packets += 1;
                    let _ = assembler.add_blocks_with_update(blocks);
                    maybe_emit_progress(
                        &options,
                        &target,
                        &counters,
                        started_at,
                        &mut assembler,
                        &warnings,
                        &mut last_progress_seen,
                    );
                }
                Err(error) if is_timeout(&error) => {
                    idle_ticks += 1;
                    emit_progress(
                        &options,
                        &target,
                        &counters,
                        started_at,
                        assembler.rows().len() as u64,
                        warnings.len() as u64,
                    );
                    if idle_ticks >= 3 {
                        idle_ticks = 0;
                        let latest = net::limited_filter_ports(&net::candidate_ports(options.pid)?);
                        if latest.iter().any(|port| !ports.contains(port)) {
                            ports = latest;
                            target.ports = ports.clone();
                            target.bpf = bpf(strategy.kind, &ports);
                            if strategy.kind == CaptureStrategyKind::PortFiltered {
                                counters.filter_restarts += 1;
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
        let attempt = CaptureAttemptSummary {
            attempt_index: attempts.len() as u32,
            capture_strategy: target.capture_strategy.clone(),
            strategy_reason: target.strategy_reason.clone(),
            started_at: attempt_started_at,
            ended_at: now_seconds(),
            counters: diagnostic_counters_delta(&attempt_start_counters, &counters),
        };
        attempts.push(attempt);
        target.attempts = attempts.clone();
        if !restart_for_ports {
            break;
        }
    }

    if let Some(writer) = raw_writer.as_mut() {
        writer.write_stop(
            counters.packets_seen,
            counters.decoded_packets,
            counters.dropped_packets,
            counters.duplicate_packets,
        )?;
    }
    if let Some(writer) = dropped_writer.as_mut() {
        writer.write_stop(&counters)?;
    }
    let assembler_warnings = std::mem::take(&mut assembler.warnings);
    add_warning_summary(&mut summary, &assembler_warnings);
    warnings.extend(assembler_warnings);
    summary.rows_count = assembler.rows().len() as u64;
    summary.warning_count = warnings.len() as u64;
    emit_progress(
        &options,
        &target,
        &counters,
        started_at,
        summary.rows_count,
        summary.warning_count,
    );
    Ok(DiagnosticCaptureResult {
        target,
        counters,
        summary,
        warnings,
        elapsed_seconds: started_at.elapsed().as_secs_f64(),
    })
}

#[cfg(windows)]
fn diagnostic_counters_delta(
    start: &DiagnosticCaptureCounters,
    end: &DiagnosticCaptureCounters,
) -> CaptureCounters {
    CaptureCounters {
        packets_seen: end.packets_seen.saturating_sub(start.packets_seen),
        decoded_packets: end.decoded_packets.saturating_sub(start.decoded_packets),
        dropped_packets: end.dropped_packets.saturating_sub(start.dropped_packets),
        duplicate_packets: end.duplicate_packets.saturating_sub(start.duplicate_packets),
        filter_restarts: end.filter_restarts.saturating_sub(start.filter_restarts),
    }
}
