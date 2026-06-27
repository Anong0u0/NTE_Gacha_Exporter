import type {
  DashboardSelection,
  DashboardSelectionDetail,
  ImportReport,
  PoolKind,
  PoolKindSummary,
  PullRarityBucketKey,
  RecordFilter,
} from "../types";
import {
  mockBanners,
  mockProfile,
  mockRecords,
  mockSummary,
  mockTimeStats,
  type MockRecord,
} from "../mock-data";

export async function mockOverview() {
  return {
    profile: mockProfile,
    last_run: mockReport("default", "raw_jsonl", "sample.raw.jsonl"),
    total_records: 182,
    pool_kinds: [
      ...mockSummary,
      {
        pool_kind: "monopoly_standard" as const,
        label: "Standard Board",
        total_pulls: 0,
        roll_points_total: 0,
        known_roll_point_records: 0,
        missing_roll_point_records: 0,
        hit_count: 0,
        five_star_item_count: 0,
        current_pity: 0,
        current_ten_pull_progress: null,
        current_guarantee: false,
        hard_pity: 90,
        average_5star_pity: null,
        average_4star_pity: null,
        min_5star_pity: null,
        max_5star_pity: null,
        early_hit_count: 0,
        up_count: 0,
        off_rate_count: 0,
        not_applicable_rate_up_count: 0,
        unknown_rate_up_count: 0,
        observed_up_rate: null,
        fork_win_count: 0,
        fork_loss_count: 0,
        fork_forced_up_count: 0,
        fork_observed_25_75_win_rate: null,
        latest_5star: null,
        latest_5star_any: null,
        four_star_count: 0,
        rate_up_4_count: 0,
        off_rate_4_count: 0,
        not_applicable_rate_up_4_count: 0,
        unknown_rate_up_4_count: 0,
        average_roll_points_to_5star: null,
        roll_point_cost_samples_5star: 0,
      },
    ],
    banners: mockBanners,
    time_stats: mockTimeStats,
    rarity_distribution: [
      { rarity: 5, count: 3, percent: 0.016 },
      { rarity: 4, count: 18, percent: 0.099 },
      { rarity: 3, count: 161, percent: 0.885 },
    ],
    item_ranking: [
      { item_id: "common_2", item_name: "Training Log", item_asset_refs: {}, rarity: 3, reward_count: 1, count: 44 },
      { item_id: "rare_1", item_name: "Sigrid", item_asset_refs: mockRecords[1].item_asset_refs, rarity: 5, reward_count: 1, count: 2 },
    ],
  };
}

