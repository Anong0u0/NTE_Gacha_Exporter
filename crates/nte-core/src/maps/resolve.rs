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
    chrono::NaiveDateTime::parse_from_str(&text, "%Y-%m-%d %H:%M:%S").ok()?;
    Some(text)
}

fn legacy_limited_source_id_by_time(map: &MapData, time: Option<&str>) -> Option<String> {
    let time = normalize_game_time(time)?;
    let mut matches = map
        .banner_occurrences
        .iter()
        .filter(|occurrence| occurrence_matches_time(occurrence, &time))
        .filter_map(|occurrence| {
            map.banner_sources
                .get(&occurrence.source_id)
                .filter(|source| source.pool_id == "CardPool_Character")
                .map(|_| occurrence.source_id.clone())
        })
        .collect::<Vec<_>>();
    matches.sort();
    matches.dedup();
    (matches.len() == 1).then(|| matches.remove(0))
}

fn legacy_banner_id(value: Option<&str>) -> Result<Option<String>, GuiError> {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let canonical = if value == "monopoly_standard" {
        "Lottery_Permanent".to_string()
    } else if value.starts_with("Lottery_") {
        value.to_string()
    } else if let Some(tail) = value.strip_prefix("monopoly_limited_") {
        format!("Lottery_{tail}")
    } else {
        return Err(GuiError::InvalidDocument(format!(
            "unknown legacy banner_id: {value}"
        )));
    };
    if !crate::is_valid_lottery_id(&canonical) {
        return Err(GuiError::InvalidDocument(format!(
            "invalid legacy banner_id: {value}"
        )));
    }
    Ok(Some(canonical))
}

impl MapData {
    pub fn resolve_legacy_lottery_id(
        &self,
        pool_id: &str,
        time: Option<&str>,
        old_banner_id: Option<&str>,
    ) -> Result<Option<String>, GuiError> {
        if pool_id == "CardPool_NewRole" {
            if let Some(id) = legacy_banner_id(old_banner_id)?
                && id != "Lottery_Permanent"
            {
                return Err(GuiError::InvalidDocument(format!(
                    "legacy banner conflicts with standard pool: {id}"
                )));
            }
            return Ok(Some("Lottery_Permanent".to_string()));
        }
        if pool_id != "CardPool_Character" {
            return Ok(None);
        }
        if let Some(id) = legacy_banner_id(old_banner_id)? {
            if self
                .banner_sources
                .get(&id)
                .is_some_and(|source| source.pool_id != pool_id)
            {
                return Err(GuiError::InvalidDocument(format!(
                    "legacy banner conflicts with pool: banner={id} pool={pool_id}"
                )));
            }
            return Ok(Some(id));
        }
        Ok(legacy_limited_source_id_by_time(self, time))
    }
}

fn resolved_source(source: &MapBannerSource) -> ResolvedBannerSource {
    ResolvedBannerSource {
        resolution_issue: None,
        reason: None,
        source_id: Some(source.source_id.clone()),
        pool_id: Some(source.pool_id.clone()),
        pool_kind: Some(source.pool_kind.clone()),
        banner_type: Some(source.banner_type.clone()),
        title: Some(source.title.clone()),
        rate_up_5: source.rate_up_5.clone(),
        rate_up_4: source.rate_up_4.clone(),
        rule_id: Some(source.rule_id.clone()),
        asset_refs: source.asset_refs.clone(),
    }
}

fn synthetic_source(
    input: BannerResolveInput<'_>,
    issue: BannerResolutionIssue,
    reason: impl Into<String>,
) -> ResolvedBannerSource {
    let (source_id, pool_kind, banner_type, title) = if input.pool_id == "CardPool_Character" {
        (
            input.lottery_id.map(ToOwned::to_owned),
            Some("monopoly_limited".to_string()),
            Some("limited".to_string()),
            None,
        )
    } else if input.pool_id == "CardPool_NewRole" {
        (
            input.lottery_id.map(ToOwned::to_owned),
            Some("monopoly_standard".to_string()),
            Some("standard".to_string()),
            None,
        )
    } else if let Some(title) = input.pool_id.strip_prefix("ForkLottery_") {
        (
            Some(input.pool_id.to_string()),
            Some("fork_lottery".to_string()),
            Some("fork".to_string()),
            Some(title.to_string()),
        )
    } else {
        (None, None, None, None)
    };
    ResolvedBannerSource {
        resolution_issue: Some(issue),
        reason: Some(reason.into()),
        source_id,
        pool_id: Some(input.pool_id.to_string()),
        pool_kind,
        banner_type,
        title,
        rate_up_5: Vec::new(),
        rate_up_4: Vec::new(),
        rule_id: None,
        asset_refs: BTreeMap::new(),
    }
}

fn occurrence_matches_time(occurrence: &MapBannerOccurrence, time: &str) -> bool {
    occurrence
        .active_from
        .as_deref()
        .is_none_or(|start| start <= time)
        && occurrence
            .active_until
            .as_deref()
            .is_none_or(|end| time < end)
}
