import { computed, ref, type ComputedRef, type Ref } from "vue";

import type {
  BannerSummary,
  DashboardSelection,
  DashboardSelectionDetail,
  DisplayRecord,
  FiveStarRecord,
  PoolKind,
  PoolKindSummary,
} from "../api";
import type { I18nKey } from "./i18n";
import { type FiveStarDistanceMode, type FiveStarWallMode } from "./recordPrefs";
import { rarityClass } from "./rarityColors";
import { dashboardRaritySlices } from "./rarityBuckets";
import { bannerTitle, formatQuantityName } from "./viewHelpers";

type RankingRarity = (typeof rankingRarities)[number];
type RankingRaritySelection = Record<RankingRarity, boolean>;
type FiveStarWallGroupDraft = {
  boundary: number;
  displayDistance: number;
  hits: FiveStarRecord[];
};
export type FiveStarWallDisplayGroup = {
  key: string;
  hits: FiveStarRecord[];
  displayDistance: number;
  distanceMode: FiveStarDistanceMode;
  recordIds: string;
  currentBannerPulls: number | null;
  otherBannerPulls: number | null;
};
type Translator = (
  key: I18nKey,
  params?: Record<string, string | number | boolean | null | undefined>,
) => string;

const rankingRarities = [3, 4, 5] as const;

type DashboardUiDeps = {
  detail: Ref<DashboardSelectionDetail | null>;
  selectedDashboardScope: Ref<DashboardSelection>;
  selectedPoolKind: Ref<PoolKind>;
  selectedPoolSummary: ComputedRef<PoolKindSummary | null>;
  bannerSummaries: ComputedRef<BannerSummary[]>;
  latestFiveStarWallModes: Ref<Record<PoolKind, FiveStarWallMode>>;
  latestFiveStarDistanceModes: Ref<Record<PoolKind, FiveStarDistanceMode>>;
  t: Translator;
  saveRecordViewPrefs(): void;
};

