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
import { type FiveStarWallMode } from "./recordPrefs";
import { rarityClass } from "./rarityColors";
import { dashboardRaritySlices } from "./rarityBuckets";
import { bannerTitle, formatQuantityName } from "./viewHelpers";

type RankingRarity = (typeof rankingRarities)[number];
type RankingRaritySelection = Record<RankingRarity, boolean>;
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
  const showLatestFiveStarWallModeToggle = computed(() => true);
  const visibleLatestFiveStarHits = computed(() => visibleFiveStarHits(deps.detail.value));
  const displayedLatestFiveStarHits = computed(() => visibleLatestFiveStarHits.value);
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

  function fiveWallDistance(hit: FiveStarRecord) {
    if (latestFiveStarWallMode.value === "focused") return hit.focused_distance ?? hit.five_star_distance;
    return hit.five_star_distance;
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
      showLatestFiveStarWallModeToggle,
      visibleLatestFiveStarHits,
      displayedLatestFiveStarHits,
      fiveWallExpanded,
      latestFiveStarEmptyText,
      showDashboardBannerRail,
    },
    actions: {
      latestFiveStarForPool,
      latestFiveStarNameForPool,
      toggleLatestFiveStarWallMode,
      latestFiveStarWallToggleLabel,
      toggleFiveWallExpanded,
      toggleRankingRarity,
      fiveWallPityTone,
      fiveWallDistance,
      summaryProgressLabel,
      pullCurrency,
      formatPityRatio,
      recordRarityClass,
      openRankingDialog,
      closeRankingDialog,
    },
    internal: {
      latestFiveStarWallModeForPool,
    },
  };
}

function defaultRankingRaritySelection(): RankingRaritySelection {
  return { 3: true, 4: true, 5: true };
}

function isRankingRarity(value?: number | null): value is RankingRarity {
  return value === 3 || value === 4 || value === 5;
}
