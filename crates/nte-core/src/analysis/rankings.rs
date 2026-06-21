fn rarity_distribution(records: &[InternalRecord], map: &MapData) -> Vec<RarityBucket> {
    let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
    for record in records {
        if let Some(rarity) = map.item_rarity(&record.item_id) {
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

fn rarity_distribution_from_display(records: &[&DisplayRecord]) -> Vec<RarityBucket> {
    let mut counts: BTreeMap<u8, u64> = BTreeMap::new();
    for record in records {
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

fn item_ranking(records: &[InternalRecord], map: &MapData) -> Vec<ItemRank> {
    let mut counts: HashMap<String, u64> = HashMap::new();
    for record in records {
        let item_id = map.canonical_item_id(&record.item_id).to_string();
        *counts.entry(item_id).or_default() += 1;
    }
    let mut ranking = counts
        .into_iter()
        .map(|(item_id, count)| ItemRank {
            item_name: map.item_name(&item_id),
            rarity: map.item_rarity(&item_id),
            item_id,
            count,
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

fn item_ranking_from_display(records: &[&DisplayRecord], map: &MapData) -> Vec<ItemRank> {
    let mut counts: HashMap<String, (String, Option<u8>, u64)> = HashMap::new();
    for record in records {
        let item_id = map.canonical_item_id(&record.item_id).to_string();
        let entry = counts
            .entry(item_id.clone())
            .or_insert_with(|| (map.item_name(&item_id), map.item_rarity(&item_id), 0));
        entry.2 += 1;
    }
    let mut ranking = counts
        .into_iter()
        .map(|(item_id, (item_name, rarity, count))| ItemRank {
            item_id,
            item_name,
            rarity,
            count,
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
