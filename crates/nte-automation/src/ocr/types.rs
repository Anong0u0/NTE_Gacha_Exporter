#[derive(Debug, Clone)]
struct PageCandidate {
    page: PageNumber,
    score: f32,
    pass: String,
}

#[derive(Debug, Clone)]
struct PassScore {
    best: PageCandidate,
    second: Option<PageCandidate>,
    ranked: Vec<PageCandidate>,
}

#[derive(Debug, Clone)]
struct CandidateAggregate {
    page: PageNumber,
    pass: String,
    score_sum: f32,
    best_score: f32,
    seen_count: usize,
    win_count: usize,
}

impl CandidateAggregate {
    fn into_page_candidate(self, pass_count: usize) -> PageCandidate {
        let pass_count = pass_count.max(1) as f32;
        let coverage = self.seen_count as f32 / pass_count;
        let consensus = self.win_count as f32 / pass_count;
        let average_score = self.score_sum / self.seen_count.max(1) as f32;
        PageCandidate {
            page: self.page,
            score: average_score * 0.82 + coverage * 0.10 + consensus * 0.08,
            pass: format!(
                "aggregate:{}:wins:{}:{}:best:{:.3}",
                self.seen_count, self.win_count, self.pass, self.best_score
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct TargetMask {
    threshold: u8,
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

impl TargetMask {
    fn char_count_hint(&self) -> usize {
        self.estimated_char_count
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
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
