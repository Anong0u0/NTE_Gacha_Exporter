#[cfg(windows)]
const DIAGNOSTIC_DUPLICATE_WINDOW_SECONDS: f64 = 0.250;
#[cfg(windows)]
const DROPPED_SAMPLE_PREFIX_BYTES: usize = 512;

#[cfg(windows)]
fn maybe_emit_progress(
    options: &DiagnosticCaptureOptions,
    target: &CaptureTarget,
    counters: &DiagnosticCaptureCounters,
    started_at: Instant,
    assembler: &mut ProtocolAssembler,
    warnings: &[ParseWarning],
    last_progress_seen: &mut u64,
) {
    if counters.packets_seen.saturating_sub(*last_progress_seen) < 250 {
        return;
    }
    *last_progress_seen = counters.packets_seen;
    emit_progress(
        options,
        target,
        counters,
        started_at,
        assembler.rows().len() as u64,
        warnings.len() as u64,
    );
}

#[cfg(windows)]
fn emit_progress(
    options: &DiagnosticCaptureOptions,
    target: &CaptureTarget,
    counters: &DiagnosticCaptureCounters,
    started_at: Instant,
    rows_count: u64,
    warning_count: u64,
) {
    let Some(callback) = options.on_progress.as_ref() else {
        return;
    };
    callback(DiagnosticCaptureProgress {
        target: target.clone(),
        counters: counters.clone(),
        elapsed_seconds: started_at.elapsed().as_secs_f64(),
        rows_count,
        warning_count,
    });
}

#[cfg(windows)]
fn add_parsed_summary(summary: &mut DiagnosticCaptureSummary, packet: &ParsedNetworkPacket) {
    increment(&mut summary.parser_counts, &packet.parser);
    increment(&mut summary.proto_counts, &packet.proto);
    increment(
        &mut summary.parsed_payload_size_buckets,
        size_bucket(packet.payload.len()),
    );
    if packet.payload.len() <= 64 {
        summary.small_parsed_payload_packets += 1;
    }
    increment(&mut summary.port_pair_counts, port_pair(packet));
}

#[cfg(windows)]
fn add_block_summary(
    summary: &mut DiagnosticCaptureSummary,
    blocks: &[crate::protocol::ParsedBlock],
) {
    for block in blocks {
        match block.record_type {
            RecordType::Monopoly => {
                summary.marker_hits.monopoly_blocks += 1;
                summary.marker_hits.monopoly_rows += block.rows.len() as u64;
            }
            RecordType::Fork => {
                summary.marker_hits.fork_blocks += 1;
                summary.marker_hits.fork_rows += block.rows.len() as u64;
            }
        }
    }
}

#[cfg(windows)]
fn add_warning_summary(summary: &mut DiagnosticCaptureSummary, warnings: &[ParseWarning]) {
    for warning in warnings {
        increment(&mut summary.warning_code_counts, &warning.code);
        if warning.message.contains("FMonopolyLotteryRecordData") {
            summary.marker_hits.monopoly_parse_warnings += 1;
        }
        if warning.message.contains("FForkLotteryRecordData") {
            summary.marker_hits.fork_parse_warnings += 1;
        }
    }
}

#[cfg(windows)]
fn port_pair(packet: &ParsedNetworkPacket) -> String {
    match (packet.sport, packet.dport) {
        (Some(sport), Some(dport)) => format!("{sport}->{dport}"),
        (Some(sport), None) => format!("{sport}->?"),
        (None, Some(dport)) => format!("?->{dport}"),
        (None, None) => "unknown".to_string(),
    }
}

#[cfg(windows)]
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

#[cfg(windows)]
fn packet_kind_name(kind: PacketKind) -> &'static str {
    match kind {
        PacketKind::Unknown => "unknown",
        PacketKind::Ethernet => "ethernet",
        PacketKind::Ip => "ip",
        PacketKind::Tcp => "tcp",
        PacketKind::Udp => "udp",
        PacketKind::L4Payload => "l4_payload",
    }
}

#[cfg(windows)]
fn increment(map: &mut BTreeMap<String, u64>, key: impl AsRef<str>) {
    *map.entry(key.as_ref().to_string()).or_default() += 1;
}

#[cfg(windows)]
fn size_bucket(size: usize) -> &'static str {
    match size {
        0..=31 => "0-31",
        32..=63 => "32-63",
        64..=127 => "64-127",
        128..=511 => "128-511",
        512..=1499 => "512-1499",
        _ => "1500+",
    }
}

#[cfg(windows)]
fn diagnostic_bpf(ports: &[u16]) -> String {
    ports
        .iter()
        .map(|port| format!("port {port}"))
        .collect::<Vec<_>>()
        .join(" or ")
}

#[cfg(windows)]
fn payload_prefix_b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD
        .encode(bytes.get(..DROPPED_SAMPLE_PREFIX_BYTES).unwrap_or(bytes))
}

#[cfg(windows)]
fn is_timeout(error: &impl std::fmt::Display) -> bool {
    format!("{error}")
        .to_ascii_lowercase()
        .contains("timed out")
}

#[cfg(windows)]
fn now_seconds() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs_f64())
        .unwrap_or_default()
}
