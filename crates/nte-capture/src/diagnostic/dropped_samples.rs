#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedPacketSample {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    captured_at: f64,
    capture_index: u64,
    packet_kind: String,
    size: usize,
    analysis: DiagnosticDroppedPacketAnalysis,
    payload_prefix_b64: String,
    payload_truncated: bool,
    payload_full_included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload_b64: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedStartRecord {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    pid: u32,
    ports: Vec<u16>,
}

#[cfg(windows)]
#[derive(Debug, Clone, Serialize)]
struct DroppedStopRecord<'a> {
    #[serde(rename = "type")]
    typ: &'static str,
    schema_version: u32,
    counters: &'a DiagnosticCaptureCounters,
}

#[cfg(windows)]
struct DroppedSampleWriter {
    writer: BufWriter<File>,
}

#[cfg(windows)]
impl DroppedSampleWriter {
    fn open(path: &std::path::Path, pid: u32, ports: &[u16]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = File::create(path)?;
        let mut writer = Self {
            writer: BufWriter::new(file),
        };
        writer.write_json(&DroppedStartRecord {
            typ: "dropped_capture_start",
            schema_version: 1,
            pid,
            ports: ports.to_vec(),
        })?;
        Ok(writer)
    }

    fn write_sample(&mut self, sample: &DroppedPacketSample) -> Result<()> {
        self.write_json(sample)
    }

    fn write_stop(&mut self, counters: &DiagnosticCaptureCounters) -> Result<()> {
        self.write_json(&DroppedStopRecord {
            typ: "dropped_capture_stop",
            schema_version: 1,
            counters,
        })?;
        self.writer.flush()?;
        Ok(())
    }

    fn write_json(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}

#[cfg(any(windows, test))]
fn should_include_full_dropped_sample(
    counters: &DiagnosticCaptureCounters,
    max_full_dropped_samples: usize,
) -> bool {
    counters.dropped_full_samples_written < max_full_dropped_samples as u64
}

#[cfg(test)]
mod dropped_sample_tests {
    use super::*;
    use base64::Engine;

    #[test]
    fn dropped_payload_prefix_is_capped_at_512_bytes() {
        let bytes = vec![7_u8; 600];

        let encoded = payload_prefix_b64(&bytes);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();

        assert_eq!(decoded.len(), 512);
    }

    #[test]
    fn dropped_full_payload_is_limited_by_counter() {
        let mut counters = DiagnosticCaptureCounters {
            dropped_full_samples_written: 31,
            ..Default::default()
        };
        assert!(should_include_full_dropped_sample(&counters, 32));

        counters.dropped_full_samples_written = 32;
        assert!(!should_include_full_dropped_sample(&counters, 32));
    }

    #[test]
    fn dropped_full_payload_encoder_keeps_complete_bytes() {
        let bytes = vec![3_u8; 600];

        let encoded = payload_b64(&bytes);
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .unwrap();

        assert_eq!(decoded, bytes);
    }
}
