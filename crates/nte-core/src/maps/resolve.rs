fn normalize_game_time(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.len() < 19 || raw.contains('+') || raw.contains('Z') || raw.contains('z') {
        return None;
    }
    let mut text = raw.get(..19)?.replace('T', " ");
    let bytes = text.as_bytes();
    let valid = bytes.len() == 19
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b' '
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7 | 10 | 13 | 16) || byte.is_ascii_digit());
    if !valid {
        return None;
    }
    if raw.len() > 19 {
        let suffix = raw.get(19..)?;
        if !suffix.starts_with('.') || !suffix[1..].bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
    }
    text.truncate(19);
    Some(text)
}

fn resolve_limited_banner(candidates: Vec<&MapBanner>, time: Option<&str>) -> ResolvedBanner {
    let record_time = match normalize_game_time(time) {
        Some(value) => value,
        None => {
            return unresolved(
                BannerResolutionStatus::UnknownTime,
                "limited banner resolution requires valid record time",
            );
        }
    };
    let mut windows = candidates
        .into_iter()
        .filter(|banner| banner.banner_type == "limited")
        .filter_map(|banner| {
            normalize_game_time(banner.end_at.as_deref()).map(|end_at| {
                (
                    normalize_game_time(banner.start_at.as_deref()),
                    end_at,
                    banner,
                )
            })
        })
        .collect::<Vec<_>>();
    windows.sort_by(|left, right| left.1.cmp(&right.1));

    if windows.is_empty() {
        return unresolved(
            BannerResolutionStatus::UnknownPool,
            "pool has no linked limited banners",
        );
    }

    let mut matches = Vec::new();
    let mut previous_end: Option<String> = None;
    for (start_at, end_at, banner) in windows {
        let effective_start = start_at.as_ref().or(previous_end.as_ref());
        let in_window = match effective_start {
            Some(start) => {
                start.as_str() < record_time.as_str() && record_time.as_str() <= end_at.as_str()
            }
            None => record_time.as_str() <= end_at.as_str(),
        };
        if in_window {
            matches.push(banner);
        }
        previous_end = Some(end_at);
    }

    match matches.len() {
        1 => matched(matches[0]),
        0 => unresolved(
            BannerResolutionStatus::OutsideKnownWindows,
            "record time is outside known limited banner windows",
        ),
        _ => unresolved(
            BannerResolutionStatus::Ambiguous,
            "multiple limited banners match record time",
        ),
    }
}

fn single_banner(
    candidates: Vec<&MapBanner>,
    banner_type: &str,
    reason_label: &str,
) -> ResolvedBanner {
    let matches = candidates
        .into_iter()
        .filter(|banner| banner.banner_type == banner_type)
        .collect::<Vec<_>>();
    match matches.len() {
        1 => matched(matches[0]),
        0 => unresolved(
            BannerResolutionStatus::UnknownPool,
            format!("pool has no linked {reason_label} banner"),
        ),
        _ => unresolved(
            BannerResolutionStatus::Ambiguous,
            format!("multiple {reason_label} banners are linked"),
        ),
    }
}

fn matched(banner: &MapBanner) -> ResolvedBanner {
    ResolvedBanner {
        status: BannerResolutionStatus::Matched,
        reason: "matched".to_string(),
        banner_id: Some(banner.banner_id.clone()),
        pool_id: Some(banner.pool_id.clone()),
        pool_kind: Some(banner.pool_kind.clone()),
        banner_type: Some(banner.banner_type.clone()),
        title: Some(banner.title.clone()),
        version: banner.version.clone(),
        phase: banner.phase.clone(),
        start_at: banner.start_at.clone(),
        end_at: banner.end_at.clone(),
        timezone: banner.timezone.clone(),
        rate_up_5: banner.rate_up_5.clone(),
        rate_up_4: banner.rate_up_4.clone(),
        rule_id: Some(banner.rule_id.clone()),
        asset_refs: banner.asset_refs.clone(),
        source_confidence: Some(banner.source.confidence.clone()),
    }
}

fn unresolved(status: BannerResolutionStatus, reason: impl Into<String>) -> ResolvedBanner {
    ResolvedBanner {
        status,
        reason: reason.into(),
        banner_id: None,
        pool_id: None,
        pool_kind: None,
        banner_type: None,
        title: None,
        version: None,
        phase: None,
        start_at: None,
        end_at: None,
        timezone: None,
        rate_up_5: Vec::new(),
        rate_up_4: Vec::new(),
        rule_id: None,
        asset_refs: BTreeMap::new(),
        source_confidence: None,
    }
}

