use std::collections::BTreeMap;
use std::sync::{Arc, atomic::AtomicBool};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn right(self) -> i32 {
        self.x + self.width as i32
    }

    pub fn bottom(self) -> i32 {
        self.y + self.height as i32
    }

    pub fn expand(self, padding: Point) -> Self {
        Self {
            x: self.x - padding.x,
            y: self.y - padding.y,
            width: self.width.saturating_add((padding.x.max(0) as u32) * 2),
            height: self.height.saturating_add((padding.y.max(0) as u32) * 2),
        }
    }

    pub fn clamp(self, size: Size) -> Self {
        let left = self.x.clamp(0, size.width as i32);
        let top = self.y.clamp(0, size.height as i32);
        let right = self.right().clamp(left + 1, size.width as i32);
        let bottom = self.bottom().clamp(top + 1, size.height as i32);
        Self {
            x: left,
            y: top,
            width: (right - left).max(1) as u32,
            height: (bottom - top).max(1) as u32,
        }
    }

    pub fn size(self) -> Size {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    pub fn center(self) -> Point {
        Point {
            x: self.x + self.width as i32 / 2,
            y: self.y + self.height as i32 / 2,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageNumber {
    pub current: u32,
    pub total: u32,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMatch {
    pub name: String,
    pub matched: bool,
    pub edge_score: f32,
    pub gray_score: f32,
    pub point: Point,
    pub size: Size,
    pub scale: f32,
    pub searched_rect: Rect,
    pub candidate_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPageStatus {
    pub elapsed_seconds: f64,
    pub message: String,
    pub kind: String,
    pub step: Option<String>,
    pub pool: Option<String>,
    pub current_page: Option<u32>,
    pub total_pages: Option<u32>,
    pub technical_detail: String,
    pub replaceable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordSnapshot {
    pub record_id: String,
    pub pool_id: String,
    pub record_type: String,
}

pub type StatusCallback = Arc<dyn Fn(AutoPageStatus) + Send + Sync + 'static>;
pub type RecordSnapshotCallback = Arc<dyn Fn() -> Vec<RecordSnapshot> + Send + Sync + 'static>;
pub type DecodedPageCountCallback = Arc<dyn Fn(&str) -> usize + Send + Sync + 'static>;

pub struct AutoPageOptions {
    pub pid: u32,
    pub stop: Arc<AtomicBool>,
    pub full_update: bool,
    pub non_interactive: bool,
    pub tooltip: bool,
    pub known_record_ids: Vec<String>,
    pub record_snapshot: Option<RecordSnapshotCallback>,
    pub decoded_page_count: Option<DecodedPageCountCallback>,
    pub on_status: Option<StatusCallback>,
    pub click_timeout: f64,
    pub click_poll_interval: f64,
    pub duplicate_check_timeout: f64,
    pub max_capture_page_lag: usize,
    pub template_timeout: f64,
}

impl AutoPageOptions {
    pub fn new(pid: u32, stop: Arc<AtomicBool>) -> Self {
        Self {
            pid,
            stop,
            full_update: false,
            non_interactive: false,
            tooltip: true,
            known_record_ids: Vec::new(),
            record_snapshot: None,
            decoded_page_count: None,
            on_status: None,
            click_timeout: 5.0,
            click_poll_interval: 0.2,
            duplicate_check_timeout: 1.5,
            max_capture_page_lag: 8,
            template_timeout: 5.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoPageResult {
    pub status: String,
    pub message: String,
    pub completed_pools: Vec<String>,
    pub skipped_pools: Vec<String>,
    pub visited_pages_by_pool: BTreeMap<String, u32>,
    pub last_page_by_pool: BTreeMap<String, u32>,
}

impl AutoPageResult {
    pub fn completed(completed_pools: Vec<String>, skipped_pools: Vec<String>) -> Self {
        Self::completed_with_pages(
            completed_pools,
            skipped_pools,
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    pub fn completed_with_pages(
        completed_pools: Vec<String>,
        skipped_pools: Vec<String>,
        visited_pages_by_pool: BTreeMap<String, u32>,
        last_page_by_pool: BTreeMap<String, u32>,
    ) -> Self {
        Self {
            status: "completed".to_string(),
            message: "auto page completed".to_string(),
            completed_pools,
            skipped_pools,
            visited_pages_by_pool,
            last_page_by_pool,
        }
    }

    pub fn failed(
        message: impl Into<String>,
        completed_pools: Vec<String>,
        skipped_pools: Vec<String>,
    ) -> Self {
        Self {
            status: "failed".to_string(),
            message: message.into(),
            completed_pools,
            skipped_pools,
            visited_pages_by_pool: BTreeMap::new(),
            last_page_by_pool: BTreeMap::new(),
        }
    }

    pub fn manual(
        message: impl Into<String>,
        completed_pools: Vec<String>,
        skipped_pools: Vec<String>,
    ) -> Self {
        Self {
            status: "manual".to_string(),
            message: message.into(),
            completed_pools,
            skipped_pools,
            visited_pages_by_pool: BTreeMap::new(),
            last_page_by_pool: BTreeMap::new(),
        }
    }

    pub fn succeeded(&self) -> bool {
        self.status == "completed"
    }

    pub fn manual_required(&self) -> bool {
        self.status == "manual"
    }
}
