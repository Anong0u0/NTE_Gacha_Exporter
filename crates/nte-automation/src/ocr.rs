use std::collections::{HashMap, HashSet};

use image::RgbaImage;

use crate::error::{AutomationError, AutomationResult};
use crate::model::{
    OcrAttemptDiagnostic, OcrReadDiagnostics, PageNumber, PageReadHintDiagnostics, Size,
};

const MAX_TOTAL_PAGE: u32 = 9999;
const MAX_PAGE_TEXT_LEN: usize = 9;
const MAX_UNHINTED_PAGE_TEXT_LEN: usize = 7;
const MIN_DECODE_SCORE: f32 = 0.58;
const MIN_UNHINTED_MARGIN: f32 = 0.025;
const MIN_HINTED_UNEXPECTED_MARGIN: f32 = 0.04;
const MIN_HINTED_UNEXPECTED_SCORE: f32 = 0.70;
const EXPECTED_MIN_SCORE: f32 = 0.54;
const EXPECTED_TIE_BREAK_MARGIN: f32 = 0.08;
const MIN_GLYPH_SCORE: f32 = 0.28;
const AGGREGATE_TOP_CANDIDATES: usize = 8;
const THRESHOLDS: [u8; 7] = [125, 135, 145, 155, 165, 175, 185];
const SEQUENCE_X_OFFSETS: [i32; 3] = [-1, 0, 1];

include!("ocr/reader.rs");
include!("ocr/types.rs");
include!("ocr/diagnostics.rs");
include!("ocr/target.rs");
include!("ocr/scoring.rs");
include!("ocr/templates.rs");
include!("ocr/tests.rs");
