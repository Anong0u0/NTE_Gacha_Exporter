import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  api,
  type AssetsPackCheckReport,
  type AssetsPackInstallReport,
  type AssetsPackStatus,
  type BackupReport,
  type CaptureMode,
  type CaptureStatus,
  type DashboardOverview,
  type DashboardSelection,
  type DashboardSelectionDetail,
  type DisplayRecord,
  type DoctorReport,
  type ImportReport,
  type ItemKind,
  type PoolKind,
  type RecordFilter,
  type RecordFilterOptions,
  type RateUpResult,
  type RollBucket,
  type SortDirection,
  type Profile,
  type RestoreReport,
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
import { defaultRecordViewPrefs, isRecordRarity, rateUpResultOptions, readRecordViewPrefs, recordPageSizes, recordPrefsKey, type RecordPageSize, type RecordViewPrefs } from "./recordPrefs";
import { createProfileActions } from "./profileActions";
import { createTaskRunner } from "./task";
import { bannerMeta, bannerTitle, captureRecordMeta, captureRecordName, forkHitBadge, forkWinRate, formatBannerWindow, formatCaptureMode, formatCaptureState, formatError, formatGlobalPullNo, formatGuarantee, formatItemKind, formatPity, formatPullNo, formatRecordResultBadge, formatResult, formatRollBucket, formatRollGiftProgress, formatTime, numberOrDash, percent } from "./viewHelpers";

use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

