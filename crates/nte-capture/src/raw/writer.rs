#[cfg(windows)]
pub struct RawWriter {
    writer: BufWriter<File>,
}

#[cfg(windows)]
pub(crate) struct RawCaptureTarget {
    iface: String,
    bpf: String,
}

#[cfg(windows)]
impl RawCaptureTarget {
    pub(crate) fn new(iface: impl Into<String>, bpf: impl Into<String>) -> Self {
        Self {
            iface: iface.into(),
            bpf: bpf.into(),
        }
    }
}

#[cfg(windows)]
impl RawWriter {
    pub fn open(
        path: &Path,
        pid: u32,
        ports: &[u16],
        strategy: crate::live::CaptureStrategy,
        pppoe_detection: &crate::net::PppoeDetection,
        append: bool,
    ) -> Result<Self> {
        Self::open_with_target(
            path,
            pid,
            ports,
            strategy,
            pppoe_detection,
            append,
            RawCaptureTarget::new("pktmon", crate::live::bpf(strategy.kind, ports)),
        )
    }

    pub fn open_with_target(
        path: &Path,
        pid: u32,
        ports: &[u16],
        strategy: crate::live::CaptureStrategy,
        pppoe_detection: &crate::net::PppoeDetection,
        append: bool,
        target: RawCaptureTarget,
    ) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        let file = if append {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .with_context(|| format!("open {}", path.display()))?
        } else {
            File::create(path).with_context(|| format!("create {}", path.display()))?
        };
        let mut writer = Self {
            writer: BufWriter::new(file),
        };
        writer.write_start_with_target(pid, ports, strategy, pppoe_detection, &target)?;
        Ok(writer)
    }

    pub fn write_start_with_target(
        &mut self,
        pid: u32,
        ports: &[u16],
        strategy: crate::live::CaptureStrategy,
        pppoe_detection: &crate::net::PppoeDetection,
        target: &RawCaptureTarget,
    ) -> Result<()> {
        self.write_json(&CaptureStartRecord {
            typ: "capture_start",
            schema_version: 2,
            pid,
            iface: target.iface.clone(),
            ports: ports.to_vec(),
            bpf: target.bpf.clone(),
            capture_strategy: strategy.kind.as_str().to_string(),
            strategy_reason: strategy.reason.as_str().to_string(),
            pppoe_detection: pppoe_detection.clone(),
        })
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
            schema_version: 2,
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