export function mockSelectionDetail(selection: DashboardSelection): DashboardSelectionDetail {
  const records = mockRecords.filter((record) =>
    selection.kind === "pool_kind"
      ? record.pool_kind === selection.pool_kind
      : record.pool_kind === selection.pool_kind && record.derived.banner_id === selection.banner_id,
  );
  const baseSummary =
    selection.kind === "pool_kind"
      ? mockSummary.find((item) => item.pool_kind === selection.pool_kind)
      : mockBanners.find((banner) => banner.banner_id === selection.banner_id);
  const label =
    selection.kind === "pool_kind"
      ? (baseSummary as PoolKindSummary | undefined)?.label
      : mockBanners.find((banner) => banner.banner_id === selection.banner_id)?.title;
  const poolKind = selection.pool_kind;
  const fallback = mockSummary.find((item) => item.pool_kind === poolKind) ?? mockSummary[0];
  const countableRecords = records.filter((record) => record.derived.counts_as_pull);
  const chronologicalRecords = [...countableRecords].sort(compareRecordsChronological);
  const fiveStarRecords = chronologicalRecords.filter((record) => record.derived.hit_rarity === 5);
  const fiveStarWallRecords = [...records].filter((record) => mockIsFiveStarWallRecord(record, poolKind)).sort(compareRecordsNewestFirst);
  const fourStarRecords = countableRecords.filter((record) => record.derived.hit_rarity === 4);
  const average4StarPity = mockAverage4StarPity(countableRecords);
  const summary: PoolKindSummary = {
    ...fallback,
    label: label ?? fallback.label,
    total_pulls: countableRecords.length,
    hit_count: fiveStarRecords.length,
    five_star_item_count: countableRecords.filter((record) => record.rarity === 5).length,
    up_count: countableRecords.filter((record) => record.rarity === 5 && record.derived.rate_up_result === "up").length,
    off_rate_count: countableRecords.filter((record) => record.rarity === 5 && record.derived.rate_up_result === "off_rate").length,
    not_applicable_rate_up_count: countableRecords.filter((record) => record.rarity === 5 && record.derived.rate_up_result === "not_applicable").length,
    unknown_rate_up_count: countableRecords.filter((record) => record.rarity === 5 && record.derived.rate_up_result === "unknown").length,
    four_star_count: fourStarRecords.length,
    average_4star_pity: average4StarPity,
    latest_5star: fiveStarRecords.at(-1) ?? null,
    latest_5star_any: fiveStarWallRecords[0] ?? null,
  };
  const rarityCounts = new Map<number, number>();
  for (const record of countableRecords) {
    if (record.rarity != null) rarityCounts.set(record.rarity, (rarityCounts.get(record.rarity) ?? 0) + 1);
  }
  const knownTotal = [...rarityCounts.values()].reduce((total, count) => total + count, 0);
  const rarity_distribution = [...rarityCounts.entries()]
    .sort(([left], [right]) => right - left)
    .map(([rarity, count]) => ({ rarity, count, percent: knownTotal ? count / knownTotal : 0 }));
  const hitRarityCounts = new Map<number, number>();
  for (const record of countableRecords) {
    if (record.derived.hit_rarity === 5 && record.derived.rate_up_result !== "up") continue;
    if (record.derived.hit_rarity === 5 || record.derived.hit_rarity === 4 || record.derived.hit_rarity === 3) {
      hitRarityCounts.set(record.derived.hit_rarity, (hitRarityCounts.get(record.derived.hit_rarity) ?? 0) + 1);
    }
  }
  const knownHitTotal = [...hitRarityCounts.values()].reduce((total, count) => total + count, 0);
  const hit_rarity_distribution = [...hitRarityCounts.entries()]
    .sort(([left], [right]) => right - left)
    .map(([rarity, count]) => ({ rarity, count, percent: knownHitTotal ? count / knownHitTotal : 0 }));
  const pullRarityCounts = new Map<PullRarityBucketKey, number>();
  for (const record of countableRecords) {
    const key = mockPullRarityBucketKey(record, poolKind);
    pullRarityCounts.set(key, (pullRarityCounts.get(key) ?? 0) + 1);
  }
  const pull_rarity_distribution = mockPullRarityBucketOrder(poolKind)
    .map((key) => {
      const count = pullRarityCounts.get(key) ?? 0;
      return {
        key,
        rarity: mockPullRarityBucketRarity(key),
        count,
        percent: countableRecords.length ? count / countableRecords.length : 0,
      };
    })
    .filter((bucket) => bucket.count > 0);
  const itemCounts = new Map<string, { item_id: string; item_name: string; item_asset_refs: Record<string, unknown>; rarity?: number | null; reward_count: number; count: number }>();
  for (const record of countableRecords) {
    const reward_count = record.count ?? 1;
    const key = `${record.item_id}\0${reward_count}`;
    const entry = itemCounts.get(key) ?? {
      item_id: record.item_id,
      item_name: record.item_name,
      item_asset_refs: record.item_asset_refs,
      rarity: record.rarity,
      reward_count,
      count: 0,
    };
    entry.count += 1;
    itemCounts.set(key, entry);
  }
  const item_ranking = [...itemCounts.values()]
    .sort(
      (left, right) =>
        right.count - left.count ||
        (right.rarity ?? 0) - (left.rarity ?? 0) ||
        left.item_name.localeCompare(right.item_name) ||
        left.reward_count - right.reward_count,
    );

  const fiveStarDistances = mockFiveStarDistances(records, poolKind);

  return {
    summary,
    five_star_history: fiveStarRecords.map((record) => ({
      record,
      pity_distance: record.derived.pity_5_before + 1,
      five_star_distance: fiveStarDistances.get(record.record_id)?.five_star_distance ?? record.derived.pity_5_before + 1,
      focused_distance: fiveStarDistances.get(record.record_id)?.focused_distance ?? null,
      result: record.derived.rate_up_result,
      guarantee_before: record.derived.guarantee_5_before,
      guarantee_after: record.derived.guarantee_5_after,
    })),
    five_star_wall_history: fiveStarWallRecords.map((record) => ({
      record,
      pity_distance: record.derived.pity_5_before + 1,
      five_star_distance: fiveStarDistances.get(record.record_id)?.five_star_distance ?? record.derived.pity_5_before + 1,
      focused_distance: fiveStarDistances.get(record.record_id)?.focused_distance ?? null,
      result: record.derived.rate_up_result,
      guarantee_before: record.derived.guarantee_5_before,
      guarantee_after: record.derived.guarantee_5_after,
    })),
    rarity_distribution,
    hit_rarity_distribution,
    pull_rarity_distribution,
    item_ranking,
  };
}

