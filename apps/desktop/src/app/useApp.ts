import { PieChart } from "echarts/charts";
import { TooltipComponent } from "echarts/components";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  api,
  type BackupReport,
  type CaptureMode,
  type CaptureStatus,
  type DashboardOverview,
  type DashboardSelection,
  type DashboardSelectionDetail,
  type DisplayRecord,
  type FiveStarRecord,
  type DoctorReport,
  type ForkResultMark,
  type ImportReport,
  type ItemKind,
  type PityBadge,
  type PoolKind,
  type RecordFilter,
  type RecordFilterOptions,
  type RateUpResult,
  type RollBucket,
  type SortDirection,
  type Profile,
  type RestoreReport,
  type Settings,
  type UpdateCheckReport,
  type UpdateStageReport,
  type UpdateStatus,
} from "../api";

import { createAssetTools } from "./assets";
import { createCaptureActions } from "./captureActions";
import { createChartTools } from "./chart";
import { createAppComputed } from "./computed";
import { createDashboardActions } from "./dashboardActions";
import { createDataOperations, type DataOperationKind } from "./dataOperations";
import { createTranslator, uiLocaleDisplayName } from "./i18n";
import { createMaintenanceActions } from "./maintenance";
import { navItems, type ViewId } from "./navigation";
import { kindOrder, type ExportMode, type ImportMode, type PoolKindFilter } from "./options";
import { defaultRecordViewPrefs, forkPityBadgeOptions, forkResultMarkOptions, isRecordRarity, rateUpResultOptions, readRecordViewPrefs, recordColumnGridTracks, recordColumnIds, recordPageSizes, recordPrefsKey, type FiveStarWallMode, type RecordColumnId, type RecordPageSize, type RecordViewPrefs } from "./recordPrefs";
import { createProfileActions } from "./profileActions";
import { rarityClass } from "./rarityColors";
import { dashboardRaritySlices } from "./rarityBuckets";
import { createTaskRunner } from "./task";
import { bannerMeta, bannerTitle, captureRecordMeta, captureRecordName, forkHitBadge, forkWinRate, formatBannerWindow, formatCaptureMode, formatCaptureState, formatError, formatForkResultMark, formatItemKind, formatPity, formatPityBadge, formatPityBadgeValue, formatPoolKindPullNo, formatPullNo, formatQuantityName, formatRecordResultBadge, formatResult, formatRollBucket, formatRolls, formatTenPullProgress, formatTenPullProgressSummary, formatTime, isHitBadgeLabel, numberOrDash, percent, primaryRecordBadge } from "./viewHelpers";

use([PieChart, TooltipComponent, CanvasRenderer]);

const rankingRarities = [3, 4, 5] as const;

type RankingRarity = (typeof rankingRarities)[number];
type RankingRaritySelection = Record<RankingRarity, boolean>;

function defaultRankingRaritySelection(): RankingRaritySelection {
  return { 3: true, 4: true, 5: true };
}

function isRankingRarity(value?: number | null): value is RankingRarity {
  return value === 3 || value === 4 || value === 5;
}

