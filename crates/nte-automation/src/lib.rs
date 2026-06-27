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
    AutoPageDiagnostics, AutoPageOptions, AutoPageResult, AutoPageStatus, PageNumber, Point,
    RecordSnapshot, Rect, Size, TemplateMatch,
};
pub use pager::run_auto_page;
pub use profile::{AutomationProfile, load_profile};