export function createDashboardUi(deps: DashboardUiDeps) {
  const rankingDialogOpen = ref(false);
  const rankingRaritySelectionsByPoolKind = ref<Record<PoolKind, RankingRaritySelection>>({
    monopoly_limited: defaultRankingRaritySelection(),
    monopoly_standard: defaultRankingRaritySelection(),
    fork_lottery: defaultRankingRaritySelection(),
  });
  const fiveWallExpandedByPoolKind = ref<Record<PoolKind, boolean>>({
    monopoly_limited: false,
    monopoly_standard: false,
    fork_lottery: false,
  });

  const selectedSummary = computed(() =>
    deps.detail.value?.summary ?? (deps.selectedDashboardScope.value.kind === "pool_kind" ? deps.selectedPoolSummary.value : null),
  );
  const selectedScopeLabel = computed(() => {
    const scope = deps.selectedDashboardScope.value;
    if (scope.kind === "banner") {
      const banner = deps.bannerSummaries.value.find((banner) => banner.banner_id === scope.banner_id);
      return banner ? bannerTitle(banner, deps.t) : undefined;
    }
    return deps.selectedPoolSummary.value?.label;
  });
  const isDashboardPoolScope = computed(() => deps.selectedDashboardScope.value.kind === "pool_kind");
  const showDashboardBannerRail = computed(() => deps.selectedPoolKind.value !== "monopoly_standard");
  const selectedDetailTitle = computed(() => {
    if (isDashboardPoolScope.value) return deps.t("dashboard.poolDetail");
    const label = selectedScopeLabel.value?.trim();
    return label ? `${label} ${deps.t("dashboard.detailSuffix")}` : deps.t("dashboard.bannerDetail");
  });
  const hasItemRankingRows = computed(() => Boolean(deps.detail.value?.item_ranking.length));
  const rankingRarityOptions = computed(() => {
    const selection = rankingRaritySelectionsByPoolKind.value[deps.selectedPoolKind.value];
    return rankingRarities.map((rarity) => ({
      rarity,
      label: `${rarity}★`,
      active: selection[rarity],
      className: rarityClass(rarity),
    }));
  });
  const selectedRankingRarities = computed(() => new Set(
    rankingRarities.filter((rarity) => rankingRaritySelectionsByPoolKind.value[deps.selectedPoolKind.value][rarity]),
  ));
  const itemRankingShares = computed(() => {
    const selectedRarities = selectedRankingRarities.value;
    const ranking = (deps.detail.value?.item_ranking ?? []).filter((item) => isRankingRarity(item.rarity) && selectedRarities.has(item.rarity));
    const total = ranking.reduce((sum, item) => sum + item.count, 0);
    return ranking.map((item) => {
      const share = total > 0 ? item.count / total : 0;
      return {
        ...item,
        share,
        shareWidth: `${Math.round(share * 100)}%`,
      };
    });
  });
  const rankingDialogTitle = computed(() => `${selectedDetailTitle.value} · ${deps.t("dashboard.itemRanking")}`);
  const selectedRarityShares = computed(() => dashboardRaritySlices(deps.detail.value, deps.t));
  const latestFiveStarWallMode = computed(() => latestFiveStarWallModeForPool(deps.selectedPoolKind.value));
  const latestFiveStarDistanceMode = computed(() => latestFiveStarDistanceModeForPool(deps.selectedPoolKind.value));
  const showLatestFiveStarWallModeToggle = computed(() => true);
  const showLatestFiveStarDistanceModeToggle = computed(() => deps.selectedPoolKind.value === "fork_lottery");
  const visibleLatestFiveStarHits = computed(() => visibleFiveStarHits(deps.detail.value));
  const displayedLatestFiveStarGroups = computed(() => latestFiveStarGroups(visibleLatestFiveStarHits.value));
  const fiveWallExpanded = computed(() => Boolean(fiveWallExpandedByPoolKind.value[deps.selectedPoolKind.value]));
  const latestFiveStarEmptyText = computed(() => deps.t("dashboard.fiveStarRecordsEmpty"));

  function visibleFiveStarHits(scopeDetail?: DashboardSelectionDetail | null): FiveStarRecord[] {
    if (!scopeDetail) return [];
    const wallHistory = scopeDetail.five_star_wall_history ?? scopeDetail.five_star_history;
    if (latestFiveStarWallModeForPool(scopeDetail.summary.pool_kind) === "focused") {
      return wallHistory.filter((hit) => hit.focused_distance != null);
    }
    return wallHistory;
  }

  function latestFiveStarForPool(summary?: { pool_kind?: PoolKind; latest_5star?: DisplayRecord | null; latest_5star_any?: DisplayRecord | null } | null) {
    if (!summary) return null;
    return summary.latest_5star_any ?? summary.latest_5star ?? null;
  }

  function latestFiveStarNameForPool(summary?: { pool_kind?: PoolKind; latest_5star?: DisplayRecord | null; latest_5star_any?: DisplayRecord | null } | null) {
    const record = latestFiveStarForPool(summary);
    return record ? formatQuantityName(record.item_name, record.count) : "-";
  }

  function latestFiveStarWallModeForPool(poolKind?: PoolKind | null): FiveStarWallMode {
    return poolKind ? (deps.latestFiveStarWallModes.value[poolKind] ?? "all") : "all";
  }

  function latestFiveStarDistanceModeForPool(poolKind?: PoolKind | null): FiveStarDistanceMode {
    if (poolKind !== "fork_lottery") return "actual";
    return deps.latestFiveStarDistanceModes.value.fork_lottery ?? "actual";
  }

  function toggleLatestFiveStarWallMode() {
    const mode = latestFiveStarWallMode.value === "all" ? "focused" : "all";
    deps.latestFiveStarWallModes.value = {
      ...deps.latestFiveStarWallModes.value,
      [deps.selectedPoolKind.value]: mode,
    };
    deps.saveRecordViewPrefs();
  }

  function latestFiveStarWallToggleLabel() {
    if (deps.selectedPoolKind.value === "fork_lottery") {
      return latestFiveStarWallMode.value === "all" ? deps.t("dashboard.allFiveStar") : deps.t("dashboard.upFiveStarOnly");
    }
    return latestFiveStarWallMode.value === "all" ? deps.t("dashboard.showingFiveStarItems") : deps.t("dashboard.hidingFiveStarItems");
  }

  function toggleLatestFiveStarDistanceMode() {
    if (deps.selectedPoolKind.value !== "fork_lottery") return;
    const mode = latestFiveStarDistanceMode.value === "actual" ? "cost" : "actual";
    deps.latestFiveStarDistanceModes.value = {
      ...deps.latestFiveStarDistanceModes.value,
      fork_lottery: mode,
    };
    deps.saveRecordViewPrefs();
  }

  function latestFiveStarDistanceModeLabel() {
    return latestFiveStarDistanceMode.value === "cost" ? deps.t("dashboard.costPulls") : deps.t("dashboard.actualPulls");
  }

  function toggleFiveWallExpanded() {
    fiveWallExpandedByPoolKind.value = {
      ...fiveWallExpandedByPoolKind.value,
      [deps.selectedPoolKind.value]: !fiveWallExpanded.value,
    };
  }

  function summaryProgressLabel(summary?: { pool_kind?: PoolKind } | null) {
    return summary?.pool_kind === "fork_lottery" ? deps.t("dashboard.fourStarGuarantee") : deps.t("dashboard.giftProgress");
  }

  function pullCurrency(totalPulls?: number | null) {
    return ((totalPulls ?? 0) * 160).toLocaleString();
  }

  function formatPityRatio(current?: number | null, max?: number | null) {
    return `${current ?? "-"}/${max ?? "-"}`;
  }

  function recordRarityClass(record?: { rarity?: number | null } | null) {
    return rarityClass(record?.rarity);
  }

  function toggleRankingRarity(rarity: RankingRarity) {
    const selection = rankingRaritySelectionsByPoolKind.value[deps.selectedPoolKind.value];
    selection[rarity] = !selection[rarity];
  }

  function openRankingDialog() {
    if (hasItemRankingRows.value) rankingDialogOpen.value = true;
  }

  function closeRankingDialog() {
    rankingDialogOpen.value = false;
  }

  function fiveWallPityTone(pity: number, poolKind?: PoolKind) {
    if (poolKind === "fork_lottery") {
      if (pity > 50) return "pity-danger";
      if (pity > 30) return "pity-warn";
      return "pity-good";
    }
    if (pity <= 70) return "pity-good";
    if (pity < 90) return "pity-warn";
    return "pity-danger";
  }

  function latestFiveStarGroups(hits: FiveStarRecord[]): FiveStarWallDisplayGroup[] {
    const distanceMode = latestFiveStarDistanceModeForPool(deps.selectedPoolKind.value);
    if (distanceMode !== "cost") {
      return hits.map((hit) => buildFiveWallGroup([hit], actualFiveWallDistance(hit), "actual"));
    }

    const chronologicalHits = [...hits].sort(compareFiveWallHitsChronological);
    const groups: FiveStarWallGroupDraft[] = [];
    let fallbackPull = 0;

    for (const hit of chronologicalHits) {
      const actualDistance = actualFiveWallDistance(hit);
      const explicitPull = fiveWallCostPull(hit);
      const effectivePull = explicitPull ?? fallbackPull + actualDistance;
      fallbackPull = effectivePull;

      const boundary = costBoundary(effectivePull);
      const previousBoundary = costBoundary(Math.max(0, effectivePull - actualDistance));
      const displayDistance = Math.max(0, boundary - previousBoundary);
      const group = groups.at(-1);

      if (group?.boundary === boundary) {
        group.hits.push(hit);
        group.displayDistance += displayDistance;
      } else {
        groups.push({ boundary, displayDistance, hits: [hit] });
      }
    }

    return groups
      .reverse()
      .map((group) => buildFiveWallGroup([...group.hits].reverse(), group.displayDistance, "cost"));
  }

  function actualFiveWallDistance(hit: FiveStarRecord) {
    if (latestFiveStarWallModeForPool(hit.record.pool_kind) === "focused") return hit.focused_distance ?? hit.five_star_distance;
    return hit.five_star_distance;
  }

  function fiveWallCostPull(hit: FiveStarRecord) {
    const pullNo = hit.record.derived.pull_no_in_pool_kind;
    return typeof pullNo === "number" && Number.isFinite(pullNo) && pullNo > 0 ? pullNo : null;
  }

  function compareFiveWallHitsChronological(left: FiveStarRecord, right: FiveStarRecord) {
    const leftPull = fiveWallCostPull(left);
    const rightPull = fiveWallCostPull(right);
    if (leftPull != null && rightPull != null && leftPull !== rightPull) return leftPull - rightPull;
    return compareRecordTime(left.record.time, right.record.time) || left.record.source_order - right.record.source_order || left.record.record_id.localeCompare(right.record.record_id);
  }

  function costBoundary(pull: number) {
    return pull > 0 ? Math.ceil(pull / 10) * 10 : 0;
  }

  function buildFiveWallGroup(hits: FiveStarRecord[], displayDistance: number, distanceMode: FiveStarDistanceMode): FiveStarWallDisplayGroup {
    const breakdown = fiveWallBannerBreakdown(hits, displayDistance, distanceMode);
    return {
      key: hits.map((hit) => hit.record.record_id).join(":"),
      hits,
      displayDistance,
      distanceMode,
      recordIds: hits.map((hit) => hit.record.record_id).join(" "),
      currentBannerPulls: breakdown?.currentBannerPulls ?? null,
      otherBannerPulls: breakdown?.otherBannerPulls ?? null,
    };
  }

  function fiveWallBannerBreakdown(hits: FiveStarRecord[], displayDistance: number, distanceMode: FiveStarDistanceMode) {
    const scope = deps.selectedDashboardScope.value;
    if (scope.kind !== "banner") return null;

    const terminalHit = hits.reduce<FiveStarRecord | null>((latest, hit) => {
      const pullNo = finitePositiveNumber(hit.record.derived.pull_no_in_pool_kind);
      const latestPullNo = finitePositiveNumber(latest?.record.derived.pull_no_in_pool_kind);
      return pullNo != null && (latestPullNo == null || pullNo > latestPullNo) ? hit : latest;
    }, null);
    if (!terminalHit || terminalHit.record.derived.banner_id !== scope.banner_id) return null;

    const poolPull = finitePositiveNumber(terminalHit.record.derived.pull_no_in_pool_kind);
    const bannerPull = finitePositiveNumber(terminalHit.record.derived.pull_no_in_banner);
    if (poolPull == null || bannerPull == null || bannerPull > poolPull) return null;

    const pullsBeforeBanner = poolPull - bannerPull;
    const intervalEnd = distanceMode === "cost" ? costBoundary(poolPull) : poolPull;
    const bannerBoundary = distanceMode === "cost" ? costBoundary(pullsBeforeBanner) : pullsBeforeBanner;
    const intervalStart = intervalEnd - displayDistance;
    const otherBannerPulls = Math.min(displayDistance, Math.max(0, bannerBoundary - intervalStart));
    if (otherBannerPulls <= 0) return null;

    return {
      currentBannerPulls: displayDistance - otherBannerPulls,
      otherBannerPulls,
    };
  }

  function fiveWallGroupItemLabel(group: FiveStarWallDisplayGroup) {
    return group.hits.map((hit) => formatQuantityName(hit.record.item_name, hit.record.count)).join(", ");
  }

  function fiveWallDistanceLabel(group: FiveStarWallDisplayGroup) {
    if (group.currentBannerPulls == null || group.otherBannerPulls == null) return String(group.displayDistance);
    return deps.t("dashboard.crossBannerPullBreakdown", {
      total: group.displayDistance,
      current: group.currentBannerPulls,
      other: group.otherBannerPulls,
    });
  }

  return {
    refs: { rankingDialogOpen },
    computed: {
      selectedSummary,
      selectedScopeLabel,
      isDashboardPoolScope,
      selectedDetailTitle,
      hasItemRankingRows,
      rankingRarityOptions,
      itemRankingShares,
      rankingDialogTitle,
      selectedRarityShares,
      latestFiveStarWallMode,
      latestFiveStarDistanceMode,
      showLatestFiveStarWallModeToggle,
      showLatestFiveStarDistanceModeToggle,
      visibleLatestFiveStarHits,
      displayedLatestFiveStarGroups,
      fiveWallExpanded,
      latestFiveStarEmptyText,
      showDashboardBannerRail,
    },
    actions: {
      latestFiveStarForPool,
      latestFiveStarNameForPool,
      toggleLatestFiveStarWallMode,
      latestFiveStarWallToggleLabel,
      toggleLatestFiveStarDistanceMode,
      latestFiveStarDistanceModeLabel,
      toggleFiveWallExpanded,
      toggleRankingRarity,
      fiveWallPityTone,
      fiveWallGroupItemLabel,
      fiveWallDistanceLabel,
      summaryProgressLabel,
      pullCurrency,
      formatPityRatio,
      recordRarityClass,
      openRankingDialog,
      closeRankingDialog,
    },
    internal: {
      latestFiveStarWallModeForPool,
      latestFiveStarDistanceModeForPool,
    },
  };
}

function defaultRankingRaritySelection(): RankingRaritySelection {
  return { 3: true, 4: true, 5: true };
}

function isRankingRarity(value?: number | null): value is RankingRarity {
  return value === 3 || value === 4 || value === 5;
}

function compareRecordTime(left?: string | null, right?: string | null) {
  if (left != null && right == null) return -1;
  if (left == null && right != null) return 1;
  return String(left ?? "").localeCompare(String(right ?? ""));
}

function finitePositiveNumber(value?: number | null) {
  return typeof value === "number" && Number.isFinite(value) && value > 0 ? value : null;
}
