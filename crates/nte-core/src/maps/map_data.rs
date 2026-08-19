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
                    item.subtype.as_deref(),
                )
            })
            .unwrap_or(ItemKind::Unknown)
    }

    pub fn item_kind_label(&self, item_kind: ItemKind) -> String {
        let label_id = match item_kind {
            ItemKind::Character => "item_kind_character",
            ItemKind::Fork => "item_kind_fork",
            ItemKind::Fashion => "item_kind_fashion",
            ItemKind::Glider => "item_kind_glider",
            ItemKind::Inventory => "item_kind_inventory",
            ItemKind::VehicleModule => "item_kind_vehicle_module",
            ItemKind::Unknown => "item_kind_unknown",
        };
        self.labels
            .get(label_id)
            .cloned()
            .unwrap_or_else(|| item_kind.as_str().to_string())
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

    pub fn pool_label(&self, input: BannerResolveInput<'_>) -> String {
        self.banner_source_label(input)
    }

    pub fn banner_source_label(&self, input: BannerResolveInput<'_>) -> String {
        let resolved = self.resolve_banner_source(input);
        if resolved.resolution_issue.is_none() {
            if let Some(title) = resolved.title {
                return title;
            }
        }
        let Some(pool) = self.pools.get(input.pool_id) else {
            return input.pool_id.to_string();
        };
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

    pub fn is_banner_source_rate_up(
        &self,
        input: BannerResolveInput<'_>,
        item_id: &str,
        rarity: Option<u8>,
    ) -> bool {
        let canonical = self.canonical_item_id(item_id);
        let resolved = self.resolve_banner_source(input);
        if resolved.resolution_issue.is_some() {
            return self.is_pickup_item(input.pool_id, item_id);
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

    pub fn resolve_banner_source(&self, input: BannerResolveInput<'_>) -> ResolvedBannerSource {
        if !self.pools.contains_key(input.pool_id) {
            return synthetic_source(
                input,
                BannerResolutionIssue::UnknownPool,
                format!("pool is not in localization map: {}", input.pool_id),
            );
        }
        if input.pool_id.starts_with("ForkLottery_") {
            return self.banner_sources.get(input.pool_id).map_or_else(
                || {
                    synthetic_source(
                        input,
                        BannerResolutionIssue::UnknownPool,
                        format!("fork banner is not in localization map: {}", input.pool_id),
                    )
                },
                resolved_source,
            );
        }
        let Some(lottery_id) = input.lottery_id else {
            return synthetic_source(
                input,
                BannerResolutionIssue::MissingLotteryId,
                "monopoly record is missing lottery_id",
            );
        };
        let Some(source) = self.banner_sources.get(lottery_id) else {
            return synthetic_source(
                input,
                BannerResolutionIssue::UnknownLotteryId,
                format!("lottery_id is not in localization map: {lottery_id}"),
            );
        };
        if source.pool_id != input.pool_id {
            return synthetic_source(
                input,
                BannerResolutionIssue::LotteryPoolMismatch,
                format!(
                    "lottery_id pool mismatch: {lottery_id} belongs to {}",
                    source.pool_id
                ),
            );
        }
        resolved_source(source)
    }
}

fn item_kind_from_taxonomy(category: Option<&str>, subtype: Option<&str>) -> ItemKind {
    let normalized = category
        .or(subtype)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    match normalized {
        Some("character") => ItemKind::Character,
        Some("fork") => ItemKind::Fork,
        Some("fashion") => ItemKind::Fashion,
        Some("glider") => ItemKind::Glider,
        Some("inventory") => ItemKind::Inventory,
        Some("vehicle_module") => ItemKind::VehicleModule,
        _ => ItemKind::Unknown,
    }
}
