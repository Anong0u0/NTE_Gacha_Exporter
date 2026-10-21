impl PageNumberReader {
    fn slash_candidates(&self, targets: &[TargetMask]) -> Vec<SlashCandidate> {
        let mut candidates = Vec::new();
        for (target_index, target) in targets.iter().enumerate() {
            let mut slots = HashSet::new();
            for component in &target.components {
                for split_count in component_split_counts(*component) {
                    slots.extend(split_component_slots(
                        *component,
                        split_count,
                        target.width,
                        target.height,
                    ));
                }
            }
            for slot in slots {
                if !has_fields_around(target, slot) {
                    continue;
                }
                // A digit may resemble '/', but must not become the separator
                // when another glyph explains the same pixels better.
                let Some(best) = self.classify_glyph(target, slot).into_iter().next() else {
                    continue;
                };
                if best.ch != '/' {
                    continue;
                }
                let score = best.score;
                let candidate = SlashCandidate {
                    target_index,
                    slot,
                    score,
                };
                candidates.push(candidate);
            }
        }
        candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
        candidates.truncate(32);
        candidates
    }
}

fn same_slash_anchor(
    targets: &[TargetMask],
    candidate: SlashCandidate,
    reference: SlashCandidate,
) -> bool {
    let candidate = absolute_slot_rect(&targets[candidate.target_index], candidate.slot);
    let reference = absolute_slot_rect(&targets[reference.target_index], reference.slot);
    let candidate_center_x = candidate.x.saturating_mul(2) + candidate.width as i32;
    let reference_center_x = reference.x.saturating_mul(2) + reference.width as i32;
    let candidate_center_y = candidate.y.saturating_mul(2) + candidate.height as i32;
    let reference_center_y = reference.y.saturating_mul(2) + reference.height as i32;
    candidate_center_x.abs_diff(reference_center_x) <= 4
        && candidate_center_y.abs_diff(reference_center_y) <= 4
        && candidate.width.abs_diff(reference.width) <= 3
        && candidate.height.abs_diff(reference.height) <= 3
}

fn has_fields_around(target: &TargetMask, slot: GlyphSlot) -> bool {
    let slot_center = slot.left.saturating_mul(2) + slot.width;
    let has_left = target
        .components
        .iter()
        .any(|component| component.left.saturating_mul(2) + component.width < slot_center);
    let has_right = target
        .components
        .iter()
        .any(|component| component.left.saturating_mul(2) + component.width > slot_center);
    has_left && has_right
}

fn field_components(
    target: &TargetMask,
    slash: GlyphSlot,
) -> (Vec<TextComponent>, Vec<TextComponent>) {
    let slash_center = slash.left.saturating_mul(2) + slash.width;
    let mut current = target
        .components
        .iter()
        .copied()
        .filter(|component| component.left.saturating_mul(2) + component.width < slash_center)
        .collect::<Vec<_>>();
    let mut total = target
        .components
        .iter()
        .copied()
        .filter(|component| component.left.saturating_mul(2) + component.width > slash_center)
        .collect::<Vec<_>>();
    current.sort_by_key(|component| (component.left, component.top));
    total.sort_by_key(|component| (component.left, component.top));
    (current, total)
}
