use std::collections::{HashMap, HashSet};

use image::RgbaImage;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{
    OcrCandidateDiagnostics, OcrFieldDiagnostics, OcrReadDiagnostics, OcrSegmentationDiagnostics,
    PageNumber, Rect,
};

const MAX_TOTAL_PAGE: u32 = 9999;
const MAX_PAGE_TEXT_LEN: usize = 9;
const MAX_FIELD_DIGITS: usize = 4;
const MIN_DECODE_SCORE: f32 = 0.58;
const MIN_HIGH_CONFIDENCE_SCORE: f32 = 0.76;
const MIN_UNHINTED_MARGIN: f32 = 0.025;
const MIN_GLYPH_SCORE: f32 = 0.28;
const MAX_RANKED_CANDIDATES: usize = 8;
const THRESHOLDS: [u8; 7] = [125, 135, 145, 155, 165, 175, 185];
const SEQUENCE_X_OFFSETS: [i32; 3] = [-1, 0, 1];
const FINGERPRINT_WIDTH: u32 = 64;
const FINGERPRINT_HEIGHT: u32 = 32;

include!("ocr/reader.rs");
include!("ocr/types.rs");
include!("ocr/diagnostics.rs");
include!("ocr/target.rs");
include!("ocr/scoring.rs");
include!("ocr/templates.rs");
include!("ocr/tests.rs");