export function useApp() {
  const activeView = ref<ViewId>("dashboard"), profiles = ref<Profile[]>([]), activeProfileName = ref("default"), newProfileName = ref("");
  const profileRenameSource = ref(""), profileRenameName = ref(""), profileDeleteTarget = ref("");
  const locale = ref("en"), uiLocale = ref("en"), locales = ref<string[]>(["en"]), uiLocales = ref<string[]>(["en"]), summary = ref<DashboardOverview | null>(null);
  const selectedPoolKind = ref<PoolKind>("monopoly_limited"), selectedDashboardScope = ref<DashboardSelection>({ kind: "pool_kind", pool_kind: "monopoly_limited" }), detail = ref<DashboardSelectionDetail | null>(null);
  const records = ref<DisplayRecord[]>([]), recordTotal = ref(0), filterOptions = ref<RecordFilterOptions>({ banners: [], roll_buckets: [], item_kinds: [] });
  const importPath = ref(""), importMode = ref<ImportMode>("raw"), exportPath = ref(""), exportMode = ref<ExportMode>("json");
  const backupPath = ref(""), restorePath = ref(""), captureMode = ref<CaptureMode>("live_only");
  const lastReport = ref<ImportReport | null>(null), lastBackup = ref<BackupReport | null>(null), lastRestore = ref<RestoreReport | null>(null), doctorReport = ref<DoctorReport | null>(null);
  const lastDataOperation = ref<DataOperationKind | null>(null);
  const updateStatus = ref<UpdateStatus | null>(null), updateCheckReport = ref<UpdateCheckReport | null>(null), stagedUpdate = ref<UpdateStageReport | null>(null);
  const assetsPackStatus = ref<AssetsPackStatus | null>(null), assetsPackCheckReport = ref<AssetsPackCheckReport | null>(null), lastAssetsPackInstall = ref<AssetsPackInstallReport | null>(null);
  const assetUrlCache = ref<Record<string, string>>({}), captureStatus = ref<CaptureStatus | null>(null), captureActionBusy = ref(false), capturePollInFlight = ref(false);
  const busy = ref(false), statusText = ref(""), errorText = ref(""), chartEl = ref<HTMLElement | null>(null);
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
  const formatRollGiftProgressText = (record: DisplayRecord) => formatRollGiftProgress(record);
  const formatGuaranteeText = (record: DisplayRecord) => formatGuarantee(record, t);
  const formatCaptureStateText = (value?: string | null) => formatCaptureState(value, t);
  const formatCaptureModeText = (value?: string | null) => formatCaptureMode(value, t);
  const uiLocaleName = (value: string) => uiLocaleDisplayName(value, t);

  function setChartEl(element: unknown) {
    chartEl.value = element instanceof HTMLElement ? element : null;
  }
  const { renderChart, disposeChart } = createChartTools(chartEl, detail);

  const recordPoolKind = ref<PoolKindFilter>("all"), recordBannerIds = ref<string[]>([]), itemRarities = ref<number[]>([]);
  const hitRarities = ref<number[]>([]), rateUpResults = ref<RateUpResult[]>([]), rollBuckets = ref<RollBucket[]>([]), itemKinds = ref<ItemKind[]>([]);
  const dateFrom = ref(""), dateTo = ref(""), search = ref("");
  const sortDirection = ref<SortDirection>("desc"), pageSize = ref<number>(defaultRecordViewPrefs.pageSize), pageIndex = ref(0);
  const recordAdvancedFiltersOpen = ref(false);
  const settingsUpdateChannel = ref("stable"), settingsCheckUpdates = ref(false);
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
      hitRarities.value.length > 0,
      rateUpResults.value.length > 0,
      rollBuckets.value.length > 0,
      itemKinds.value.length > 0,
      Boolean(dateFrom.value),
      Boolean(dateTo.value),
      Boolean(search.value.trim()),
    ].filter(Boolean).length,
  );

  const {
    activeProfile,
    allPoolSummaries,
    trackedPoolCount,
    bannerSummaries,
    selectedPoolBannerSummaries,
    trackedBannerCount,
    totalRollPoints,
    selectedSummary: selectedPoolSummary,
    recordPageStart,
    recordPageEnd,
    canPrevPage,
    canNextPage,
    bannersForRecordKind,
    isCaptureActive,
    isWorkflowBusy,
    captureTitle,
    captureSubtitle,
    autoPageStatusLine,
    captureModeLabel,
    assetsPackSummary,
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
    assetsPackStatus,
    recordPoolKind,
    pageSize,
    pageIndex,
    t,
  });
  const selectedSummary = computed(() => detail.value?.summary ?? selectedPoolSummary.value);
  const recordBannerOptions = computed(() =>
    bannersForRecordKind.value.map((banner) => ({
      value: banner.banner_id,
      label: banner.title,
      meta: String(banner.count),
    })),
  );
  const hitRarityOptions = computed(() =>
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
  const runTask = createTaskRunner({ busy, statusText, errorText, formatError });

  const {
    itemVisualUrl,
    bannerVisualUrl,
    hasRecordVisual,
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
    selectDashboardBannerById,
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
    resolveVisibleAssets,
  });
  const maintenance = createMaintenanceActions({
    doctorReport, updateStatus, updateCheckReport, stagedUpdate, assetsPackStatus, assetsPackCheckReport,
    lastAssetsPackInstall, assetUrlCache, settingsUpdateChannel, statusText, runTask, resolveVisibleAssets, t,
  });
  const { pingRuntime, runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate, loadAssetsPackStatus, checkAssetsPack, downloadAssetsPack, removeAssetsPack } = maintenance;

  function currentRecordViewPrefs(): RecordViewPrefs {
    const normalizedPageSize = recordPageSizes.includes(pageSize.value as RecordPageSize)
      ? (pageSize.value as RecordPageSize)
      : defaultRecordViewPrefs.pageSize;
    return {
      recordPoolKind: recordPoolKind.value,
      recordBannerIds: [...recordBannerIds.value],
      itemRarities: [...itemRarities.value],
      hitRarities: [...hitRarities.value],
      rateUpResults: [...rateUpResults.value],
      rollBuckets: [...rollBuckets.value],
      itemKinds: [...itemKinds.value],
      dateFrom: dateFrom.value,
      dateTo: dateTo.value,
      search: search.value,
      sortDirection: sortDirection.value,
      pageSize: normalizedPageSize,
      recordAdvancedFiltersOpen: recordAdvancedFiltersOpen.value,
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
      hitRarities.value = [...prefs.hitRarities];
      rateUpResults.value = [...prefs.rateUpResults];
      rollBuckets.value = [...prefs.rollBuckets];
      itemKinds.value = [...prefs.itemKinds];
      dateFrom.value = prefs.dateFrom;
      dateTo.value = prefs.dateTo;
      search.value = prefs.search;
      sortDirection.value = prefs.sortDirection;
      pageSize.value = prefs.pageSize;
      recordAdvancedFiltersOpen.value = prefs.recordAdvancedFiltersOpen;
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
    hitRarities.value = hitRarities.value.filter(isRecordRarity);
    rateUpResults.value = rateUpResults.value.filter((result) => rateUpResultOptions.includes(result));
    rollBuckets.value = rollBuckets.value.filter((bucket) => availableRollBuckets.has(bucket));
    itemKinds.value = itemKinds.value.filter((itemKind) => availableItemKinds.has(itemKind));
    if (!recordPageSizes.includes(pageSize.value as RecordPageSize)) pageSize.value = defaultRecordViewPrefs.pageSize;
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
    locale,
    uiLocale,
    settingsUpdateChannel,
    settingsCheckUpdates,
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
    uiLocale,
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
    settingsUpdateChannel,
    settingsCheckUpdates,
    runTask,
    t,
    saveRecordViewPrefs,
    setActiveProfileName,
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

  watch(() => detail.value?.rarity_distribution, async () => { await nextTick(); renderChart(); }, { deep: true });
  watch([recordPoolKind, recordBannerIds, itemRarities, hitRarities, rateUpResults, rollBuckets, itemKinds, dateFrom, dateTo, search, sortDirection, pageSize], () => {
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
  async function bootstrap() {
    busy.value = true;
    try {
      const [settings, maps] = await Promise.all([api.getSettings(), api.mapsList()]);
      locale.value = settings.locale;
      uiLocale.value = settings.ui_locale || "en";
      settingsUpdateChannel.value = settings.update_channel;
      settingsCheckUpdates.value = settings.check_updates_on_startup;
      locales.value = maps.locales;
      uiLocales.value = maps.locales;
      setActiveProfileName(settings.active_profile);
      await loadProfiles();
      await refreshAll();
      await loadUpdaterStatus();
      await loadAssetsPackStatus();
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
      });
      saveRecordViewPrefs();
      setActiveProfileName(settings.active_profile);
      locale.value = settings.locale;
      uiLocale.value = settings.ui_locale || uiLocale.value;
      settingsUpdateChannel.value = settings.update_channel;
      settingsCheckUpdates.value = settings.check_updates_on_startup;
      await loadProfiles();
      await refreshAll();
    });
  }

  async function refreshAll() {
    if (!activeProfileName.value) return;
    summary.value = await api.dashboardOverview(activeProfileName.value, locale.value);
    const firstActive = summary.value.pool_kinds.find((pool) => pool.total_pulls > 0)?.pool_kind;
    normalizeDashboardScope(firstActive);
    await Promise.all([loadDetail(), loadFilterOptions()]);
    if (!recordPrefsReady) applyRecordViewPrefs();
    else normalizeRecordFilterSelection();
    await loadRecords();
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
    const filter: RecordFilter = {
      pool_kind: recordPoolKind.value === "all" ? null : recordPoolKind.value,
      banner_ids: recordBannerIds.value,
      rarities: itemRarities.value,
      hit_rarities: hitRarities.value,
      rate_up_results: rateUpResults.value,
      roll_buckets: rollBuckets.value,
      item_kinds: itemKinds.value,
      date_from: dateFrom.value || null,
      date_to: dateTo.value || null,
      search: search.value || null,
      sort_direction: sortDirection.value,
      limit: pageSize.value,
      offset: pageIndex.value * pageSize.value,
    };
    const result = await api.listRecords(activeProfileName.value, filter, locale.value);
    records.value = result.records;
    recordTotal.value = result.total;
    await resolveVisibleAssets();
  }

  function resetRecordFilters() {
    applyingRecordPrefs = true;
    try {
      recordPoolKind.value = "all";
      recordBannerIds.value = [];
      itemRarities.value = [];
      hitRarities.value = [];
      rateUpResults.value = [];
      rollBuckets.value = [];
      itemKinds.value = [];
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
    t, uiLocaleName, navItems, kindOrder, kindLabels, activeView, profiles, activeProfileName, newProfileName, profileRenameSource, profileRenameName, profileDeleteTarget, locale, uiLocale, locales, uiLocales, summary, selectedPoolKind, selectedDashboardScope, detail, records, recordTotal, filterOptions, importPath, importMode,
    exportPath, exportMode, backupPath, restorePath, captureMode, lastReport, lastBackup, lastRestore, doctorReport, updateStatus, updateCheckReport, stagedUpdate, assetsPackStatus, assetsPackCheckReport, lastAssetsPackInstall, assetUrlCache, captureStatus, captureActionBusy,
    capturePollInFlight, busy, statusText, errorText, setChartEl, recordPoolKind, recordBannerIds, itemRarities, hitRarities, rateUpResults, rollBuckets, itemKinds, dateFrom, dateTo, search,
    sortDirection, pageSize, pageIndex, recordPageSizes, recordAdvancedFiltersOpen, activeRecordFilterCount, recordBannerOptions, itemRarityOptions, hitRarityOptions, rateUpResultSelectOptions, rollBucketOptions, itemKindOptions, settingsUpdateChannel, settingsCheckUpdates, dataOperationSummary, activeProfile, allPoolSummaries, trackedPoolCount, bannerSummaries, selectedPoolBannerSummaries, trackedBannerCount, totalRollPoints, selectedSummary, recordPageStart, recordPageEnd, canPrevPage,
    canNextPage, bannersForRecordKind, isCaptureActive, isWorkflowBusy, captureTitle, captureSubtitle, autoPageStatusLine, captureModeLabel, assetsPackSummary, bootstrap, startPendingAdminCapture, loadProfiles, createProfile, startRenameProfile, cancelRenameProfile, saveProfileRename, requestDeleteProfile, cancelDeleteProfile, confirmDeleteProfile, selectProfile, saveSettings, refreshAll, selectDashboardPool, selectDashboardBanner, selectDashboardBannerById, isSelectedDashboardPool, isSelectedDashboardBanner, loadDetail,
    loadFilterOptions, loadRecords, resetRecordFilters, pickImportFile, runImport, startLiveCapture, startFullCapture, stopLiveCapture, pollCaptureStatus, applyCaptureStatus, ensureCapturePolling, clearCapturePolling, pickExportFile, runExport, pickBackupFile, runBackup, pickRestoreFile, runRestore, pingRuntime,
    runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate, loadAssetsPackStatus, checkAssetsPack, downloadAssetsPack, removeAssetsPack, runTask, renderChart, percent, numberOrDash, formatTime, formatResult: formatResultText, bannerTitle: bannerTitleText, bannerMeta: bannerMetaText,
    formatBannerWindow: formatBannerWindowText, formatPullNo, formatGlobalPullNo, formatPity: formatPityText, formatRollGiftProgress: formatRollGiftProgressText, formatGuarantee: formatGuaranteeText, formatRecordResultBadge: formatRecordResultBadgeText, forkHitBadge, forkWinRate, itemVisualUrl, bannerVisualUrl, hasRecordVisual, hasBannerVisual, recordsHaveAnyVisual, resolveVisibleAssets, formatCaptureState: formatCaptureStateText, formatCaptureMode: formatCaptureModeText, captureRecordName, captureRecordMeta, formatError,
  });
}

export type AppState = ReturnType<typeof useApp>;
