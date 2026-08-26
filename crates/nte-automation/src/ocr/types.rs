#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PageReadConstraint {
    pub known_total: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationState {
    Confident,
    Ambiguous,
    Missing,
}

impl ObservationState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Confident => "confident",
            Self::Ambiguous => "ambiguous",
            Self::Missing => "missing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NumberCandidate {
    pub value: u32,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NumberObservation {
    pub state: ObservationState,
    pub candidates: Vec<NumberCandidate>,
}

impl NumberObservation {
    pub fn confident_value(&self) -> Option<u32> {
        (self.state == ObservationState::Confident)
            .then(|| self.candidates.first().map(|candidate| candidate.value))
            .flatten()
    }

    fn margin(&self) -> f32 {
        let Some(best) = self.candidates.first() else {
            return 0.0;
        };
        self.candidates
            .get(1)
            .map_or(best.score, |second| best.score - second.score)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageFingerprint {
    pub current_mask: Vec<u8>,
    pub suffix_mask: Vec<u8>,
}

impl PageFingerprint {
    pub fn current_distance(&self, other: &Self) -> f32 {
        mask_distance(&self.current_mask, &other.current_mask)
    }

    pub fn suffix_distance(&self, other: &Self) -> f32 {
        mask_distance(&self.suffix_mask, &other.suffix_mask)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageObservation {
    pub slash: ObservationState,
    pub current: NumberObservation,
    pub total: NumberObservation,
    pub fingerprint: Option<PageFingerprint>,
}

#[derive(Debug, Clone, Copy)]
struct FieldCandidate {
    value: u32,
    score: f32,
}

#[derive(Debug, Clone, Copy)]
struct SlashCandidate {
    target_index: usize,
    slot: GlyphSlot,
    score: f32,
}

#[derive(Debug, Clone)]
struct TargetMask {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    text_height: u32,
    component_count: usize,
    estimated_char_count: usize,
    components: Vec<TextComponent>,
    weights: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct TextComponent {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

impl TextComponent {
    fn right(self) -> u32 {
        self.left + self.width - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphSlot {
    left: u32,
    top: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct GlyphCandidate {
    ch: char,
    score: f32,
}

type GlyphScores = Vec<GlyphCandidate>;

trait GlyphClassifier {
    fn classify_glyph(&self, target: &TargetMask, slot: GlyphSlot) -> GlyphScores;
    fn score_glyph(&self, target: &TargetMask, slot: GlyphSlot, ch: char) -> f32;
}

#[derive(Debug, Clone)]
struct ScaledTemplate {
    width: u32,
    height: u32,
    points: Vec<(u32, u32)>,
}

#[derive(Debug, Clone)]
struct GlyphTemplate {
    ch: char,
    width: u32,
    height: u32,
    data: Vec<bool>,
}

impl GlyphTemplate {
    fn get(&self, x: u32, y: u32) -> bool {
        self.data[(y * self.width + x) as usize]
    }
}

fn mask_distance(left: &[u8], right: &[u8]) -> f32 {
    if left.len() != right.len() || left.is_empty() {
        return 1.0;
    }
    if left.len() != (FINGERPRINT_WIDTH * FINGERPRINT_HEIGHT) as usize {
        return correlation_distance(left, right, 0);
    }
    (-2..=2)
        .map(|x_shift| correlation_distance(left, right, x_shift))
        .fold(f32::INFINITY, f32::min)
}

fn correlation_distance(left: &[u8], right: &[u8], x_shift: i32) -> f32 {
    let count = left.len() as f64;
    let left_mean = left.iter().map(|value| *value as f64).sum::<f64>() / count;
    let right_mean = (0..right.len())
        .map(|index| shifted_mask_value(right, index, x_shift) as f64)
        .sum::<f64>()
        / count;
    let (mut covariance, mut left_energy, mut right_energy) = (0.0, 0.0, 0.0);
    for (index, left) in left.iter().enumerate() {
        let left = *left as f64 - left_mean;
        let right = shifted_mask_value(right, index, x_shift) as f64 - right_mean;
        covariance += left * right;
        left_energy += left * left;
        right_energy += right * right;
    }
    if left_energy == 0.0 || right_energy == 0.0 {
        return if left == right { 0.0 } else { 1.0 };
    }
    let correlation = covariance / (left_energy * right_energy).sqrt();
    ((1.0 - correlation.clamp(-1.0, 1.0)) * 0.5) as f32
}

fn shifted_mask_value(mask: &[u8], index: usize, x_shift: i32) -> u8 {
    let width = FINGERPRINT_WIDTH as usize;
    let x = index % width;
    let shifted_x = x as i32 + x_shift;
    if !(0..FINGERPRINT_WIDTH as i32).contains(&shifted_x) {
        return 0;
    }
    mask[index - x + shifted_x as usize]
}
