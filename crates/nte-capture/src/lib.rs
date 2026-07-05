mod diagnostic;
mod document;
mod live;
mod net;
mod protocol;
mod raw;
pub mod windivert;

pub use diagnostic::{
    DiagnosticCaptureCounters, DiagnosticCaptureOptions, DiagnosticCaptureProgress,
    DiagnosticCaptureResult, DiagnosticCaptureSummary, DiagnosticDroppedEvidenceExample,
    DiagnosticDroppedEvidenceSummary, DiagnosticDroppedOffsets, DiagnosticDroppedPacketAnalysis,
    DiagnosticIpEvidence, DiagnosticMarkerHits, DiagnosticPppoeEvidence,
    DiagnosticTransportEvidence, DiagnosticVlanTagEvidence, run_diagnostic_capture,
};
pub use document::{CapturePublicRecord, CaptureRecordBuilder, build_capture_document};
pub use live::{
    CaptureAttemptSummary, CaptureBackend, CaptureCounters, CaptureOptions, CaptureProgress,
    CaptureResult, CaptureStrategy, CaptureStrategyKind, CaptureStrategyReason, CaptureTarget,
    capture_live,
};
pub use net::{
    CaptureDoctorReport, PppoeDetection, PppoeDetectionSource, candidate_ports, capture_doctor,
    detect_pppoe, find_process_pid, find_process_pids, is_admin,
};
pub use protocol::{ParseWarning, ParsedRow, RecordType, SourceRef};
pub use raw::read_raw_capture;