export function useApp() {
  const activeView = ref<ViewId>("dashboard"), profiles = ref<Profile[]>([]), activeProfileName = ref("default"), newProfileName = ref("");
  const profileRenameSource = ref(""), profileRenameName = ref(""), profileDeleteTarget = ref("");
  const locale = ref("en"), uiLocale = ref("en"), locales = ref<string[]>(["en"]), uiLocales = ref<string[]>(["en"]), summary = ref<DashboardOverview | null>(null);
  const selectedPoolKind = ref<PoolKind>("monopoly_limited"), selectedDashboardScope = ref<DashboardSelection>({ kind: "pool_kind", pool_kind: "monopoly_limited" }), detail = ref<DashboardSelectionDetail | null>(null), detailLoading = ref(false);
  const records = ref<DisplayRecord[]>([]), recordTotal = ref(0), filterOptions = ref<RecordFilterOptions>({ banners: [], roll_buckets: [], item_kinds: [] });
  const importPath = ref(""), importMode = ref<ImportMode>("raw"), exportPath = ref(""), exportMode = ref<ExportMode>("json");
  const backupPath = ref(""), restorePath = ref(""), captureMode = ref<CaptureMode>("live_only");
  const lastReport = ref<ImportReport | null>(null), lastBackup = ref<BackupReport | null>(null), lastRestore = ref<RestoreReport | null>(null), doctorReport = ref<DoctorReport | null>(null);
  const lastDataOperation = ref<DataOperationKind | null>(null);
  const updateStatus = ref<UpdateStatus | null>(null), updateCheckReport = ref<UpdateCheckReport | null>(null), stagedUpdate = ref<UpdateStageReport | null>(null);
  const assetUrlCache = ref<Record<string, string>>({}), captureStatus = ref<CaptureStatus | null>(null), captureActionBusy = ref(false), capturePollInFlight = ref(false);
  const busy = ref(false), statusText = ref(""), errorText = ref(""), chartEl = ref<HTMLElement | null>(null);
  const rankingDialogOpen = ref(false);
  const t = createTranslator(uiLocale);
  statusText.value = t("status.ready");
  const kindLabels = {
    get monopoly_limited() {
      return t("kind.limited");
    },
    get monopoly_standard() {
      return t("kind.standard");
    },
    get fork_lottery() {
      return t("kind.fork");
    },
  } as Record<PoolKind, string>;
  const formatResultText = (value: string) => formatResult(value, t);
  const formatRecordResultBadgeText = (value: string) => formatRecordResultBadge(value, t);
  const bannerTitleText = (banner?: Parameters<typeof bannerTitle>[0]) => bannerTitle(banner, t);
  const bannerMetaText = (banner?: Parameters<typeof bannerMeta>[0]) => bannerMeta(banner);
  const formatBannerWindowText = (start?: string | null, end?: string | null) => formatBannerWindow(start, end, t);
  const formatPityText = (record: DisplayRecord) => formatPity(record);
  const formatTenPullProgressText = (record: DisplayRecord) => formatTenPullProgress(record);
  const formatPityBadgeText = (record: DisplayRecord) => formatPityBadge(record, t);
  const primaryRecordBadgeText = (record: DisplayRecord) => primaryRecordBadge(record, t);
  const formatCaptureStateText = (value?: string | null) => formatCaptureState(value, t);
  const formatCaptureModeText = (value?: string | null) => formatCaptureMode(value, t);
  const uiLocaleName = (value: string) => uiLocaleDisplayName(value, t);

  function setChartEl(element: unknown) {
    chartEl.value = element instanceof HTMLElement ? element : null;
  }
  const { renderChart, disposeChart } = createChartTools(chartEl, detail, t);

  const recordPoolKind = ref<PoolKindFilter>("all"), recordBannerIds = ref<string[]>([]), itemRarities = ref<number[]>([]);
  const focusedRarities = ref<number[]>([]), rateUpResults = ref<RateUpResult[]>([]), rollBuckets = ref<RollBucket[]>([]), itemKinds = ref<ItemKind[]>([]);
  const forkResultMarks = ref<ForkResultMark[]>([]), forkPityBadges = ref<PityBadge[]>([]);
  const dateFrom = ref(""), dateTo = ref(""), search = ref("");
  const sortDirection = ref<SortDirection>("desc"), pageSize = ref<number>(defaultRecordViewPrefs.pageSize), pageIndex = ref(0);
  const recordPageJumpOpen = ref(false), recordPageJumpInput = ref("1");
  const visibleRecordColumns = ref<RecordColumnId[]>([...defaultRecordViewPrefs.visibleRecordColumns]);
  const recordAdvancedFiltersOpen = ref(false);
  const latestFiveStarWallModes = ref<Record<PoolKind, FiveStarWallMode>>({ ...defaultRecordViewPrefs.latestFiveStarWallModes });
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
  const settingsUpdateChannel = ref("stable"), settingsCheckUpdates = ref(false);
  const captureAutoPageEnabled = ref(true), captureFullUpdateEnabled = ref(false);
  const effectiveCaptureMode = computed<CaptureMode>(() => {
    if (!captureAutoPageEnabled.value) return "live_only";
    return captureFullUpdateEnabled.value ? "auto_page_full" : "auto_page_incremental";
  });
  let recordPrefsReady = false;
  let applyingRecordPrefs = false;
  let normalizingRecordFilters = false;
  let resettingRecordPage = false;
  const dataOperationSummary = computed(() => {
    if (lastDataOperation.value === "import" && lastReport.value) {
      return `${t("import.lastImport")} · ${lastReport.value.source_kind} · ${t("common.seen")} ${lastReport.value.records_seen} · ${t("common.inserted")} ${lastReport.value.records_inserted} · ${t("common.skipped")} ${lastReport.value.records_skipped}`;
    }
    if (lastDataOperation.value === "export") {
      const mode = exportMode.value === "json" ? t("import.publicJson") : "CSV";
      return `${t("import.export")} ${mode} · ${t("status.exportCompleted")}`;
    }
    if (lastDataOperation.value === "backup" && lastBackup.value) {
      return `${t("import.lastBackup")} · ${t("common.profiles")} ${lastBackup.value.profile_count} · ${t("common.records")} ${lastBackup.value.record_count}`;
    }
    if (lastDataOperation.value === "restore" && lastRestore.value) {
      return `${t("import.lastRestore")} · ${t("common.profiles")} ${lastRestore.value.profiles_seen} · ${t("common.created")} ${lastRestore.value.profiles_created} · ${t("common.merged")} ${lastRestore.value.profiles_merged} · ${t("common.inserted")} ${lastRestore.value.records_inserted} · ${t("common.skipped")} ${lastRestore.value.records_skipped}`;
    }
    return "";
  });
  const activeRecordFilterCount = computed(() =>
    [
      recordPoolKind.value !== "all",
      recordBannerIds.value.length > 0,
      itemRarities.value.length > 0,
      focusedRarities.value.length > 0,
      rateUpResults.value.length > 0,
      rollBuckets.value.length > 0,
      itemKinds.value.length > 0,
      forkResultMarks.value.length > 0,
      forkPityBadges.value.length > 0,
      Boolean(dateFrom.value),
      Boolean(dateTo.value),
      Boolean(search.value.trim()),
    ].filter(Boolean).length,
  );
  const recordColumnOptions = computed(() =>
    recordColumnIds.map((column) => ({
      value: column,
      label: recordColumnLabel(column),
    })),
  );
  const visibleRecordColumnSet = computed(() => new Set(visibleRecordColumns.value));
  const visibleRecordGridTemplate = computed(() =>
    visibleRecordColumns.value.map((column) => recordColumnGridTracks[column]).join(" ") || "none",
  );

  const {
    activeProfile,
    allPoolSummaries,
    bannerSummaries,
    selectedPoolBannerSummaries,
    selectedSummary: selectedPoolSummary,
    recordPageStart,
    recordPageEnd,
    recordPageCount,
    canPrevPage,
    canNextPage,
    canFirstPage,
    canLastPage,
    bannersForRecordKind,
    isCaptureActive,
    isWorkflowBusy,
    captureTitle,
    captureSubtitle,
    autoPageStatusLine,
    captureModeLabel,
  } = createAppComputed({
    profiles,
    activeProfileName,
    summary,
    selectedPoolKind,
    recordTotal,
    filterOptions,
    captureStatus,
    captureMode,
    busy,
    captureActionBusy,
    recordPoolKind,
    pageSize,
    pageIndex,
    t,
  });
  const selectedSummary = computed(() =>
    detail.value?.summary ?? (selectedDashboardScope.value.kind === "pool_kind" ? selectedPoolSummary.value : null),
  );
  const selectedScopeLabel = computed(() => {
    const scope = selectedDashboardScope.value;
    if (scope.kind === "banner") {
      return bannerSummaries.value.find((banner) => banner.banner_id === scope.banner_id)?.title;
    }
    return selectedPoolSummary.value?.label;
  });
  const isDashboardPoolScope = computed(() => selectedDashboardScope.value.kind === "pool_kind");
  const showDashboardBannerRail = computed(() => selectedPoolKind.value !== "monopoly_standard");
  const selectedDetailTitle = computed(() => {
    if (isDashboardPoolScope.value) return t("dashboard.poolDetail");
    const label = selectedScopeLabel.value?.trim();
    return label ? `${label} ${t("dashboard.detailSuffix")}` : t("dashboard.bannerDetail");
  });
  const hasItemRankingRows = computed(() => Boolean(detail.value?.item_ranking.length));
  const rankingRarityOptions = computed(() => {
    const selection = rankingRaritySelectionsByPoolKind.value[selectedPoolKind.value];
    return rankingRarities.map((rarity) => ({
      rarity,
      label: `${rarity}★`,
      active: selection[rarity],
      className: rarityClass(rarity),
    }));
  });
  const selectedRankingRarities = computed(() => new Set(
    rankingRarities.filter((rarity) => rankingRaritySelectionsByPoolKind.value[selectedPoolKind.value][rarity]),
  ));
  const itemRankingShares = computed(() => {
    const selectedRarities = selectedRankingRarities.value;
    const ranking = (detail.value?.item_ranking ?? []).filter((item) => isRankingRarity(item.rarity) && selectedRarities.has(item.rarity));
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
  const rankingDialogTitle = computed(() => `${selectedDetailTitle.value} · ${t("dashboard.itemRanking")}`);
  const selectedRarityShares = computed(() => dashboardRaritySlices(detail.value, t));
  const latestFiveStarWallMode = computed(() => latestFiveStarWallModeForPool(selectedPoolKind.value));
  const showLatestFiveStarWallModeToggle = computed(() => true);
  const visibleLatestFiveStarHits = computed(() => visibleFiveStarHits(detail.value));
  const displayedLatestFiveStarHits = computed(() => visibleLatestFiveStarHits.value);
  const fiveWallExpanded = computed(() => Boolean(fiveWallExpandedByPoolKind.value[selectedPoolKind.value]));
  const latestFiveStarEmptyText = computed(() => t("dashboard.fiveStarRecordsEmpty"));
  const recordBannerOptions = computed(() =>
    bannersForRecordKind.value.map((banner) => ({
      value: banner.banner_id,
      label: banner.title,
      meta: String(banner.count),
    })),
  );
  const focusedRarityOptions = computed(() =>
    [5, 4, 3].map((rarity) => ({
      value: rarity,
      label: `${rarity}★`,
    })),
  );
  const itemRarityOptions = computed(() =>
    [5, 4, 3].map((rarity) => ({
      value: rarity,
      label: `${rarity}★`,
    })),
  );
  const rateUpResultSelectOptions = computed(() =>
    rateUpResultOptions.map((result) => ({
      value: result,
      label: formatResultText(result),
    })),
  );
  const rollBucketOptions = computed(() =>
    filterOptions.value.roll_buckets.map((bucket) => ({
      value: bucket.bucket,
      label: formatRollBucket(bucket.bucket, t),
      meta: String(bucket.count),
    })),
  );
  const itemKindOptions = computed(() =>
    filterOptions.value.item_kinds.map((itemKind) => ({
      value: itemKind.item_kind,
      label: formatItemKind(itemKind.item_kind, t),
      meta: String(itemKind.count),
    })),
  );
  const showForkRecordFilters = computed(() => recordPoolKind.value === "all" || recordPoolKind.value === "fork_lottery");
  const forkResultMarkSelectOptions = computed(() =>
    forkResultMarkOptions.map((mark) => ({
      value: mark,
      label: formatForkResultMark(mark, t),
    })),
  );
  const forkPityBadgeSelectOptions = computed(() =>
    forkPityBadgeOptions.map((badge) => ({
      value: badge,
      label: formatPityBadgeValue(badge, t),
    })),
  );
  const runTask = createTaskRunner({ busy, statusText, errorText, formatError });

  function applySettings(settings: Settings) {
    setActiveProfileName(settings.active_profile);
    locale.value = settings.locale;
    uiLocale.value = settings.ui_locale || uiLocale.value;
    settingsUpdateChannel.value = settings.update_channel;
    settingsCheckUpdates.value = settings.check_updates_on_startup;
    captureAutoPageEnabled.value = settings.capture_auto_page_enabled;
    captureFullUpdateEnabled.value = settings.capture_auto_page_enabled && settings.capture_full_update_enabled;
    captureMode.value = effectiveCaptureMode.value;
  }

  async function saveCaptureSettings() {
    try {
      const settings = await api.updateSettings({
        capture_auto_page_enabled: captureAutoPageEnabled.value,
        capture_full_update_enabled: captureFullUpdateEnabled.value,
      });
      applySettings(settings);
    } catch (error) {
      errorText.value = formatError(error);
    }
  }

  function setCaptureAutoPageEnabled(value: boolean) {
    captureAutoPageEnabled.value = value;
    if (!value) captureFullUpdateEnabled.value = false;
    captureMode.value = effectiveCaptureMode.value;
    void saveCaptureSettings();
  }

  function setCaptureFullUpdateEnabled(value: boolean) {
    captureFullUpdateEnabled.value = value;
    if (value) captureAutoPageEnabled.value = true;
    captureMode.value = effectiveCaptureMode.value;
    void saveCaptureSettings();
  }

  async function startPreferredCapture() {
    captureMode.value = effectiveCaptureMode.value;
    await startLiveCapture();
  }

  const {
    itemVisualUrl,
    bannerVisualUrl,
    hasRecordVisual,
    hasItemVisual,
    hasBannerVisual,
    recordsHaveAnyVisual,
    resolveVisibleAssets,
  } = createAssetTools({
    assetUrlCache,
    bannerSummaries,
    records,
    detail,
    errorText,
    formatError,
  });
  const {
    normalizeDashboardScope,
    selectDashboardPool,
    selectDashboardBanner,
    isSelectedDashboardPool,
    isSelectedDashboardBanner,
    loadDetail,
  } = createDashboardActions({
    activeProfileName,
    locale,
    summary,
    selectedPoolKind,
    selectedDashboardScope,
    detail,
    detailLoading,
    errorText,
    formatError,
    resolveVisibleAssets,
  });
  const maintenance = createMaintenanceActions({
    doctorReport, updateStatus, updateCheckReport, stagedUpdate, settingsUpdateChannel, statusText, runTask, t,
  });
  const { runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate } = maintenance;

  function currentRecordViewPrefs(): RecordViewPrefs {
    const normalizedPageSize = recordPageSizes.includes(pageSize.value as RecordPageSize)
      ? (pageSize.value as RecordPageSize)
      : defaultRecordViewPrefs.pageSize;
    return {
      recordPoolKind: recordPoolKind.value,
      recordBannerIds: [...recordBannerIds.value],
      itemRarities: [...itemRarities.value],
      focusedRarities: [...focusedRarities.value],
      rateUpResults: [...rateUpResults.value],
      rollBuckets: [...rollBuckets.value],
      itemKinds: [...itemKinds.value],
      forkResultMarks: [...forkResultMarks.value],
      forkPityBadges: [...forkPityBadges.value],
      dateFrom: dateFrom.value,
      dateTo: dateTo.value,
      search: search.value,
      sortDirection: sortDirection.value,
      pageSize: normalizedPageSize,
      visibleRecordColumns: [...visibleRecordColumns.value],
      recordAdvancedFiltersOpen: recordAdvancedFiltersOpen.value,
      latestFiveStarWallModes: { ...latestFiveStarWallModes.value },
    };
  }

  function saveRecordViewPrefs(profileName = activeProfileName.value) {
    if (!recordPrefsReady || !profileName) return;
    try {
      window.localStorage.setItem(recordPrefsKey(profileName), JSON.stringify(currentRecordViewPrefs()));
    } catch {
      // localStorage may be unavailable in restricted runtimes; record view still works.
    }
  }

  function setActiveProfileName(profileName: string) {
    if (activeProfileName.value !== profileName) {
      recordPrefsReady = false;
    }
    activeProfileName.value = profileName;
  }

  function copyRecordViewPrefs(sourceProfileName: string, targetProfileName: string) {
    if (!sourceProfileName || !targetProfileName || sourceProfileName === targetProfileName) return;
    try {
      const raw = window.localStorage.getItem(recordPrefsKey(sourceProfileName));
      if (raw) window.localStorage.setItem(recordPrefsKey(targetProfileName), raw);
    } catch {
      // Best-effort UI preference migration.
    }
  }

  function removeRecordViewPrefs(profileName: string) {
    if (!profileName) return;
    try {
      window.localStorage.removeItem(recordPrefsKey(profileName));
    } catch {
      // Best-effort cleanup only.
    }
  }

  function applyRecordViewPrefs(profileName = activeProfileName.value) {
    const prefs = readRecordViewPrefs(profileName);
    applyingRecordPrefs = true;
    try {
      recordPoolKind.value = prefs.recordPoolKind;
      recordBannerIds.value = [...prefs.recordBannerIds];
      itemRarities.value = [...prefs.itemRarities];
      focusedRarities.value = [...prefs.focusedRarities];
      rateUpResults.value = [...prefs.rateUpResults];
      rollBuckets.value = [...prefs.rollBuckets];
      itemKinds.value = [...prefs.itemKinds];
      forkResultMarks.value = [...prefs.forkResultMarks];
      forkPityBadges.value = [...prefs.forkPityBadges];
      dateFrom.value = prefs.dateFrom;
      dateTo.value = prefs.dateTo;
      search.value = prefs.search;
      sortDirection.value = prefs.sortDirection;
      pageSize.value = prefs.pageSize;
      visibleRecordColumns.value = [...prefs.visibleRecordColumns];
      recordAdvancedFiltersOpen.value = prefs.recordAdvancedFiltersOpen;
      latestFiveStarWallModes.value = { ...prefs.latestFiveStarWallModes };
      pageIndex.value = 0;
      normalizeRecordFilterSelection();
    } finally {
      applyingRecordPrefs = false;
    }
    recordPrefsReady = true;
    saveRecordViewPrefs(profileName);
  }

  function normalizeRecordFilterSelection() {
    const availableBannerIds = new Set(bannersForRecordKind.value.map((banner) => banner.banner_id));
    const availableRollBuckets = new Set(filterOptions.value.roll_buckets.map((bucket) => bucket.bucket));
    const availableItemKinds = new Set(filterOptions.value.item_kinds.map((itemKind) => itemKind.item_kind));
    recordBannerIds.value = recordBannerIds.value.filter((bannerId) => availableBannerIds.has(bannerId));
    itemRarities.value = itemRarities.value.filter(isRecordRarity);
    focusedRarities.value = focusedRarities.value.filter(isRecordRarity);
    rateUpResults.value = rateUpResults.value.filter((result) => rateUpResultOptions.includes(result));
    rollBuckets.value = rollBuckets.value.filter((bucket) => availableRollBuckets.has(bucket));
    itemKinds.value = itemKinds.value.filter((itemKind) => availableItemKinds.has(itemKind));
    if (recordPoolKind.value === "monopoly_limited" || recordPoolKind.value === "monopoly_standard") {
      forkResultMarks.value = [];
      forkPityBadges.value = [];
    } else {
      forkResultMarks.value = forkResultMarks.value.filter((mark) => forkResultMarkOptions.includes(mark));
      forkPityBadges.value = forkPityBadges.value.filter((badge) => forkPityBadgeOptions.includes(badge));
    }
    if (!recordPageSizes.includes(pageSize.value as RecordPageSize)) pageSize.value = defaultRecordViewPrefs.pageSize;
    visibleRecordColumns.value = recordColumnIds.filter((column) => visibleRecordColumns.value.includes(column));
  }

  function recordColumnLabel(column: RecordColumnId) {
    switch (column) {
      case "index":
        return "#";
      case "time":
        return t("common.time");
      case "banner":
        return t("common.banner");
      case "item":
        return t("common.item");
      case "rarity":
        return t("dashboard.rarity");
      case "pullNo":
        return t("records.pullNo");
      case "fiveStarProgress":
        return t("records.fiveStarProgress");
      case "tenPullProgress":
        return t("records.tenPullProgress");
      case "rolls":
        return t("records.rolls");
    }
  }

  function isRecordColumnVisible(column: RecordColumnId) {
    return visibleRecordColumnSet.value.has(column);
  }

  function goToRecordPage(pageNumber: number) {
    if (recordPageCount.value === 0) return;
    if (!Number.isFinite(pageNumber)) return;
    const target = Math.trunc(pageNumber);
    const clamped = Math.min(Math.max(target, 1), recordPageCount.value);
    pageIndex.value = clamped - 1;
  }

  function goToFirstRecordPage() {
    goToRecordPage(1);
  }

  function goToLastRecordPage() {
    goToRecordPage(recordPageCount.value);
  }

  function openRecordPageJump() {
    if (recordPageCount.value === 0 || isWorkflowBusy.value) return;
    recordPageJumpInput.value = String(pageIndex.value + 1);
    recordPageJumpOpen.value = true;
  }

  function closeRecordPageJump() {
    recordPageJumpOpen.value = false;
  }

  function confirmRecordPageJump() {
    const value = Number.parseInt(recordPageJumpInput.value, 10);
    if (Number.isFinite(value)) goToRecordPage(value);
    closeRecordPageJump();
  }

  function currentRecordFilter(): RecordFilter {
    return {
      pool_kind: recordPoolKind.value === "all" ? null : recordPoolKind.value,
      banner_ids: recordBannerIds.value,
      rarities: itemRarities.value,
      focused_rarities: focusedRarities.value,
      rate_up_results: rateUpResults.value,
      roll_buckets: rollBuckets.value,
      item_kinds: itemKinds.value,
      fork_result_marks: forkResultMarks.value,
      fork_pity_badges: forkPityBadges.value,
      date_from: dateFrom.value || null,
      date_to: dateTo.value || null,
      search: search.value || null,
      sort_direction: sortDirection.value,
      limit: pageSize.value,
      offset: pageIndex.value * pageSize.value,
    };
  }

  function recordFilterKey(filter: RecordFilter) {
    return JSON.stringify(filter);
  }

  function isSameDashboardScope(left: DashboardSelection, right: DashboardSelection) {
    if (left.kind !== right.kind || left.pool_kind !== right.pool_kind) return false;
    if (left.kind === "pool_kind" || right.kind === "pool_kind") return true;
    return left.banner_id === right.banner_id;
  }

  const {
    loadProfiles,
    createProfile,
    startRenameProfile,
    cancelRenameProfile,
    saveProfileRename,
    requestDeleteProfile,
    cancelDeleteProfile,
    confirmDeleteProfile,
    selectProfile,
  } = createProfileActions({
    profiles,
    activeProfileName,
    newProfileName,
    profileRenameSource,
    profileRenameName,
    profileDeleteTarget,
    applySettings,
    runTask,
    t,
    saveRecordViewPrefs,
    setActiveProfileName,
    copyRecordViewPrefs,
    removeRecordViewPrefs,
    refreshAll,
  });

  const {
    startPendingAdminCapture,
    startLiveCapture,
    startFullCapture,
    stopLiveCapture,
    pollCaptureStatus,
    applyCaptureStatus,
    ensureCapturePolling,
    clearCapturePolling,
  } = createCaptureActions({
    activeProfileName,
    locale,
    captureMode,
    captureStatus,
    captureActionBusy,
    capturePollInFlight,
    lastReport,
    statusText,
    errorText,
    isCaptureActive,
    isWorkflowBusy,
    t,
    formatError,
    formatCaptureState: formatCaptureStateText,
    formatCaptureMode: formatCaptureModeText,
    saveRecordViewPrefs,
    setActiveProfileName,
    refreshAll,
  });

  const {
    pickImportFile,
    runImport,
    pickExportFile,
    runExport,
    pickBackupFile,
    runBackup,
    pickRestoreFile,
    runRestore,
  } = createDataOperations({
    activeProfileName,
    locale,
    importPath,
    importMode,
    exportPath,
    exportMode,
    backupPath,
    restorePath,
    lastReport,
    lastBackup,
    lastRestore,
    lastDataOperation,
    applySettings,
    runTask,
    t,
    saveRecordViewPrefs,
    loadProfiles,
    refreshAll,
  });

  onMounted(async () => {
    await bootstrap();
  });

  onBeforeUnmount(() => {
    clearCapturePolling();
    disposeChart();
  });

  watch(chartEl, async (element) => {
    if (!element) {
      disposeChart();
      return;
    }
    await nextTick();
    renderChart();
  });
  watch(detail, async () => { await nextTick(); renderChart(); }, { deep: true });
  watch(uiLocale, async () => { await nextTick(); renderChart(); });
  watch(() => selectedDashboardScope.value, () => { rankingDialogOpen.value = false; }, { deep: true });
  watch([recordPoolKind, recordBannerIds, itemRarities, focusedRarities, rateUpResults, rollBuckets, itemKinds, forkResultMarks, forkPityBadges, dateFrom, dateTo, search, sortDirection, pageSize], () => {
    if (applyingRecordPrefs || normalizingRecordFilters) return;
    normalizingRecordFilters = true;
    try {
      normalizeRecordFilterSelection();
    } finally {
      normalizingRecordFilters = false;
    }
    resettingRecordPage = true;
    try {
      pageIndex.value = 0;
    } finally {
      resettingRecordPage = false;
    }
    saveRecordViewPrefs();
    void loadRecords();
  }, { flush: "sync" });
  watch(pageIndex, () => {
    if (applyingRecordPrefs || resettingRecordPage) return;
    void loadRecords();
  }, { flush: "sync" });
  watch(recordAdvancedFiltersOpen, () => {
    if (applyingRecordPrefs) return;
    saveRecordViewPrefs();
  }, { flush: "sync" });
  watch(visibleRecordColumns, () => {
    if (applyingRecordPrefs || normalizingRecordFilters) return;
    normalizingRecordFilters = true;
    try {
      normalizeRecordFilterSelection();
    } finally {
      normalizingRecordFilters = false;
    }
    saveRecordViewPrefs();
  }, { flush: "sync" });
  async function bootstrap() {
    busy.value = true;
    try {
      const [settings, maps, uiLocaleList] = await Promise.all([api.getSettings(), api.mapsList(), api.uiLocaleList()]);
      applySettings(settings);
      locales.value = maps.locales;
      uiLocales.value = uiLocaleList.locales;
      await loadProfiles();
      await refreshAll();
      await loadUpdaterStatus();
      const startedPendingCapture = await startPendingAdminCapture();
      if (!startedPendingCapture && settings.check_updates_on_startup) {
        void checkForUpdates(false);
      }
    } catch (error) {
      errorText.value = formatError(error);
    } finally {
      busy.value = false;
    }
  }

  async function saveSettings() {
    await runTask(t("status.settingsUpdated"), async () => {
      const settings = await api.updateSettings({
        locale: locale.value,
        ui_locale: uiLocale.value,
        update_channel: settingsUpdateChannel.value,
        check_updates_on_startup: settingsCheckUpdates.value,
        capture_auto_page_enabled: captureAutoPageEnabled.value,
        capture_full_update_enabled: captureFullUpdateEnabled.value,
      });
      saveRecordViewPrefs();
      applySettings(settings);
      await loadProfiles();
      await refreshAll();
    });
  }

  async function refreshAll() {
    if (!activeProfileName.value) return;
    const requestedScope = selectedDashboardScope.value;
    const view = await api.profileAnalysisView(activeProfileName.value, requestedScope, currentRecordFilter(), locale.value);
    summary.value = view.overview;
    detail.value = view.selected_detail;
    detailLoading.value = false;
    filterOptions.value = view.record_filter_options;
    records.value = view.record_page.records;
    recordTotal.value = view.record_page.total;
    const firstActive = summary.value.pool_kinds.find((pool) => pool.total_pulls > 0)?.pool_kind;
    normalizeDashboardScope(firstActive);
    const scopeChanged = !isSameDashboardScope(requestedScope, selectedDashboardScope.value);
    const beforeFilter = recordFilterKey(currentRecordFilter());
    if (!recordPrefsReady) {
      applyRecordViewPrefs();
    } else {
      normalizeRecordFilterSelection();
    }
    const filterChanged = recordFilterKey(currentRecordFilter()) !== beforeFilter;
    await Promise.all([scopeChanged ? loadDetail() : Promise.resolve(), filterChanged ? loadRecords() : Promise.resolve()]);
    statusText.value = t("status.dashboardUpdated");
    await resolveVisibleAssets();
    await nextTick();
    renderChart();
  }

  async function loadFilterOptions() {
    if (!activeProfileName.value) return;
    filterOptions.value = await api.recordFilterOptions(activeProfileName.value, locale.value);
  }

  async function loadRecords() {
    if (!activeProfileName.value) return;
    const result = await api.recordPage(activeProfileName.value, currentRecordFilter(), locale.value);
    records.value = result.records;
    recordTotal.value = result.total;
    await resolveVisibleAssets();
  }

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
    return poolKind ? (latestFiveStarWallModes.value[poolKind] ?? "all") : "all";
  }

  function toggleLatestFiveStarWallMode() {
    const mode = latestFiveStarWallMode.value === "all" ? "focused" : "all";
    latestFiveStarWallModes.value = {
      ...latestFiveStarWallModes.value,
      [selectedPoolKind.value]: mode,
    };
    saveRecordViewPrefs();
  }

  function latestFiveStarWallToggleLabel() {
    if (selectedPoolKind.value === "fork_lottery") {
      return latestFiveStarWallMode.value === "all" ? t("dashboard.allFiveStar") : t("dashboard.upFiveStarOnly");
    }
    return latestFiveStarWallMode.value === "all" ? t("dashboard.showingFiveStarItems") : t("dashboard.hidingFiveStarItems");
  }

  function toggleFiveWallExpanded() {
    fiveWallExpandedByPoolKind.value = {
      ...fiveWallExpandedByPoolKind.value,
      [selectedPoolKind.value]: !fiveWallExpanded.value,
    };
  }

  function showDashboardFiveStarRecords() {
    const scope = selectedDashboardScope.value;
    const mode = latestFiveStarWallModeForPool(scope.pool_kind);
    applyingRecordPrefs = true;
    try {
      recordPoolKind.value = scope.pool_kind;
      recordBannerIds.value = scope.kind === "banner" ? [scope.banner_id] : [];
      itemRarities.value = mode === "all" ? [5] : [];
      focusedRarities.value = mode === "focused" ? [5] : [];
      rateUpResults.value = [];
      rollBuckets.value = [];
      itemKinds.value = [];
      forkResultMarks.value = [];
      forkPityBadges.value = [];
      dateFrom.value = "";
      dateTo.value = "";
      search.value = "";
      recordAdvancedFiltersOpen.value = false;
      pageIndex.value = 0;
      activeView.value = "records";
    } finally {
      applyingRecordPrefs = false;
    }
    normalizeRecordFilterSelection();
    saveRecordViewPrefs();
    void loadRecords();
  }

  function summaryProgressLabel(summary?: { pool_kind?: PoolKind } | null) {
    return summary?.pool_kind === "fork_lottery" ? t("dashboard.fourStarGuarantee") : t("dashboard.giftProgress");
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
    const selection = rankingRaritySelectionsByPoolKind.value[selectedPoolKind.value];
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

  function resetRecordFilters() {
    applyingRecordPrefs = true;
    try {
      recordPoolKind.value = "all";
      recordBannerIds.value = [];
      itemRarities.value = [];
      focusedRarities.value = [];
      rateUpResults.value = [];
      rollBuckets.value = [];
      itemKinds.value = [];
      forkResultMarks.value = [];
      forkPityBadges.value = [];
      dateFrom.value = "";
      dateTo.value = "";
      search.value = "";
      pageIndex.value = 0;
    } finally {
      applyingRecordPrefs = false;
    }
    saveRecordViewPrefs();
    void loadRecords();
  }

  return reactive({
    t, uiLocaleName, navItems, kindOrder, kindLabels, activeView, profiles, activeProfileName, newProfileName, profileRenameSource, profileRenameName, profileDeleteTarget, locale, uiLocale, locales, uiLocales, summary, selectedPoolKind, selectedDashboardScope, detail, detailLoading, records, recordTotal, filterOptions, importPath, importMode,
    exportPath, exportMode, backupPath, restorePath, captureMode, captureAutoPageEnabled, captureFullUpdateEnabled, effectiveCaptureMode, lastReport, lastBackup, lastRestore, doctorReport, updateStatus, updateCheckReport, stagedUpdate, assetUrlCache, captureStatus, captureActionBusy,
    capturePollInFlight, busy, statusText, errorText, setChartEl, recordPoolKind, recordBannerIds, itemRarities, focusedRarities, rateUpResults, rollBuckets, itemKinds, forkResultMarks, forkPityBadges, dateFrom, dateTo, search,
    sortDirection, pageSize, pageIndex, recordPageJumpOpen, recordPageJumpInput, visibleRecordColumns, recordColumnOptions, visibleRecordGridTemplate, isRecordColumnVisible, recordPageSizes, recordAdvancedFiltersOpen, latestFiveStarWallMode, activeRecordFilterCount, recordBannerOptions, itemRarityOptions, focusedRarityOptions, rateUpResultSelectOptions, rollBucketOptions, itemKindOptions, showForkRecordFilters, forkResultMarkSelectOptions, forkPityBadgeSelectOptions, settingsUpdateChannel, settingsCheckUpdates, dataOperationSummary, activeProfile, allPoolSummaries, bannerSummaries, selectedPoolBannerSummaries, selectedSummary, selectedScopeLabel, isDashboardPoolScope, selectedDetailTitle, hasItemRankingRows, rankingRarityOptions, itemRankingShares, recordPageStart, recordPageEnd, recordPageCount, canPrevPage,
    canNextPage, canFirstPage, canLastPage, bannersForRecordKind, isCaptureActive, isWorkflowBusy, captureTitle, captureSubtitle, autoPageStatusLine, captureModeLabel, showDashboardBannerRail, showLatestFiveStarWallModeToggle, visibleLatestFiveStarHits, displayedLatestFiveStarHits, fiveWallExpanded, latestFiveStarEmptyText, rankingDialogOpen, rankingDialogTitle, bootstrap, startPendingAdminCapture, loadProfiles, createProfile, startRenameProfile, cancelRenameProfile, saveProfileRename, requestDeleteProfile, cancelDeleteProfile, confirmDeleteProfile, selectProfile, saveSettings, refreshAll, selectDashboardPool, selectDashboardBanner, isSelectedDashboardPool, isSelectedDashboardBanner, loadDetail,
    loadFilterOptions, loadRecords, resetRecordFilters, pickImportFile, runImport, startPreferredCapture, startLiveCapture, startFullCapture, setCaptureAutoPageEnabled, setCaptureFullUpdateEnabled, stopLiveCapture, pollCaptureStatus, applyCaptureStatus, ensureCapturePolling, clearCapturePolling, pickExportFile, runExport, pickBackupFile, runBackup, pickRestoreFile, runRestore,
    runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate, runTask, renderChart, goToRecordPage, goToFirstRecordPage, goToLastRecordPage, openRecordPageJump, closeRecordPageJump, confirmRecordPageJump, percent, numberOrDash, formatTime, formatResult: formatResultText, bannerTitle: bannerTitleText, bannerMeta: bannerMetaText,
    formatBannerWindow: formatBannerWindowText, formatPullNo, formatPoolKindPullNo, formatPity: formatPityText, formatPityRatio, formatTenPullProgress: formatTenPullProgressText, formatTenPullProgressSummary, formatPityBadge: formatPityBadgeText, formatRolls, formatQuantityName, formatRecordResultBadge: formatRecordResultBadgeText, primaryRecordBadge: primaryRecordBadgeText, isHitBadgeLabel, forkHitBadge, forkWinRate, summaryProgressLabel, pullCurrency, recordRarityClass, latestFiveStarForPool, latestFiveStarNameForPool, toggleLatestFiveStarWallMode, latestFiveStarWallToggleLabel, toggleFiveWallExpanded, toggleRankingRarity, fiveWallPityTone, fiveWallDistance, showDashboardFiveStarRecords, selectedRarityShares, itemVisualUrl, bannerVisualUrl, hasRecordVisual, hasItemVisual, hasBannerVisual, recordsHaveAnyVisual, resolveVisibleAssets, openRankingDialog, closeRankingDialog, formatCaptureState: formatCaptureStateText, formatCaptureMode: formatCaptureModeText, captureRecordName, captureRecordMeta, formatError,
  });
}

export type AppState = ReturnType<typeof useApp>;
