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
    payload_prefix_b64: String,
    payload_truncated: bool,
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
