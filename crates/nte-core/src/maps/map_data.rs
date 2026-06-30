impl MapData {
    pub fn canonical_item_id<'a>(&'a self, item_id: &'a str) -> &'a str {
        if let Some(alias) = self.item_aliases.get(item_id) {
            return alias;
        }
        if self.items.contains_key(item_id) {
            return item_id;
        }
        self.unique_case_folded_item_id(item_id).unwrap_or(item_id)
    }

    fn unique_case_folded_item_id<'a>(&'a self, item_id: &str) -> Option<&'a str> {
        let mut matches = self
            .items
            .keys()
            .filter(|candidate| candidate.eq_ignore_ascii_case(item_id));
        let first = matches.next()?;
        matches.next().is_none().then_some(first.as_str())
    }

    pub fn item<'a>(&'a self, item_id: &'a str) -> Option<(&'a str, &'a MapItem)> {
        let canonical = self.canonical_item_id(item_id);
        self.items
            .get(canonical)
            .map(|item| (canonical, item))
            .or_else(|| self.items.get(item_id).map(|item| (item_id, item)))
    }

    pub fn item_name(&self, item_id: &str) -> String {
        self.item(item_id)
            .map(|(_, item)| item.name.clone())
            .unwrap_or_else(|| item_id.to_string())
    }

    pub fn item_rarity(&self, item_id: &str) -> Option<u8> {
        self.item(item_id).map(|(_, item)| item.rarity)
    }

    pub fn item_kind(&self, item_id: &str) -> ItemKind {
        self.item(item_id)
            .map(|(_, item)| {
                item_kind_from_taxonomy(
                    item.category.as_deref(),
                    item.domain_type.as_deref(),
                    item.subtype.as_deref(),
                )
            })
            .unwrap_or(ItemKind::Unknown)
    }

    pub fn label(&self, label_id: &str) -> String {
        self.labels
            .get(label_id)
            .cloned()
            .unwrap_or_else(|| label_id.to_string())
    }

    pub fn gacha_rule(&self, rule_id: &str) -> Option<&MapGachaRule> {
        self.gacha_rules.get(rule_id)
    }

    pub fn pool_label(&self, pool_id: &str, time: Option<&str>) -> String {
        self.banner_label(pool_id, time)
    }

    pub fn banner_label(&self, pool_id: &str, time: Option<&str>) -> String {
        let resolved = self.resolve_banner(pool_id, time);
        if resolved.resolution_issue.is_none() {
            if let Some(title) = resolved.title {
                return title;
            }
        }
        self.pool_fallback_label(pool_id, time)
    }

    fn pool_fallback_label(&self, pool_id: &str, time: Option<&str>) -> String {
        let Some(pool) = self.pools.get(pool_id) else {
            return pool_id.to_string();
        };
        if let (Some(record_time), Some(windows)) =
            (normalize_game_time(time), pool.title_windows.as_ref())
        {
            for window in windows {
                if let Some(end_at) = normalize_game_time(Some(&window.end_at_tz8)) {
                    if record_time.as_str() <= end_at.as_str() {
                        return window.title.clone();
                    }
                } else if record_time.as_str() <= window.end_at_tz8.as_str() {
                    return window.title.clone();
                }
            }
        }
        pool.title
            .clone()
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| pool.name.clone())
    }

    pub fn pool_kind_label(&self, kind: PoolKind) -> String {
        let pool_id = match kind {
            PoolKind::MonopolyLimited => "CardPool_Character",
            PoolKind::MonopolyStandard => "CardPool_NewRole",
            PoolKind::ForkLottery => self
                .pools
                .keys()
                .find(|pool_id| pool_id.starts_with("ForkLottery_"))
                .map(String::as_str)
                .unwrap_or("ForkLottery"),
        };
        self.pools
            .get(pool_id)
            .map(|pool| {
                pool.group_label
                    .clone()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| pool.name.clone())
            })
            .unwrap_or_else(|| kind.as_str().to_string())
    }

    pub fn has_pool_id(&self, pool_id: &str) -> bool {
        self.pools.contains_key(pool_id)
    }

    pub fn is_pickup_item(&self, pool_id: &str, item_id: &str) -> bool {
        let canonical = self.canonical_item_id(item_id);
        self.pools
            .get(pool_id)
            .and_then(|pool| pool.pickup_item_ids.as_ref())
            .is_some_and(|items| items.iter().any(|candidate| candidate == canonical))
    }

    pub fn is_banner_rate_up(
        &self,
        pool_id: &str,
        time: Option<&str>,
        item_id: &str,
        rarity: Option<u8>,
    ) -> bool {
        let canonical = self.canonical_item_id(item_id);
        let resolved = self.resolve_banner(pool_id, time);
        if resolved.resolution_issue.is_some() {
            return self.is_pickup_item(pool_id, item_id);
        }
        match rarity {
            Some(5) => resolved
                .rate_up_5
                .iter()
                .any(|candidate| candidate == canonical),
            Some(4) => resolved
                .rate_up_4
                .iter()
                .any(|candidate| candidate == canonical),
            _ => false,
        }
    }

    pub fn resolve_banner(&self, pool_id: &str, time: Option<&str>) -> ResolvedBanner {
        let Some(pool) = self.pools.get(pool_id) else {
            return synthetic_unresolved(pool_id, BannerResolutionIssue::UnknownPool, format!("pool is not in localization map: {pool_id}"))
                .unwrap_or_else(|| unresolved(
                BannerResolutionIssue::UnknownPool,
                format!("pool is not in localization map: {pool_id}"),
            ));
        };
        let Some(banner_ids) = pool.banner_ids.as_ref() else {
            return synthetic_unresolved(pool_id, BannerResolutionIssue::UnknownPool, format!("pool has no linked banners: {pool_id}"))
                .unwrap_or_else(|| unresolved(
                BannerResolutionIssue::UnknownPool,
                format!("pool has no linked banners: {pool_id}"),
            ));
        };

        let candidates = banner_ids
            .iter()
            .filter_map(|banner_id| self.banners.get(banner_id))
            .filter(|banner| banner.pool_id == pool_id)
            .collect::<Vec<_>>();
        if candidates.is_empty() {
            return synthetic_unresolved(pool_id, BannerResolutionIssue::UnknownPool, format!("pool has no usable linked banners: {pool_id}"))
                .unwrap_or_else(|| unresolved(
                BannerResolutionIssue::UnknownPool,
                format!("pool has no usable linked banners: {pool_id}"),
            ));
        }

        match pool_id {
            "CardPool_NewRole" => single_banner(candidates, "standard", "standard"),
            "CardPool_Character" => resolve_limited_banner(pool_id, candidates, time),
            value if value.starts_with("ForkLottery_") => {
                let matching_pool = candidates
                    .iter()
                    .copied()
                    .filter(|banner| banner.banner_id == pool_id)
                    .collect::<Vec<_>>();
                if matching_pool.is_empty() {
                    single_banner(candidates, "fork", "fork").or_synthetic(pool_id)
                } else {
                    single_banner(matching_pool, "fork", "fork").or_synthetic(pool_id)
                }
            }
            _ => unresolved(
                BannerResolutionIssue::UnknownPool,
                format!("pool has unsupported banner resolution: {pool_id}"),
            ),
        }
    }
}

fn item_kind_from_taxonomy(
    category: Option<&str>,
    domain_type: Option<&str>,
    subtype: Option<&str>,
) -> ItemKind {
    let normalized = category
        .or(domain_type)
        .or(subtype)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match normalized {
        Some("character") => ItemKind::Character,
        Some("fork") => ItemKind::Fork,
        Some("appearance") => ItemKind::Appearance,
        Some("inventory") => ItemKind::Inventory,
        Some("vehicle_module") => ItemKind::VehicleModule,
        _ => ItemKind::Unknown,
    }
}
