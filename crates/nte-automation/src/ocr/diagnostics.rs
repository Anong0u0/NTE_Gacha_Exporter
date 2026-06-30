fn sequence_attempt(
    candidate_index: usize,
    size: Size,
    target: &TargetMask,
    best: &PageCandidate,
    second: Option<&PageCandidate>,
    error: Option<String>,
) -> OcrAttemptDiagnostic {
    OcrAttemptDiagnostic {
        candidate_index,
        size,
        method: Some("lattice".to_string()),
        pass: Some(best.pass.clone()),
        text: Some(best.page.text.clone()),
        score: Some(best.score),
        second_text: second.map(|candidate| candidate.page.text.clone()),
        second_score: second.map(|candidate| candidate.score),
        margin: second.map(|candidate| best.score - candidate.score),
        glyph_count: Some(target.char_count_hint()),
        error,
    }
}

fn sequence_attempt_error(
    candidate_index: usize,
    size: Size,
    pass: String,
    glyph_count: Option<usize>,
    error: impl Into<String>,
) -> OcrAttemptDiagnostic {
    OcrAttemptDiagnostic {
        candidate_index,
        size,
        method: Some("lattice".to_string()),
        pass: Some(pass),
        text: None,
        score: None,
        second_text: None,
        second_score: None,
        margin: None,
        glyph_count,
        error: Some(error.into()),
    }
}

fn page_read_diagnostics(
    hint: PageReadHint,
    attempts: Vec<OcrAttemptDiagnostic>,
    error: impl Into<String>,
) -> OcrReadDiagnostics {
    OcrReadDiagnostics {
        hint: hint.into(),
        attempts,
        error: error.into(),
    }
}

fn merge_best_candidate(
    best_by_text: &mut HashMap<String, PageCandidate>,
    candidate: PageCandidate,
) {
    best_by_text
        .entry(candidate.page.text.clone())
        .and_modify(|current| {
            if candidate.score > current.score {
                *current = candidate.clone();
            }
        })
        .or_insert(candidate);
}

fn merge_candidate_score(
    candidate_scores: &mut HashMap<String, CandidateAggregate>,
    candidate: PageCandidate,
    won_pass: bool,
) {
    candidate_scores
        .entry(candidate.page.text.clone())
        .and_modify(|aggregate| {
            aggregate.score_sum += candidate.score;
            aggregate.seen_count += 1;
            if won_pass {
                aggregate.win_count += 1;
            }
            if candidate.score > aggregate.best_score {
                aggregate.best_score = candidate.score;
                aggregate.pass = candidate.pass.clone();
            }
        })
        .or_insert(CandidateAggregate {
            page: candidate.page,
            pass: candidate.pass,
            score_sum: candidate.score,
            best_score: candidate.score,
            seen_count: 1,
            win_count: usize::from(won_pass),
        });
}

fn digit_count(value: u32) -> u32 {
    value.ilog10() + 1
}

fn digit_floor(digits: u32) -> u32 {
    if digits <= 1 {
        1
    } else {
        10_u32.pow(digits - 1)
    }
}

fn digit_ceil(digits: u32) -> u32 {
    10_u32.pow(digits) - 1
}