function mockFiveStarDistances(records: MockRecord[], poolKind: PoolKind) {
  const distances = new Map<string, { five_star_distance: number; focused_distance: number | null }>();
  let fallbackPull = 0;
  let currentPull = 0;
  let lastFiveStarPull: number | null = null;
  let currentFiveStarDistance: number | null = null;
  let lastFocusedPull: number | null = null;
  let currentFocusedDistance: number | null = null;

  for (const record of [...records].sort(compareRecordsChronological)) {
    if (record.derived.counts_as_pull) {
      fallbackPull += 1;
      currentPull = record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? fallbackPull;
    }
    const effectivePull = currentPull || record.derived.pull_no_in_banner || record.derived.pull_no_in_pool_kind || record.derived.pity_5_before + 1;
    if (!mockIsFiveStarWallRecord(record, poolKind)) continue;

    const fiveStarDistance: number =
      effectivePull === lastFiveStarPull
        ? currentFiveStarDistance ?? 0
        : effectivePull - (lastFiveStarPull ?? 0);
    lastFiveStarPull = effectivePull;
    currentFiveStarDistance = fiveStarDistance;

    let focusedDistance: number | null = null;
    if (mockIsFocusedFiveStarWallRecord(record)) {
      focusedDistance =
        effectivePull === lastFocusedPull
          ? currentFocusedDistance ?? 0
          : effectivePull - (lastFocusedPull ?? 0);
      lastFocusedPull = effectivePull;
      currentFocusedDistance = focusedDistance;
    }

    distances.set(record.record_id, { five_star_distance: fiveStarDistance, focused_distance: focusedDistance });
  }

  return distances;
}

function mockIsFiveStarWallRecord(record: MockRecord, poolKind: PoolKind) {
  return poolKind === "fork_lottery" ? record.derived.hit_rarity === 5 : record.rarity === 5;
}

function mockMatchesFocusedRarity(record: MockRecord, rarity: number) {
  if (rarity === 5) return mockIsFocusedFiveStarWallRecord(record);
  if (rarity === 3 || rarity === 4) return record.derived.hit_rarity === rarity;
  return false;
}

function mockIsFocusedFiveStarWallRecord(record: MockRecord) {
  if (record.pool_kind === "fork_lottery") {
    return record.derived.hit_rarity === 5 && record.derived.rate_up_result === "up";
  }
  return record.item_kind === "character" && record.rarity === 5;
}

function mockPullRarityBucketKey(record: MockRecord, poolKind: PoolKind): PullRarityBucketKey {
  if (record.rarity === 5) {
    if (poolKind === "monopoly_limited" || poolKind === "monopoly_standard") {
      return record.item_kind === "character" ? "five_character" : "five_item";
    }
    if (poolKind === "fork_lottery" && record.derived.hit_rarity === 5) {
      if (record.derived.rate_up_result === "up") return "five_up";
      if (record.derived.rate_up_result === "off_rate") return "five_non_up";
      return "five_item";
    }
    return "five_item";
  }
  if (record.rarity === 4) {
    if (poolKind === "monopoly_limited" || poolKind === "monopoly_standard") {
      return record.item_kind === "character" ? "four_character" : "four_item";
    }
    return record.derived.hit_rarity === 4 ? "four_hit" : "four_item";
  }
  if (record.rarity === 3) return "three";
  return "unknown";
}

