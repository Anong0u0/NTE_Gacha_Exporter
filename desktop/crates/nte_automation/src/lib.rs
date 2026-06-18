pub mod error;
pub mod matcher;
pub mod model;
pub mod ocr;
pub mod pager;
pub mod profile;
pub mod screenshot;
pub mod tooltip;
pub mod window;

pub use error::{AutomationError, AutomationResult};
pub use model::{
    AutoPageOptions, AutoPageResult, AutoPageStatus, PageNumber, Point, RecordSnapshot, Rect, Size,
    TemplateMatch,
};
pub use pager::run_auto_page;
pub use profile::{load_profile, AutomationProfile};
