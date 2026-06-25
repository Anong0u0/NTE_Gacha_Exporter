#[cfg(windows)]
pub struct RawWriter {
    writer: BufWriter<File>,
}

#[cfg(windows)]
impl RawWriter {
    pub fn open(path: &Path, pid: u32, ports: &[u16]) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let file = File::create(path).with_context(|| format!("create {}", path.display()))?;
        let mut writer = Self {
            writer: BufWriter::new(file),
        };
        writer.write_json(&CaptureStartRecord {
            typ: "capture_start",
            schema_version: 1,
            pid,
            iface: "pktmon",
            ports: ports.to_vec(),
            bpf: ports
                .iter()
                .map(|port| format!("port {port}"))
                .collect::<Vec<_>>()
                .join(" or "),
        })?;
        Ok(writer)
    }

    pub fn write_packet(&mut self, record: &RawPacketRecord) -> Result<()> {
        self.write_json(record)
    }

    pub fn write_stop(
        &mut self,
        seen: u64,
        decoded_packets: u64,
        dropped: u64,
        duplicate_packets: u64,
    ) -> Result<()> {
        self.write_json(&CaptureStopRecord {
            typ: "capture_stop",
            schema_version: 1,
            seen,
            decoded_packets,
            dropped,
            duplicate_packets,
        })?;
        self.writer.flush().context("flush raw writer")
    }

    fn write_json(&mut self, value: &impl Serialize) -> Result<()> {
        serde_json::to_writer(&mut self.writer, value)?;
        self.writer.write_all(b"\n")?;
        Ok(())
    }
}
