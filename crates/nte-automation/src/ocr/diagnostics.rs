fn page_read_diagnostics(
    constraint: PageReadConstraint,
    slash: Option<(f32, Rect)>,
    segmentation: Option<OcrSegmentationDiagnostics>,
    current: &NumberObservation,
    total: &NumberObservation,
    error: impl Into<String>,
) -> OcrReadDiagnostics {
    OcrReadDiagnostics {
        known_total: constraint.known_total,
        slash_score: slash.map(|value| value.0),
        slash_rect: slash.map(|value| value.1),
        segmentation,
        current: field_diagnostics(current),
        total: field_diagnostics(total),
        error: error.into(),
    }
}

fn field_diagnostics(observation: &NumberObservation) -> OcrFieldDiagnostics {
    OcrFieldDiagnostics {
        state: observation.state.as_str().to_string(),
        margin: observation.margin(),
        candidates: observation
            .candidates
            .iter()
            .map(|candidate| OcrCandidateDiagnostics {
                value: candidate.value,
                score: candidate.score,
            })
            .collect(),
    }
}
