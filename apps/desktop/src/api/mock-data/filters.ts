import type { ItemKind, RecordFilterOptions, RollBucket } from "../types";
import { mockScenario, type MockScenario } from "./common";

const mockFilterOptions: RecordFilterOptions = {
  banners: [
    { banner_id: "limited_mock", pool_kind: "monopoly_limited", title: "Limited Board", count: 146 },
    { banner_id: "ForkLottery_AnHunQu", pool_kind: "fork_lottery", title: "Arc Research", count: 36 },
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

const mockUnknownFilterBanners: RecordFilterOptions["banners"] = [
  { banner_id: "ForkLottery_KaesiNew", pool_kind: "fork_lottery", resolution_issue: "unknown_pool", title: "KaesiNew", count: 1 },
  { banner_id: "CardPool_Character", pool_kind: "monopoly_limited", resolution_issue: "outside_known_windows", title: "CardPool_Character", count: 1 },
];

export function mockFilterOptionsForScenario(scenario: MockScenario = mockScenario()): RecordFilterOptions {
  return scenario === "unknown-banners"
    ? {
        ...mockFilterOptions,
        banners: [...mockUnknownFilterBanners, ...mockFilterOptions.banners],
      }
    : mockFilterOptions;
}
