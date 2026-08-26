pub mod error;
pub mod home;
pub mod matcher;
pub mod model;
pub mod ocr;
pub mod pager;
pub mod profile;
pub mod screenshot;
pub mod tooltip;
pub mod window;

pub use error::{AutomationError, AutomationResult};
pub use home::restore_game_home;
pub use model::{
    AUTO_PAGE_INCREMENTAL_DUPLICATE_RECORD_THRESHOLD, AutoPageControlContext,
    AutoPageControlDecision, AutoPageDiagnostics, AutoPageInputDiagnostics, AutoPageOptions,
    AutoPageOutcome, AutoPageRecoveryDiagnostics, AutoPageResult, AutoPageStatus,
    AutoPageStopReason, MouseButton, MouseClickDiagnostics, OcrCandidateDiagnostics,
    OcrFieldDiagnostics, OcrReadDiagnostics, OcrSegmentationDiagnostics, PageNumber,
    PageTransitionDiagnostics, Point, RecordSnapshot, Rect, Size, TemplateMatch,
    TemplateTransitionDiagnostics,
};
pub use ocr::{
    NumberCandidate, NumberObservation, ObservationState, PageFingerprint, PageNumberReader,
    PageObservation, PageReadConstraint,
};
pub use pager::run_auto_page;
pub use profile::{AutomationProfile, load_profile};
