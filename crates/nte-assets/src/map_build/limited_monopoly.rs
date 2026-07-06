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
    let mut merged = Localization::new();
    for locale in ["zh-Hans", "zh-CN"] {
        let path = assets_root
            .join("Localization")
            .join(locale)
            .join("game.json");
        if path.exists() {
            if let Ok(source) = load_localization(assets_root, locale) {
                for (namespace, values) in source {
                    merged.entry(namespace).or_default().extend(values);
                }
            }
        }
    }
    if merged.is_empty() {
        localization.clone()
    } else {
        merged
    }
}

fn canonical_identity_key(value: &str) -> String {
    strip_rich_text(value)
        .chars()
        .filter(|char| char.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn tail_node(value: &str) -> Option<String> {
    let key = canonical_identity_key(value);
    (!key.is_empty()).then(|| format!("tail:{key}"))
}

fn title_node(value: &str) -> Option<String> {
    let key = canonical_title_key(value);
    (!key.is_empty()).then(|| format!("title:{key}"))
}

fn role_node(value: &str) -> String {
    format!("role:{}", value.to_ascii_lowercase())
}

fn display_name_node(value: &str) -> Option<String> {
    let key = canonical_identity_key(value);
    (!key.is_empty()).then(|| format!("name:{key}"))
}

fn localization_key_node(namespace: &str, key: &str) -> Option<String> {
    let key = canonical_identity_key(&format!("{namespace}:{key}"));
    (!key.is_empty()).then(|| format!("loc:{key}"))
}

#[derive(Debug, Clone)]
struct LimitedPoolSource {
    tail: String,
    identity_tails: Vec<String>,
}

#[derive(Debug, Default)]
struct LimitedIdentityGraph {
    parents: BTreeMap<String, String>,
}

impl LimitedIdentityGraph {
    fn add_node(&mut self, node: String) {
        self.parents.entry(node.clone()).or_insert(node);
    }

    fn add_edge(&mut self, left: Option<String>, right: Option<String>) {
        let (Some(left), Some(right)) = (left, right) else {
            return;
        };
        let left_root = self.root(left);
        let right_root = self.root(right);
        if left_root != right_root {
            self.parents.insert(right_root, left_root);
        }
    }

    fn root(&mut self, node: String) -> String {
        self.add_node(node.clone());
        let mut root = node.clone();
        loop {
            let parent = self.parents.get(&root).cloned().unwrap_or_else(|| root.clone());
            if parent == root {
                break;
            }
            root = parent;
        }
        let mut current = node;
        loop {
            let parent = self
                .parents
                .get(&current)
                .cloned()
                .unwrap_or_else(|| current.clone());
            if parent == root {
                break;
            }
            self.parents.insert(current.clone(), root.clone());
            current = parent;
        }
        root
    }
}

fn map_drop_tail(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "None" {
        return None;
    }
    let tail = trimmed
        .strip_prefix("Lottery_")
        .unwrap_or(trimmed)
        .split('_')
        .next()
        .unwrap_or(trimmed)
        .trim();
    (!tail.is_empty() && tail != "Permanent").then(|| tail.to_string())
}

fn add_pool_source(
    sources: &mut Vec<LimitedPoolSource>,
    seen_tails: &mut BTreeSet<String>,
    source: LimitedPoolSource,
) {
    if seen_tails.insert(source.tail.clone()) {
        sources.push(source);
    }
}

fn limited_monopoly_pool_sources(assets_root: &Path) -> Result<Vec<LimitedPoolSource>, GuiError> {
    let path = assets_root.join(MONOPOLY_CELL_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut expected_order: Option<Vec<String>> = None;
    let mut map_tail_counts: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for (row_id, row) in rows_from_datatable(&path)? {
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(drop_data) = row.get("PoolDropDatas").and_then(Value::as_array) else {
            continue;
        };
        let mut row_sources = Vec::new();
        let mut seen_tails = BTreeSet::new();
        for entry in drop_data.iter().filter_map(Value::as_object) {
            let Some(tail) = entry
                .get("Key")
                .and_then(value_to_text)
                .and_then(|key| key.strip_prefix("Lottery_").map(ToString::to_string))
                .filter(|tail| tail != "Permanent")
            else {
                continue;
            };
            let mut identity_tails = Vec::new();
            if let Some(map_drop_datas) = entry
                .get("Value")
                .and_then(Value::as_object)
                .and_then(|value| value.get("MapDropDatas"))
                .and_then(Value::as_array)
            {
                for map_drop in map_drop_datas.iter().filter_map(Value::as_object) {
                    if let Some(map_tail) =
                        map_drop.get("Value").and_then(value_to_text).and_then(|value| {
                            map_drop_tail(&value)
                        })
                    {
                        identity_tails.push(map_tail);
                    }
                }
            }
            dedupe_strings(&mut identity_tails);
            let counts = map_tail_counts.entry(tail.clone()).or_default();
            for identity_tail in &identity_tails {
                *counts.entry(identity_tail.clone()).or_default() += 1;
            }
            add_pool_source(
                &mut row_sources,
                &mut seen_tails,
                LimitedPoolSource {
                    tail,
                    identity_tails: Vec::new(),
                },
            );
        }
        if row_sources.is_empty() {
            continue;
        }
        let row_order = row_sources
            .iter()
            .map(|source| source.tail.clone())
            .collect::<Vec<_>>();
        if let Some(expected_order) = &expected_order {
            if &row_order != expected_order {
                return Err(invalid(format!(
                    "inconsistent monopoly limited pool order in {MONOPOLY_CELL_TABLE}: row {row_id}"
                )));
            }
        } else {
            expected_order = Some(row_order);
        }
    }
    Ok(expected_order
        .unwrap_or_default()
        .into_iter()
        .map(|tail| {
            let mut identity_tails = vec![tail.clone()];
            if let Some(counts) = map_tail_counts.get(&tail) {
                let max_count = counts.values().copied().max().unwrap_or(0);
                identity_tails.extend(
                    counts
                        .iter()
                        .filter(|(_, count)| **count == max_count)
                        .map(|(map_tail, _)| map_tail.clone()),
                );
            }
            dedupe_strings(&mut identity_tails);
            LimitedPoolSource {
                tail,
                identity_tails,
            }
        })
        .collect())
}

fn localized_key_limited_case_insensitive(
    localization: &Localization,
    namespace: &str,
    key: &str,
) -> Result<Option<String>, GuiError> {
    let Some(values) = localization.get(namespace) else {
        return Ok(None);
    };
    if let Some(value) = values.get(key) {
        return Ok(Some(value.clone()));
    }
    let hits = values
        .iter()
        .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    let unique = hits.iter().collect::<BTreeSet<_>>();
    if unique.len() > 1 {
        return Err(invalid(format!(
            "case-insensitive duplicate limited monopoly localization key: {namespace}.{key}"
        )));
    }
    Ok(hits.first().cloned())
}

fn localized_limited_monopoly_pool_titles(
    localization: &Localization,
    tail: &str,
) -> Result<Vec<String>, GuiError> {
    let mut titles = Vec::new();
    for template in MONOPOLY_DESCRIPTION_KEYS {
        let key = template.replace("{tail}", tail);
        if let Some(title) = localized_key_limited_case_insensitive(
            localization,
            MONOPOLY_TITLE_NAMESPACE,
            &key,
        )?
        .and_then(|text| description_pool_title(Some(text)))
        {
            titles.push(title);
        }
    }
    for suffix in title_suffix_candidates(tail) {
        let key = format!("{MONOPOLY_TITLE_PREFIX}{suffix}");
        if let Some(title) = localized_key_limited_case_insensitive(
            localization,
            MONOPOLY_TITLE_NAMESPACE,
            &key,
        )?
        .and_then(|text| clean_name(Some(text)))
        {
            titles.push(title);
        }
    }
    dedupe_strings(&mut titles);
    Ok(titles)
}

fn limited_banner_title(
    assets_root: &Path,
    locale: &str,
    localization: &Localization,
    tail: &str,
) -> Result<Option<String>, GuiError> {
    if let Some(title) = localized_limited_monopoly_pool_titles(localization, tail)?
        .into_iter()
        .next()
    {
        return Ok(Some(title));
    }
    if locale.to_ascii_lowercase().starts_with("zh") {
        let source_localization = source_monopoly_localization(assets_root, localization);
        if let Some(title) = localized_limited_monopoly_pool_titles(&source_localization, tail)?
            .into_iter()
            .next()
        {
            return Ok(Some(title));
        }
        return Ok(None);
    }
    Ok(Some(tail.to_string()))
}

fn limited_pool_titles_for_graph(
    assets_root: &Path,
    localization: &Localization,
    tail: &str,
) -> Result<Vec<String>, GuiError> {
    let mut titles = localized_limited_monopoly_pool_titles(localization, tail)?;
    let source_localization = source_monopoly_localization(assets_root, localization);
    titles.extend(localized_limited_monopoly_pool_titles(
        &source_localization,
        tail,
    )?);
    dedupe_strings(&mut titles);
    Ok(titles)
}

#[derive(Debug, Clone)]
struct DiceTitle {
    tail: String,
    title: String,
}

fn dice_tail_titles(
    assets_root: &Path,
    localization: &Localization,
) -> Result<Vec<DiceTitle>, GuiError> {
    let path = assets_root.join(INVENTORY_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut links = Vec::new();
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
            links.push(DiceTitle {
                tail: tail.to_string(),
                title,
            });
        }
    }
    Ok(links)
}

#[derive(Debug, Clone)]
struct LotteryShowRole {
    tail: String,
    item_id: String,
    display_names: Vec<String>,
}

fn lottery_show_display_name(value: &str) -> Option<String> {
    let name = value
        .split(['（', '('])
        .next()
        .unwrap_or(value)
        .trim()
        .to_string();
    clean_name(Some(name))
}

fn lottery_show_role_entries(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<Vec<LotteryShowRole>, GuiError> {
    let path = assets_root.join(INVENTORY_TABLE);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let row_id = Regex::new(r"^(?P<item_id>\d+)_LotteryShow_(?P<tail>.+)$")
        .expect("valid regex");
    let mut roles = Vec::new();
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
        let mut display_names = Vec::new();
        if let Some(name) = lottery_show_display_name(&name) {
            display_names.push(name);
        }
        dedupe_strings(&mut display_names);
        roles.push(LotteryShowRole {
            tail: captures["tail"].to_string(),
            item_id,
            display_names,
        });
    }
    Ok(roles)
}

fn lottery_show_roles(
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<BTreeMap<String, Vec<String>>, GuiError> {
    let mut roles: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in lottery_show_role_entries(assets_root, localization, canonicalizer, known_item_ids)?
    {
        roles
            .entry(role.tail.to_lowercase())
            .or_default()
            .push(role.item_id);
    }
    for item_ids in roles.values_mut() {
        dedupe_strings(item_ids);
    }
    Ok(roles)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct AssetDateTime {
    value: NaiveDateTime,
}

impl AssetDateTime {
    fn from_value(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        let year = object.get("Year").and_then(Value::as_i64)?.try_into().ok()?;
        let month = object.get("Month").and_then(Value::as_u64)?.try_into().ok()?;
        let day = object.get("Day").and_then(Value::as_u64)?.try_into().ok()?;
        let hour = object.get("Hour").and_then(Value::as_u64)?.try_into().ok()?;
        let minute = object.get("minute").and_then(Value::as_u64)?.try_into().ok()?;
        let second = object.get("Second").and_then(Value::as_u64)?.try_into().ok()?;
        Some(Self {
            value: NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, second)?,
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
            date_time.value += Duration::hours(8);
        }
        (date_time.value.year() > 1).then_some(date_time)
    }

    fn format(self) -> String {
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.value.year(),
            self.value.month(),
            self.value.day(),
            self.value.hour(),
            self.value.minute(),
            self.value.second()
        )
    }

    fn banner_end_at(self) -> String {
        if self.value.hour() == 6 && self.value.minute() == 30 && self.value.second() == 0 {
            return format!(
                "{:04}-{:02}-{:02} 05:59:00",
                self.value.year(),
                self.value.month(),
                self.value.day()
            );
        }
        self.format()
    }

    fn plus_days(self, days: i64) -> Self {
        Self {
            value: self.value + Duration::days(days),
        }
    }
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
        .filter(|date_time| date_time.value.year() >= 2024)
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

fn add_character_identity_edges(
    graph: &mut LimitedIdentityGraph,
    assets_root: &Path,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<(), GuiError> {
    let path = assets_root.join(CHARACTER_TABLE);
    if !path.exists() {
        return Ok(());
    }
    for (character_id, row) in rows_from_datatable(&path)? {
        let item_id = canonicalizer.canonicalize(&character_id);
        if known_item_ids.is_some_and(|known| !known.contains(&item_id)) {
            continue;
        }
        let Some(row) = row.as_object() else {
            continue;
        };
        let Some(name) = localized_text(row.get("ItemName"), localization)
            .and_then(|name| clean_name(Some(name)))
        else {
            continue;
        };
        graph.add_edge(Some(role_node(&item_id)), display_name_node(&name));
    }
    Ok(())
}

fn build_limited_identity_graph(
    assets_root: &Path,
    locale: &str,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    sources: &[LimitedPoolSource],
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<(LimitedIdentityGraph, Vec<LotteryShowRole>), GuiError> {
    let mut graph = LimitedIdentityGraph::default();
    let source_localization = source_monopoly_localization(assets_root, localization);
    for source in sources {
        for identity_tail in &source.identity_tails {
            graph.add_edge(tail_node(&source.tail), tail_node(identity_tail));
        }
        for title in limited_pool_titles_for_graph(assets_root, localization, &source.tail)? {
            graph.add_edge(tail_node(&source.tail), title_node(&title));
        }
        for template in MONOPOLY_DESCRIPTION_KEYS {
            graph.add_edge(
                tail_node(&source.tail),
                localization_key_node(
                    MONOPOLY_TITLE_NAMESPACE,
                    &template.replace("{tail}", &source.tail),
                ),
            );
        }
        for suffix in title_suffix_candidates(&source.tail) {
            graph.add_edge(
                tail_node(&source.tail),
                localization_key_node(
                    MONOPOLY_TITLE_NAMESPACE,
                    &format!("{MONOPOLY_TITLE_PREFIX}{suffix}"),
                ),
            );
        }
    }

    for titles in [&source_localization, localization] {
        for (namespace, values) in titles {
            if namespace != MONOPOLY_TITLE_NAMESPACE {
                continue;
            }
            for (key, value) in values {
                let lower_key = key.to_ascii_lowercase();
                if lower_key.starts_with(&MONOPOLY_TITLE_PREFIX.to_ascii_lowercase())
                    || MONOPOLY_DESCRIPTION_KEYS.iter().any(|template| {
                        let (prefix, suffix) = template.split_once("{tail}").unwrap_or((template, ""));
                        lower_key.starts_with(&prefix.to_ascii_lowercase())
                            && lower_key.ends_with(&suffix.to_ascii_lowercase())
                    })
                {
                    if let Some(title) = description_pool_title(Some(value.clone()))
                        .or_else(|| clean_name(Some(value.clone())))
                    {
                        graph.add_edge(
                            localization_key_node(namespace, key),
                            title_node(&title),
                        );
                    }
                }
            }
        }
    }

    for dice in dice_tail_titles(assets_root, localization)? {
        graph.add_edge(tail_node(&dice.tail), title_node(&dice.title));
    }

    let roles = lottery_show_role_entries(assets_root, localization, canonicalizer, known_item_ids)?;
    for role in &roles {
        graph.add_edge(tail_node(&role.tail), Some(role_node(&role.item_id)));
        for display_name in &role.display_names {
            graph.add_edge(Some(role_node(&role.item_id)), display_name_node(display_name));
        }
    }

    add_character_identity_edges(
        &mut graph,
        assets_root,
        localization,
        canonicalizer,
        known_item_ids,
    )?;

    if !locale.to_ascii_lowercase().starts_with("zh") {
        add_character_identity_edges(
            &mut graph,
            assets_root,
            &source_localization,
            canonicalizer,
            known_item_ids,
        )?;
    }

    Ok((graph, roles))
}

fn role_ids_by_pool(
    graph: &mut LimitedIdentityGraph,
    sources: &[LimitedPoolSource],
    roles: &[LotteryShowRole],
) -> Result<Vec<Vec<String>>, GuiError> {
    let mut role_roots: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for role in roles {
        let root = graph.root(role_node(&role.item_id));
        role_roots.entry(root).or_default().push(role.item_id.clone());
    }
    for role_ids in role_roots.values_mut() {
        dedupe_strings(role_ids);
    }

    sources
        .iter()
        .map(|source| {
            let mut matched_roots = BTreeSet::new();
            for identity_tail in &source.identity_tails {
                if let Some(node) = tail_node(identity_tail) {
                    let root = graph.root(node);
                    if role_roots.contains_key(&root) {
                        matched_roots.insert(root);
                    }
                }
            }
            if matched_roots.len() > 1 {
                return Err(invalid(format!(
                    "conflicting limited monopoly identity links: {}",
                    source.tail
                )));
            }
            let mut role_ids = matched_roots
                .into_iter()
                .filter_map(|root| role_roots.get(&root))
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            dedupe_strings(&mut role_ids);
            Ok(role_ids)
        })
        .collect()
}

fn infer_last_end_boundary(
    slot: usize,
    end_boundaries: &mut [Option<AssetDateTime>],
    role_ids_by_pool: &[Vec<String>],
    character_boundaries: &BTreeMap<String, AssetDateTime>,
) {
    if end_boundaries.get(slot).is_none_or(Option::is_some) {
        return;
    }
    let Some(role_ids) = role_ids_by_pool.get(slot) else {
        return;
    };
    let start = role_ids
        .iter()
        .filter_map(|role_id| character_boundaries.get(role_id).copied())
        .min()
        .or_else(|| end_boundaries[..slot].iter().rev().flatten().next().copied());
    if let Some(start) = start {
        end_boundaries[slot] = Some(start.plus_days(21));
    }
}

fn limited_monopoly_banners(
    assets_root: &Path,
    locale: &str,
    localization: &Localization,
    canonicalizer: &ItemCanonicalizer,
    known_item_ids: Option<&BTreeSet<String>>,
) -> Result<Vec<LimitedMonopolyBanner>, GuiError> {
    let sources = limited_monopoly_pool_sources(assets_root)?;
    if sources.is_empty() {
        return Ok(Vec::new());
    }
    let (mut graph, roles) = build_limited_identity_graph(
        assets_root,
        locale,
        localization,
        canonicalizer,
        &sources,
        known_item_ids,
    )?;
    let role_ids_by_pool = role_ids_by_pool(&mut graph, &sources, &roles)?;
    let character_boundaries = character_show_boundaries(assets_root)?;
    let mut end_boundaries = assign_limited_end_boundaries(
        &role_ids_by_pool,
        &character_boundaries,
        &combat_award_start_boundaries(assets_root)?,
    );
    if !end_boundaries.is_empty() {
        infer_last_end_boundary(
            end_boundaries.len() - 1,
            &mut end_boundaries,
            &role_ids_by_pool,
            &character_boundaries,
        );
    }

    let mut banners = Vec::new();
    let mut previous_end = None;
    for ((source, rate_up_5), end_boundary) in sources
        .into_iter()
        .zip(role_ids_by_pool)
        .zip(end_boundaries)
    {
        let end_at = end_boundary.map(AssetDateTime::banner_end_at);
        let title = limited_banner_title(assets_root, locale, localization, &source.tail)?;
        if let Some(title) = title {
            if !rate_up_5.is_empty() {
                banners.push(LimitedMonopolyBanner {
                    banner_id: format!("monopoly_limited_{}", source.tail),
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
