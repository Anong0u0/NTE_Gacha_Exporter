import type {
  AssetRefs,
  GachaRuleView,
  PoolKind,
  Profile,
  RateUpResult,
  RecordDerived,
  ResolvedBanner,
} from "../types";

export const mockProfile: Profile = {
  name: "default",
  created_at: "0",
  updated_at: "0",
  active: true,
};

export const mockItemAssetRefs: Record<string, AssetRefs> = {
  rare_1: {
    portrait: "/Game/UI/UI/Gacha/YH_lihui_character_anhunqu.YH_lihui_character_anhunqu",
    icon: "/Game/UI/UI_Icon/Character/Sigrid.Sigrid",
  },
  rare_item_1: {
    icon: "/Game/UI/UI_Icon/Item/rare_item_1.rare_item_1",
  },
  fork_1: {
    portrait: "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose",
    icon: "/Game/UI/UI_Icon/Fork/256/fork_Rose.fork_Rose",
  },
};

export type MockScenario = "default" | "unknown-banners" | "capture-stalled";
const MOCK_SCENARIO_KEY = "nte.mockScenario";

export function mockBanner(
  bannerId: string,
  poolKind: PoolKind,
  bannerType: "limited" | "standard" | "fork",
  title: string,
): ResolvedBanner {
  const limitedAssetRefs: AssetRefs = {
    image: "/Game/UI/UI/Gacha/Activityillustate/YH_UI_choukahuodong_xinzheng03.YH_UI_choukahuodong_xinzheng03",
    featured_portraits: ["/Game/UI/UI/Gacha/YH_lihui_character_anhunqu.YH_lihui_character_anhunqu"],
  };
  const forkAssetRefs: AssetRefs = {
    background: "/Game/UI/UI/ForkShop/UI_YH_Shoppingmall_hupandibanahqbg.UI_YH_Shoppingmall_hupandibanahqbg",
    icon: "/Game/UI/UI_Icon/Fork/1024/fork_Rose.fork_Rose",
  };
  return {
    banner_id: bannerId,
    pool_kind: poolKind,
    banner_type: bannerType,
    title,
    rate_up_5: [],
    rate_up_4: [],
    standard_5_pool: [],
    standard_4_pool: [],
    rule_id: poolKind === "fork_lottery" ? "fork_lottery_s" : poolKind,
    asset_refs: bannerType === "fork" ? forkAssetRefs : limitedAssetRefs,
  };
}

function mockRule(poolKind: PoolKind, ruleId: string): GachaRuleView {
  return {
    rule_id: ruleId,
    pool_kind: poolKind,
    hard_pity_5: poolKind === "fork_lottery" ? 60 : 90,
    hard_up_pity_5: poolKind === "fork_lottery" ? 80 : null,
    pickup_win_rate_5: poolKind === "fork_lottery" ? 25 : null,
    has_guarantee_5: poolKind === "fork_lottery" ? true : false,
    guarantee_scope: poolKind === "fork_lottery" ? "pool_kind" : "unknown",
    carry_scope: "pool_kind",
  };
}

export function mockDerived(
  recordId: string,
  options: {
    bannerId: string;
    poolKind: PoolKind;
    countsAsPull?: boolean;
    globalPullNo?: number | null;
    pullNoInPoolKind: number | null;
    pullNoInBanner: number | null;
    pity5Before: number;
    pity5After: number;
    tenPullProgressBefore?: number | null;
    tenPullProgressAfter: number | null;
    hitRarity: number | null;
    rateUpResult: RateUpResult;
    pityBadge?: RecordDerived["pity_badge"];
    guarantee5Before: boolean;
    guarantee5After: boolean;
    ruleId: string;
  },
): RecordDerived {
  return {
    record_id: recordId,
    banner_id: options.bannerId,
    banner_version: null,
    counts_as_pull: options.countsAsPull ?? true,
    global_pull_no: options.countsAsPull === false ? null : (options.globalPullNo ?? options.pullNoInPoolKind),
    pull_no_in_pool_kind: options.pullNoInPoolKind,
    pull_no_in_banner: options.pullNoInBanner,
    pity_5_before: options.pity5Before,
    pity_5_after: options.pity5After,
    ten_pull_progress_before: options.tenPullProgressBefore ?? options.tenPullProgressAfter,
    ten_pull_progress_after: options.tenPullProgressAfter,
    hit_rarity: options.hitRarity,
    rate_up_result: options.rateUpResult,
    pity_badge: options.pityBadge ?? null,
    guarantee_5_before: options.guarantee5Before,
    guarantee_5_after: options.guarantee5After,
    fork_up_pity_before: options.poolKind === "fork_lottery" ? options.pity5Before : null,
    fork_up_pity_after: options.poolKind === "fork_lottery" ? options.pity5After : null,
    fork_forced_up: options.poolKind === "fork_lottery" && options.hitRarity === 5 && options.rateUpResult === "up" ? false : null,
    rule: mockRule(options.poolKind, options.ruleId),
  };
}

export function mockSyntheticBanner(
  bannerId: string,
  poolId: string,
  poolKind: PoolKind,
  bannerType: "limited" | "fork",
  title: string | null,
  resolutionIssue: NonNullable<ResolvedBanner["resolution_issue"]>,
): ResolvedBanner {
  return {
    resolution_issue: resolutionIssue,
    reason: "mock synthetic banner",
    banner_id: bannerId,
    pool_id: poolId,
    pool_kind: poolKind,
    banner_type: bannerType,
    title,
    rate_up_5: [],
    rate_up_4: [],
    standard_5_pool: [],
    standard_4_pool: [],
    rule_id: null,
    asset_refs: {},
  };
}

export function mockScenario(): MockScenario {
  if (typeof window === "undefined") return "default";
  const value = window.localStorage.getItem(MOCK_SCENARIO_KEY);
  return value === "unknown-banners" || value === "capture-stalled" ? value : "default";
}