function mockPullRarityBucketOrder(poolKind: PoolKind): PullRarityBucketKey[] {
  if (poolKind === "monopoly_standard" || poolKind === "monopoly_limited") {
    return ["five_character", "five_item", "four_character", "four_item", "three", "unknown"];
  }
  if (poolKind === "fork_lottery") return ["five_up", "five_non_up", "five_item", "four_hit", "four_item", "three", "unknown"];
  return ["five_character", "five_item", "four_character", "four_item", "three", "unknown"];
}

function mockPullRarityBucketRarity(key: PullRarityBucketKey) {
  if (key.startsWith("five_")) return 5;
  if (key.startsWith("four_")) return 4;
  if (key === "three") return 3;
  return null;
}

function mockAverage4StarPity(records: MockRecord[]) {
  const intervals: number[] = [];
  let current = 0;
  for (const record of [...records].sort((left, right) => (left.derived.pull_no_in_banner ?? 0) - (right.derived.pull_no_in_banner ?? 0))) {
    current += 1;
    if (record.derived.hit_rarity === 4 || record.derived.hit_rarity === 5) {
      intervals.push(current);
      current = 0;
    }
  }
  return intervals.length ? intervals.reduce((total, value) => total + value, 0) / intervals.length : null;
}

export function mockRecordPage(filter: RecordFilter) {
  const search = filter.search?.toLowerCase().trim();
  let records = mockRecords.filter((record) => {
    if (filter.pool_kind && record.pool_kind !== filter.pool_kind) return false;
    if (filter.banner_ids?.length && (!record.derived.banner_id || !filter.banner_ids.includes(record.derived.banner_id))) return false;
    if (filter.rarities?.length && (record.rarity == null || !filter.rarities.includes(record.rarity))) return false;
    if (filter.focused_rarities?.length && !filter.focused_rarities.some((rarity) => mockMatchesFocusedRarity(record, rarity))) return false;
    if (filter.rate_up_results?.length && !filter.rate_up_results.includes(record.derived.rate_up_result)) return false;
    if (filter.roll_buckets?.length && !filter.roll_buckets.includes(record.roll_bucket)) return false;
    if (filter.item_kinds?.length && !filter.item_kinds.includes(record.item_kind)) return false;
    if (filter.fork_result_marks?.length && (!record.fork_result_mark || !filter.fork_result_marks.includes(record.fork_result_mark))) return false;
    if (filter.fork_pity_badges?.length) {
      const badge = record.derived.pity_badge;
      if (!badge || !filter.fork_pity_badges.includes(badge)) return false;
    }
    if (search && !`${record.item_name} ${record.item_id}`.toLowerCase().includes(search)) return false;
    return true;
  });
  const direction = filter.sort_direction ?? "desc";
  records = [...records].sort(direction === "asc" ? compareRecordsOldestFirst : compareRecordsNewestFirst);
  const offset = filter.offset ?? 0;
  const limit = filter.limit ?? 50;
  return { total: records.length, records: records.slice(offset, offset + limit) };
}

function compareRecordsChronological(left: MockRecord, right: MockRecord) {
  return (
    compareTimeAsc(left.time, right.time) ||
    left.source_order - right.source_order ||
    left.record_id.localeCompare(right.record_id)
  );
}

function compareRecordsNewestFirst(left: MockRecord, right: MockRecord) {
  return (
    compareTimeDesc(left.time, right.time) ||
    left.source_order - right.source_order ||
    left.record_id.localeCompare(right.record_id)
  );
}

function compareRecordsOldestFirst(left: MockRecord, right: MockRecord) {
  return compareRecordsNewestFirst(right, left);
}

function compareTimeAsc(left?: string | null, right?: string | null) {
  if (left != null && right == null) return -1;
  if (left == null && right != null) return 1;
  return String(left ?? "").localeCompare(String(right ?? ""));
}

function compareTimeDesc(left?: string | null, right?: string | null) {
  if (left != null && right == null) return -1;
  if (left == null && right != null) return 1;
  return String(right ?? "").localeCompare(String(left ?? ""));
}
export function mockReport(profileName: string, sourceKind: string, sourcePath: string): ImportReport {
  return {
    profile_name: profileName,
    source_kind: sourceKind,
    source_path: sourcePath,
    records_seen: mockRecords.length,
    records_inserted: mockRecords.length,
    records_skipped: 0,
    completed_at: String(Date.now()),
  };
}
