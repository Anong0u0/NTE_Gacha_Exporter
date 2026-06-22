fn rarity_distribution_from_display_refs<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
) -> Vec<RarityBucket> {
    let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
        if let Some(rarity) = record.rarity {
            *counts.entry(rarity).or_default() += 1;
        }
    }
    let known_total = counts.values().sum::<u64>();
    counts
        .into_iter()
        .rev()
        .map(|(rarity, count)| RarityBucket {
            rarity,
            count,
            percent: if known_total == 0 {
                0.0
            } else {
                count as f64 / known_total as f64
            },
        })
        .collect()
}

fn hit_rarity_distribution_from_display_refs<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
) -> Vec<RarityBucket> {
    let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
        match record.derived.hit_rarity {
            Some(5) if record.derived.rate_up_result == RateUpResult::Up => {
                *counts.entry(5).or_default() += 1;
            }
            Some(rarity @ (3 | 4)) => {
                *counts.entry(rarity).or_default() += 1;
            }
            _ => {}
        }
    }
    let known_total = counts.values().sum::<u64>();
    counts
        .into_iter()
        .rev()
        .map(|(rarity, count)| RarityBucket {
            rarity,
            count,
            percent: if known_total == 0 {
                0.0
            } else {
                count as f64 / known_total as f64
            },
        })
        .collect()
}

fn item_ranking_from_display_refs<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
    map: &MapData,
) -> Vec<ItemRank> {
    let mut counts: HashMap<String, ItemRankAccumulator> = HashMap::new();
    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
        let item_id = map.canonical_item_id(&record.item_id).to_string();
        let entry = counts.entry(item_id.clone()).or_insert_with(|| ItemRankAccumulator {
            item_name: map.item_name(&item_id),
            item_asset_refs: map
                .item(&item_id)
                .map(|(_, item)| item.asset_refs.clone())
                .unwrap_or_default(),
            rarity: map.item_rarity(&item_id),
            count: 0,
        });
        entry.count += 1;
    }
    let mut ranking = counts
        .into_iter()
        .map(|(item_id, item)| ItemRank {
            item_id,
            item_name: item.item_name,
            item_asset_refs: item.item_asset_refs,
            rarity: item.rarity,
            count: item.count,
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.rarity.cmp(&left.rarity))
            .then_with(|| left.item_name.cmp(&right.item_name))
    });
    ranking.truncate(20);
    ranking
}

struct ItemRankAccumulator {
    item_name: String,
    item_asset_refs: BTreeMap<String, serde_json::Value>,
    rarity: Option<u8>,
    count: u64,
}
