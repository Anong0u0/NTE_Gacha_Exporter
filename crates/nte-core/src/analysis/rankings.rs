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

fn pull_rarity_distribution_from_display_refs<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
    pool_kind: PoolKind,
) -> Vec<PullRarityBucket> {
    let mut counts: BTreeMap<PullRarityBucketKey, u64> = BTreeMap::new();
    let mut total = 0_u64;
    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
        total += 1;
        *counts
            .entry(pull_rarity_bucket_key(record, pool_kind))
            .or_default() += 1;
    }
    pull_rarity_bucket_order(pool_kind)
        .into_iter()
        .filter_map(|key| {
            let count = counts.get(&key).copied().unwrap_or_default();
            (count > 0).then(|| PullRarityBucket {
                key,
                rarity: pull_rarity_bucket_rarity(key),
                count,
                percent: if total == 0 {
                    0.0
                } else {
                    count as f64 / total as f64
                },
            })
        })
        .collect()
}

fn pull_rarity_bucket_key(
    record: &DisplayRecord,
    pool_kind: PoolKind,
) -> PullRarityBucketKey {
    match record.rarity {
        Some(5) => match pool_kind {
            PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => {
                if record.item_kind == ItemKind::Character {
                    PullRarityBucketKey::FiveCharacter
                } else {
                    PullRarityBucketKey::FiveItem
                }
            }
            PoolKind::ForkLottery => {
                if record.derived.hit_rarity == Some(5) {
                    match record.derived.rate_up_result {
                        RateUpResult::Up => PullRarityBucketKey::FiveUp,
                        RateUpResult::OffRate => PullRarityBucketKey::FiveNonUp,
                        RateUpResult::NotApplicable | RateUpResult::Unknown => {
                            PullRarityBucketKey::FiveItem
                        }
                    }
                } else {
                    PullRarityBucketKey::FiveItem
                }
            }
        },
        Some(4) => match pool_kind {
            PoolKind::MonopolyLimited | PoolKind::MonopolyStandard => {
                if record.item_kind == ItemKind::Character {
                    PullRarityBucketKey::FourCharacter
                } else {
                    PullRarityBucketKey::FourItem
                }
            }
            PoolKind::ForkLottery if record.derived.hit_rarity == Some(4) => {
                PullRarityBucketKey::FourHit
            }
            PoolKind::ForkLottery => PullRarityBucketKey::FourItem,
        },
        Some(3) => PullRarityBucketKey::Three,
        Some(_) | None => PullRarityBucketKey::Unknown,
    }
}

fn pull_rarity_bucket_order(pool_kind: PoolKind) -> Vec<PullRarityBucketKey> {
    match pool_kind {
        PoolKind::MonopolyLimited => vec![
            PullRarityBucketKey::FiveCharacter,
            PullRarityBucketKey::FiveItem,
            PullRarityBucketKey::FourCharacter,
            PullRarityBucketKey::FourItem,
            PullRarityBucketKey::Three,
            PullRarityBucketKey::Unknown,
        ],
        PoolKind::MonopolyStandard => vec![
            PullRarityBucketKey::FiveCharacter,
            PullRarityBucketKey::FiveItem,
            PullRarityBucketKey::FourCharacter,
            PullRarityBucketKey::FourItem,
            PullRarityBucketKey::Three,
            PullRarityBucketKey::Unknown,
        ],
        PoolKind::ForkLottery => vec![
            PullRarityBucketKey::FiveUp,
            PullRarityBucketKey::FiveNonUp,
            PullRarityBucketKey::FiveItem,
            PullRarityBucketKey::FourHit,
            PullRarityBucketKey::FourItem,
            PullRarityBucketKey::Three,
            PullRarityBucketKey::Unknown,
        ],
    }
}

fn pull_rarity_bucket_rarity(key: PullRarityBucketKey) -> Option<u8> {
    match key {
        PullRarityBucketKey::FiveUp
        | PullRarityBucketKey::FiveNonUp
        | PullRarityBucketKey::FiveCharacter
        | PullRarityBucketKey::FiveItem => Some(5),
        PullRarityBucketKey::FourCharacter
        | PullRarityBucketKey::FourHit
        | PullRarityBucketKey::FourItem => Some(4),
        PullRarityBucketKey::Three => Some(3),
        PullRarityBucketKey::Unknown => None,
    }
}

fn item_ranking_from_display_refs<'a>(
    records: impl IntoIterator<Item = &'a DisplayRecord>,
    map: &MapData,
) -> Vec<ItemRank> {
    let mut counts: HashMap<(String, i64), ItemRankAccumulator> = HashMap::new();
    for record in records {
        if !record.derived.counts_as_pull {
            continue;
        }
        let item_id = map.canonical_item_id(&record.item_id).to_string();
        let reward_count = record.count.unwrap_or(1);
        let entry =
            counts
                .entry((item_id.clone(), reward_count))
                .or_insert_with(|| ItemRankAccumulator {
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
        .map(|((item_id, reward_count), item)| ItemRank {
            item_id,
            item_name: item.item_name,
            item_asset_refs: item.item_asset_refs,
            rarity: item.rarity,
            reward_count,
            count: item.count,
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.rarity.cmp(&left.rarity))
            .then_with(|| left.item_name.cmp(&right.item_name))
            .then_with(|| left.reward_count.cmp(&right.reward_count))
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
