use std::sync::{Arc, atomic::AtomicBool};

use anyhow::Result;

mod types;
pub use types::*;

#[cfg(windows)]
mod engine;
#[cfg(windows)]
mod pktmon;
#[cfg(windows)]
mod progress;
#[cfg(any(windows, test))]
mod raw_filter;
#[cfg(windows)]
mod windivert_backend;

#[cfg(windows)]
pub(crate) use pktmon::bpf;
#[cfg(windows)]
pub(crate) use raw_filter::should_write_raw_packet;

#[cfg(not(windows))]
pub fn capture_live(options: CaptureOptions, _stop: Arc<AtomicBool>) -> Result<CaptureResult> {
    if options.backend == CaptureBackend::WinDivert {
        anyhow::bail!(crate::windivert::windivert_unavailable_for_platform());
    }
    anyhow::bail!("capture requires Windows")
}

#[cfg(windows)]
pub fn capture_live(options: CaptureOptions, stop: Arc<AtomicBool>) -> Result<CaptureResult> {
    match options.backend {
        CaptureBackend::Pktmon => pktmon::capture_live_pktmon(options, stop),
        CaptureBackend::WinDivert => windivert_backend::capture_live_windivert(options, stop),
    }
}
