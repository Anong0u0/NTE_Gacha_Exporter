use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::protocol::{ParseWarning, ParsedRow};

pub struct CaptureOptions {
    pub pid: u32,
    pub exe: String,
    pub ports: Vec<u16>,
    pub pppoe_detection: Option<crate::net::PppoeDetection>,
    pub backend: CaptureBackend,
    pub strategy: Option<CaptureStrategy>,
    pub raw_out: Option<std::path::PathBuf>,
    pub raw_append: bool,
    pub windivert_dir: Option<std::path::PathBuf>,
    pub max_packets: u64,
    pub max_decoded: u64,
    pub on_progress: Option<CaptureProgressCallback>,
}

pub type CaptureProgressCallback = Arc<dyn Fn(CaptureProgress) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CaptureBackend {
    #[default]
    Pktmon,
    WinDivert,
}

impl CaptureBackend {
    const WIRE_VARIANTS: &'static [&'static str] = &["pktmon", "windivert"];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pktmon => "pktmon",
            Self::WinDivert => "windivert",
        }
    }

    fn from_wire(value: &str) -> Option<Self> {
        match value {
            "pktmon" => Some(Self::Pktmon),
            "windivert" => Some(Self::WinDivert),
            _ => None,
        }
    }
}

impl Serialize for CaptureBackend {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for CaptureBackend {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_wire(&value)
            .ok_or_else(|| serde::de::Error::unknown_variant(&value, Self::WIRE_VARIANTS))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CaptureTarget {
    pub pid: u32,
    pub exe: String,
    pub interface: String,
    pub ports: Vec<u16>,
    pub bpf: String,
    pub capture_strategy: String,
    pub strategy_reason: String,
    pub pppoe_detection: crate::net::PppoeDetection,
    pub attempts: Vec<CaptureAttemptSummary>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStrategyKind {
    PortFiltered,
    NoFilter,
}

impl CaptureStrategyKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PortFiltered => "port_filtered",
            Self::NoFilter => "no_filter",
        }
    }
}

impl Serialize for CaptureStrategyKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStrategyReason {
    Default,
    PppoeFastPath,
    DiagnosticNoFilter,
    WinDivertBackend,
}

impl CaptureStrategyReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::PppoeFastPath => "pppoe_fast_path",
            Self::DiagnosticNoFilter => "diagnostic_no_filter",
            Self::WinDivertBackend => "windivert_backend",
        }
    }
}

impl Serialize for CaptureStrategyReason {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CaptureStrategy {
    pub kind: CaptureStrategyKind,
    pub reason: CaptureStrategyReason,
}

impl CaptureStrategy {
    pub fn port_filtered() -> Self {
        Self {
            kind: CaptureStrategyKind::PortFiltered,
            reason: CaptureStrategyReason::Default,
        }
    }

    pub fn no_filter(reason: CaptureStrategyReason) -> Self {
        Self {
            kind: CaptureStrategyKind::NoFilter,
            reason,
        }
    }

    pub fn for_pppoe_detection(detection: &crate::net::PppoeDetection) -> Self {
        if detection.detected {
            Self::no_filter(CaptureStrategyReason::PppoeFastPath)
        } else {
            Self::port_filtered()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptureCounters {
    pub packets_seen: u64,
    pub decoded_packets: u64,
    pub dropped_packets: u64,
    pub duplicate_packets: u64,
    pub filter_restarts: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CaptureAttemptSummary {
    pub attempt_index: u32,
    pub capture_strategy: String,
    pub strategy_reason: String,
    pub started_at: f64,
    pub ended_at: f64,
    pub counters: CaptureCounters,
}

#[derive(Debug)]
pub struct CaptureResult {
    pub target: CaptureTarget,
    pub counters: CaptureCounters,
    pub attempts: Vec<CaptureAttemptSummary>,
    pub rows: Vec<ParsedRow>,
    pub warnings: Vec<ParseWarning>,
}

#[derive(Debug, Clone)]
pub struct CaptureProgress {
    pub target: CaptureTarget,
    pub counters: CaptureCounters,
    pub new_rows: Vec<ParsedRow>,
    pub rows_snapshot: Vec<ParsedRow>,
    pub row_count: usize,
    pub warning_count: usize,
}
