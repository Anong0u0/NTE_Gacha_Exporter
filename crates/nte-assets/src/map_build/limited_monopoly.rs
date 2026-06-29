fn text_ref_source_text(value: Option<&Value>, localization: &Localization) -> Option<String> {
    let value = value?;
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(text_ref) => [
            "CultureInvariantString",
            "SourceString",
            "LocalizedString",
        ]
        .into_iter()
        .find_map(|field| text_ref.get(field).and_then(value_to_text))
        .filter(|value| !value.is_empty())
        .or_else(|| localized_text(Some(value), localization)),
        _ => None,
    }
}

fn canonical_title_key(value: &str) -> String {
    strip_rich_text(value)
        .chars()
        .filter(|char| !char.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

fn quoted_title(value: &str) -> Option<String> {
    for (left, right) in [
        ("「", "」"),
        ("『", "』"),
        ("“", "”"),
        ("\"", "\""),
        ("'", "'"),
    ] {
        let Some(start) = value.find(left) else {
            continue;
        };
        let content_start = start + left.len();
        if let Some(end) = value[content_start..].find(right) {
            let content_end = content_start + end;
            return clean_pool_title(Some(value[content_start..content_end].to_string()));
        }
    }
    None
}

fn source_monopoly_localization(assets_root: &Path, localization: &Localization) -> Localization {
    for locale in ["zh-Hans", "zh-CN"] {
        let path = assets_root
            .join("Localization")
            .join(locale)
            .join("game.json");
        if path.exists() {
            if let Ok(source) = load_localization(assets_root, locale) {
                return source;
            }
        }
    }
    localization.clone()
}

fn limited_monopoly_pool_tails(assets_root: &Path) -> Result<Vec<String>, GuiError> {
    let path = assets_root.join(MONOPOLY_CELL_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut expected: Option<Vec<String>> = None;
    for (row_id, row) in rows_from_datatable(&path)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(drop_data) = row.get("PoolDropDatas").and_then(Value::as_array) else {
            continue;
        };
        let mut tails = drop_data
            .iter()
            .filter_map(Value::as_object)
            .filter_map(|entry| entry.get("Key"))
            .filter_map(value_to_text)
            .filter_map(|key| key.strip_prefix("Lottery_").map(ToString::to_string))
            .filter(|tail| tail != "Permanent")
            .collect::<Vec<_>>();
        dedupe_strings(&mut tails);
        if tails.is_empty() {
            continue;
        }
        if let Some(expected) = &expected {
            if &tails != expected {
                return Err(invalid(format!(
                    "inconsistent monopoly limited pool order in {MONOPOLY_CELL_TABLE}: row {row_id}"
                )));
            }
        } else {
            expected = Some(tails);
        }
    }
    Ok(expected.unwrap_or_default())
}

fn monopoly_pool_title_keys_by_tail(
    assets_root: &Path,
    localization: &Localization,
    tails: &[String],
) -> BTreeMap<String, String> {
    let source_localization = source_monopoly_localization(assets_root, localization);
    tails
        .iter()
        .filter_map(|tail| {
            localized_monopoly_pool_title(&source_localization, tail)
                .or_else(|| localized_monopoly_pool_title(localization, tail))
                .map(|title| (tail.clone(), canonical_title_key(&title)))
        })
        .collect()
}

fn dice_tail_title_keys(
    assets_root: &Path,
    localization: &Localization,
) -> Result<BTreeMap<String, String>, GuiError> {
    let path = assets_root.join(INVENTORY_TABLE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let mut links = BTreeMap::new();
    for (row_id, row) in rows_from_datatable(&path)? {
        let Some(tail) = row_id.strip_prefix("Dicelimite_") else {
            continue;
        };
        let Some(row) = row.as_object() else {
            continue;
        };
        let title = text_ref_source_text(row.get("UseContext"), localization)
            .and_then(|text| quoted_title(&text))
            .or_else(|| {
                localized_text(row.get("UseContext"), localization).and_then(|text| quoted_title(&text))
            });
        if let Some(title) = title {
            links.insert(tail.to_lowercase(), canonical_title_key(&title));
        }
    }
    Ok(links)
}

fn lottery_show_roles(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<BTreeMap<String, Vec<String>>, GuiError> {
    let path = assets_root.join(INVENTORY_TABLE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let row_id = Regex::new(r"^(?P<item_id>\d+)_LotteryShow_(?P<tail>.+)$")
        .expect("valid regex");
    let mut roles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (id, row) in rows_from_datatable(&path)? {
        let Some(captures) = row_id.captures(&id) else {
            continue;
        };
        let Some(row) = row.as_object() else {
            continue;
        };
        if rarity_from_quality(row.get("ItemQuality")) != Some(5) {
            continue;
        }
        let name = text_ref_source_text(row.get("ItemName"), localization)
            .or_else(|| localized_text(row.get("ItemName"), localization))
            .unwrap_or_default();
        if !name.contains("1.8700%") {
            continue;
        }
        let item_id = canonicalizer.canonicalize(&captures["item_id"]);
        if known_item_ids.is_some_and(|known| !known.contains(&item_id)) {
            continue;
        }
        roles
            .entry(captures["tail"].to_lowercase())
            .or_default()
            .push(item_id);
    }
    for item_ids in roles.values_mut() {
        dedupe_strings(item_ids);
    }
    Ok(roles)
}

fn linked_role_ids_for_pool(
    tail: &str,
    title_key: Option<&str>,
    role_ids_by_tail: &BTreeMap<String, Vec<String>>,
    dice_title_keys: &BTreeMap<String, String>,
) -> Vec<String> {
    let direct_tail = tail.to_lowercase();
    if let Some(role_ids) = role_ids_by_tail.get(&direct_tail) {
        return role_ids.clone();
    }
    let Some(title_key) = title_key else {
        return Vec::new();
    };
    for (dice_tail, dice_title_key) in dice_title_keys {
        if dice_title_key == title_key {
            if let Some(role_ids) = role_ids_by_tail.get(dice_tail) {
                return role_ids.clone();
            }
        }
    }
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AssetDateTime {
    year: u32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
}

impl AssetDateTime {
    fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        Some(Self {
            year: object.get("Year").and_then(Value::as_u64)?.try_into().ok()?,
            month: object.get("Month").and_then(Value::as_u64)?.try_into().ok()?,
            day: object.get("Day").and_then(Value::as_u64)?.try_into().ok()?,
            hour: object.get("Hour").and_then(Value::as_u64)?.try_into().ok()?,
            minute: object.get("minute").and_then(Value::as_u64)?.try_into().ok()?,
            second: object.get("Second").and_then(Value::as_u64)?.try_into().ok()?,
        })
    }

    fn from_schedule(value: Option<&Value>) -> Option<Self> {
        let object = value?.as_object()?;
        let mut date_time = object
            .get("OverseaTime")
            .and_then(Self::from_value)
            .or_else(|| object.get("MainlandTime").and_then(Self::from_value))?;
        if object
            .get("OverseaUseUTCTime")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            date_time = date_time.add_hours(8);
        }
        (date_time.year > 1).then_some(date_time)
    }

    fn add_hours(mut self, hours: u32) -> Self {
        self.hour += hours;
        while self.hour >= 24 {
            self.hour -= 24;
            self.day += 1;
            if self.day > days_in_month(self.year, self.month) {
                self.day = 1;
                self.month += 1;
                if self.month > 12 {
                    self.month = 1;
                    self.year += 1;
                }
            }
        }
        self
    }

    fn format(self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }

    fn banner_end_at(self) -> String {
        if self.hour == 6 && self.minute == 30 && self.second == 0 {
            return format!(
                "{:04}-{:02}-{:02} 05:59:00",
                self.year, self.month, self.day
            );
        }
        self.format()
    }
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

fn is_leap_year(year: u32) -> bool {
    year % 400 == 0 || (year % 4 == 0 && year % 100 != 0)
}

fn character_show_boundaries(
    assets_root: &Path,
) -> Result<BTreeMap<String, AssetDateTime>, GuiError> {
    let path = assets_root.join(CHARACTER_TABLE);
    if !path.exists() {
        return Ok(BTreeMap::new());
    }
    let mut boundaries = BTreeMap::new();
    for (character_id, row) in rows_from_datatable(&path)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(element_data) = row.get("ElementData").and_then(Value::as_object) else {
            continue;
        };
        if let Some(show_time) = AssetDateTime::from_schedule(element_data.get("ShowTime")) {
            boundaries.insert(character_id, show_time);
        }
    }
    Ok(boundaries)
}

fn combat_award_start_boundaries(assets_root: &Path) -> Result<Vec<AssetDateTime>, GuiError> {
    let path = assets_root.join(COMBAT_AWARD_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut boundaries = rows_from_datatable(&path)?
        .values()
        .filter_map(Value::as_object)
        .filter_map(|row| AssetDateTime::from_schedule(row.get("StartDateTime")))
        .filter(|date_time| date_time.year >= 2024)
        .collect::<Vec<_>>();
    boundaries.sort();
    boundaries.dedup();
    Ok(boundaries)
}

fn assign_limited_end_boundaries(
    role_ids_by_pool: &[Vec<String>],
    character_boundaries: &BTreeMap<String, AssetDateTime>,
    combat_boundaries: &[AssetDateTime],
) -> Vec<Option<AssetDateTime>> {
    let mut end_boundaries = vec![None; role_ids_by_pool.len()];
    for (pool_index, role_ids) in role_ids_by_pool.iter().enumerate().skip(1) {
        if let Some(boundary) = role_ids
            .iter()
            .filter_map(|role_id| character_boundaries.get(role_id).copied())
            .min()
        {
            end_boundaries[pool_index - 1] = Some(boundary);
        }
    }

    let mut used = end_boundaries
        .iter()
        .flatten()
        .copied()
        .collect::<BTreeSet<_>>();
    for slot in 0..end_boundaries.len() {
        if end_boundaries[slot].is_some() {
            continue;
        }
        let lower = end_boundaries[..slot].iter().rev().flatten().next().copied();
        let upper = end_boundaries[slot + 1..].iter().flatten().next().copied();
        let candidate = combat_boundaries.iter().copied().find(|boundary| {
            !used.contains(boundary)
                && lower.is_none_or(|lower| *boundary > lower)
                && upper.is_none_or(|upper| *boundary < upper)
        });
        if let Some(candidate) = candidate {
            used.insert(candidate);
            end_boundaries[slot] = Some(candidate);
        }
    }
    end_boundaries
}

fn limited_monopoly_banners(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<Vec<LimitedMonopolyBanner>, GuiError> {
    let tails = limited_monopoly_pool_tails(assets_root)?;
    if tails.is_empty() {
        return Ok(Vec::new());
    }
    let title_keys = monopoly_pool_title_keys_by_tail(assets_root, localization, &tails);
    let dice_title_keys = dice_tail_title_keys(assets_root, localization)?;
    let role_ids_by_tail =
        lottery_show_roles(assets_root, localization, canonicalizer, known_item_ids)?;
    let role_ids_by_pool = tails
        .iter()
        .map(|tail| {
            linked_role_ids_for_pool(
                tail,
                title_keys.get(tail).map(String::as_str),
                &role_ids_by_tail,
                &dice_title_keys,
            )
        })
        .collect::<Vec<_>>();
    let end_boundaries = assign_limited_end_boundaries(
        &role_ids_by_pool,
        &character_show_boundaries(assets_root)?,
        &combat_award_start_boundaries(assets_root)?,
    );

    let mut banners = Vec::new();
    let mut previous_end = None;
    for ((tail, rate_up_5), end_boundary) in tails
        .into_iter()
        .zip(role_ids_by_pool)
        .zip(end_boundaries)
    {
        let end_at = end_boundary.map(AssetDateTime::banner_end_at);
        let title = localized_monopoly_pool_title(localization, &tail).or_else(|| {
            let source_localization = source_monopoly_localization(assets_root, localization);
            localized_monopoly_pool_title(&source_localization, &tail)
        });
        if let (Some(title), Some(end_at)) = (title, end_at.clone()) {
            if !rate_up_5.is_empty() {
                banners.push(LimitedMonopolyBanner {
                    banner_id: format!("monopoly_limited_{tail}"),
                    title,
                    start_at_tz8: previous_end.clone(),
                    end_at_tz8: end_at.clone(),
                    rate_up_5,
                });
            }
        }
        if let Some(end_at) = end_at {
            previous_end = Some(end_at);
        }
    }
    Ok(banners)
}
