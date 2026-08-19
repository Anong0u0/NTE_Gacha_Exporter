import type { ItemKind, RecordFilterOptions, RollBucket } from "../types";
import { mockScenario, type MockScenario } from "./common";

const mockFilterOptions: RecordFilterOptions = {
  banner_occurrences: [
    {
      occurrence_id: "limited_mock@2026-01",
      source_id: "limited_mock",
      resolution_source: "catalog",
      pool_kind: "monopoly_limited",
      title: "Limited Board · 2026-01-01",
      active_from: "2026-01-01 00:00:00",
      active_until: null,
      count: 146,
    },
    {
      occurrence_id: "ForkLottery_AnHunQu@2026-01",
      source_id: "ForkLottery_AnHunQu",
      resolution_source: "catalog",
      pool_kind: "fork_lottery",
      title: "Arc Research · 2026-01-01",
      active_from: "2026-01-01 00:00:00",
      active_until: null,
      count: 36,
    },
  ],
  roll_buckets: [
    { bucket: "gift", count: 0 },
    { bucket: "sleep", count: 0 },
    { bucket: "1", count: 0 },
    { bucket: "2", count: 0 },
    { bucket: "3", count: 0 },
    { bucket: "4", count: 0 },
    { bucket: "5", count: 0 },
    { bucket: "6", count: 0 },
    { bucket: "not_applicable", count: 3 },
  ] satisfies { bucket: RollBucket; count: number }[],
  item_kinds: [
    { item_kind: "character", label: "Character", count: 1 },
    { item_kind: "fork", label: "Arc", count: 1 },
    { item_kind: "fashion", label: "Fashion", count: 0 },
    { item_kind: "glider", label: "Glider", count: 0 },
    { item_kind: "inventory", label: "Item", count: 1 },
    { item_kind: "vehicle_module", label: "Mod Parts", count: 0 },
    { item_kind: "unknown", label: "unknown", count: 0 },
  ] satisfies { item_kind: ItemKind; label: string; count: number }[],
};

const mockUnknownFilterBanners: RecordFilterOptions["banner_occurrences"] = [
  {
    occurrence_id: "ForkLottery_KaesiNew@observed-20260801100100",
    source_id: "ForkLottery_KaesiNew",
    resolution_source: "observed_gap",
    pool_kind: "fork_lottery",
    resolution_issue: "unknown_pool",
    title: "KaesiNew · 2026-08-01",
    active_from: "2026-08-01 10:01:00",
    active_until: null,
    count: 1,
  },
  {
    occurrence_id: "Lottery_Unknown@observed-20260801100000",
    source_id: "Lottery_Unknown",
    resolution_source: "observed_gap",
    pool_kind: "monopoly_limited",
    resolution_issue: "unknown_lottery_id",
    title: "Lottery_Unknown · 2026-08-01",
    active_from: "2026-08-01 10:00:00",
    active_until: null,
    count: 1,
  },
];

export function mockFilterOptionsForScenario(scenario: MockScenario = mockScenario()): RecordFilterOptions {
  if (scenario === "unknown-banners") {
    return {
      ...mockFilterOptions,
      banner_occurrences: [
        ...mockUnknownFilterBanners,
        ...mockFilterOptions.banner_occurrences,
      ],
    };
  }
  if (scenario === "latest-five-cost-distance") {
    return {
      ...mockFilterOptions,
      banner_occurrences: mockFilterOptions.banner_occurrences.map((banner) =>
        banner.source_id === "ForkLottery_AnHunQu" ? { ...banner, count: 300 } : banner,
      ),
    };
  }
  return mockFilterOptions;
}
