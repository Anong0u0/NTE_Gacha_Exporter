import { computed, nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  api,
  type AboutLinkTarget,
  type BackupReport,
  type CaptureMode,
  type CaptureStatus,
  type DashboardOverview,
  type DashboardSelection,
  type DashboardSelectionDetail,
  type DisplayRecord,
  type DiagnosticStatus,
  type ImportReport,
  type PoolKind,
  type RecordFilterOptions,
  type Profile,
  type RestoreReport,
  type Settings,
  type SettingsPatch,
  type UpdateCheckReport,
  type UpdateStatus,
} from "../api";

import { createAssetTools } from "./assets";
import { createCaptureActions } from "./captureActions";
import { createChartTools } from "./chart";
import { createAppComputed } from "./computed";
import { createDashboardActions } from "./dashboardActions";
import { createDashboardUi } from "./dashboardUi";
import { createDataOperations, type DataOperationKind } from "./dataOperations";
import { createDiagnosticActions } from "./diagnosticActions";
import { installEcharts } from "./echarts";
import { createAppFormatters } from "./formatters";
import { createTranslator } from "./i18n";
import { createMaintenanceActions } from "./maintenance";
import { navItems, type ViewId } from "./navigation";
import { kindOrder, type ExportMode, type ImportMode } from "./options";
import { createProfileActions } from "./profileActions";
import { createRecordState } from "./recordState";
import { createTaskRunner } from "./task";

installEcharts();

export function useApp() {
  const activeView = ref<ViewId>("dashboard"), profiles = ref<Profile[]>([]), activeProfileName = ref("default"), newProfileName = ref("");
  const profileRenameSource = ref(""), profileRenameName = ref(""), profileDeleteTarget = ref("");
  const locale = ref("en"), uiLocale = ref("en"), locales = ref<string[]>(["en"]), uiLocales = ref<string[]>(["en"]), summary = ref<DashboardOverview | null>(null);
  const selectedPoolKind = ref<PoolKind>("monopoly_limited"), selectedDashboardScope = ref<DashboardSelection>({ kind: "pool_kind", pool_kind: "monopoly_limited" }), detail = ref<DashboardSelectionDetail | null>(null), detailLoading = ref(false);
  const records = ref<DisplayRecord[]>([]), recordTotal = ref(0), filterOptions = ref<RecordFilterOptions>({ banners: [], roll_buckets: [], item_kinds: [] });
  const importPath = ref(""), importMode = ref<ImportMode>("raw"), exportPath = ref(""), exportMode = ref<ExportMode>("json");
  const backupPath = ref(""), restorePath = ref(""), captureMode = ref<CaptureMode>("live_only");
  const lastReport = ref<ImportReport | null>(null), lastBackup = ref<BackupReport | null>(null), lastRestore = ref<RestoreReport | null>(null);
  const lastDataOperation = ref<DataOperationKind | null>(null);
  const updateStatus = ref<UpdateStatus | null>(null), updateCheckReport = ref<UpdateCheckReport | null>(null);
  const assetUrlCache = ref<Record<string, string>>({}), captureStatus = ref<CaptureStatus | null>(null), captureStalledDialogOpen = ref(false), captureActionBusy = ref(false), capturePollInFlight = ref(false);
  const diagnosticPromptOpen = ref(false), diagnosticStatus = ref<DiagnosticStatus | null>(null), diagnosticActionBusy = ref(false), diagnosticPollInFlight = ref(false);
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
  const formatters = createAppFormatters(t);

  function setChartEl(element: unknown) {
    chartEl.value = element instanceof HTMLElement ? element : null;
  }
  const { renderChart, disposeChart } = createChartTools(chartEl, detail, t);

  const recordState = createRecordState({
    activeProfileName,
    filterOptions,
    isWorkflowBusy: () => isWorkflowBusy.value,
    t,
  });
  const {
    recordPoolKind, recordBannerIds, itemRarities, focusedRarities, rateUpResults, rollBuckets,
    itemKinds, forkResultMarks, forkPityBadges, dateFrom, dateTo, search, sortDirection, pageSize,
    pageIndex, recordPageJumpOpen, recordPageJumpInput, visibleRecordColumns, recordAdvancedFiltersOpen,
    latestFiveStarWallModes,
  } = recordState.refs;
  const {
    activeRecordFilterCount, recordColumnOptions, visibleRecordGridTemplate, recordBannerOptions,
    itemRarityOptions, focusedRarityOptions, rateUpResultSelectOptions, rollBucketOptions,
    itemKindOptions, showForkRecordFilters, forkResultMarkSelectOptions, forkPityBadgeSelectOptions,
    recordPageSizes,
  } = recordState.computed;
  const {
    isRecordColumnVisible, goToRecordPage, goToFirstRecordPage, goToLastRecordPage, openRecordPageJump,
    closeRecordPageJump, confirmRecordPageJump,
  } = recordState.actions;
  const {
    currentRecordFilter, recordFilterKey, saveRecordViewPrefs, setActiveProfileName, copyRecordViewPrefs,
    removeRecordViewPrefs, applyRecordViewPrefs, normalizeRecordFilterSelection,
    withApplyingRecordPrefs, normalizeAfterFilterWatch, resetPageAfterFilterWatch,
    shouldSkipFilterWatch, shouldSkipPageWatch, shouldSkipPrefsWatch, shouldSkipColumnWatch,
    isRecordPrefsReady,
  } = recordState.internal;
  const settingsUpdateChannel = ref("stable"), settingsCheckUpdates = ref(false);
  const settingsSkippedUpdateVersion = ref<string | null>(null);
  const updatePromptOpen = ref(false), dismissedUpdateVersion = ref<string | null>(null);
  const captureAutoPageEnabled = ref(true), captureFullUpdateEnabled = ref(false);
  const effectiveCaptureMode = computed<CaptureMode>(() => {
    if (!captureAutoPageEnabled.value) return "live_only";
    return captureFullUpdateEnabled.value ? "auto_page_full" : "auto_page_incremental";
  });
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
    isDiagnosticActive,
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
    diagnosticStatus,
    busy,
    captureActionBusy,
    diagnosticActionBusy,
    recordPoolKind,
    pageSize,
    pageIndex,
    t,
  });
  recordState.internal.bindBannersForRecordKind(bannersForRecordKind);
  recordState.internal.bindRecordPageCount(recordPageCount);
  const dashboardUi = createDashboardUi({
    detail,
    selectedDashboardScope,
    selectedPoolKind,
    selectedPoolSummary,
    bannerSummaries,
    latestFiveStarWallModes,
    t,
    saveRecordViewPrefs,
  });
  const { rankingDialogOpen } = dashboardUi.refs;
  const {
    selectedSummary, selectedScopeLabel, isDashboardPoolScope, selectedDetailTitle, hasItemRankingRows,
    rankingRarityOptions, itemRankingShares, rankingDialogTitle, selectedRarityShares, latestFiveStarWallMode,
    showLatestFiveStarWallModeToggle, visibleLatestFiveStarHits, displayedLatestFiveStarHits, fiveWallExpanded,
    latestFiveStarEmptyText, showDashboardBannerRail,
  } = dashboardUi.computed;
  const {
    latestFiveStarForPool, latestFiveStarNameForPool, toggleLatestFiveStarWallMode, latestFiveStarWallToggleLabel,
    toggleFiveWallExpanded, toggleRankingRarity, fiveWallPityTone, fiveWallDistance, summaryProgressLabel,
    pullCurrency, formatPityRatio, recordRarityClass, openRankingDialog, closeRankingDialog,
  } = dashboardUi.actions;
  const { latestFiveStarWallModeForPool } = dashboardUi.internal;
  const runTask = createTaskRunner({ busy, statusText, errorText, formatError: formatters.formatError });

  function applySettings(settings: Settings) {
    setActiveProfileName(settings.active_profile);
    locale.value = settings.locale;
    uiLocale.value = settings.ui_locale || uiLocale.value;
    settingsUpdateChannel.value = settings.update_channel;
    settingsCheckUpdates.value = settings.check_updates_on_startup;
    settingsSkippedUpdateVersion.value = settings.skipped_update_version ?? null;
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
      errorText.value = formatters.formatError(error);
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

  function closeCaptureStalledDialog() {
    captureStalledDialogOpen.value = false;
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
    formatError: formatters.formatError,
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
    formatError: formatters.formatError,
    resolveVisibleAssets,
  });
  const maintenance = createMaintenanceActions({
    updateStatus,
    updateCheckReport,
    settingsUpdateChannel,
    settingsSkippedUpdateVersion,
    updatePromptOpen,
    dismissedUpdateVersion,
    runTask,
    applySettings,
    t,
  });
  const {
    loadUpdaterStatus,
    checkForUpdates,
    openUpdatePrompt,
    cancelUpdatePrompt,
    skipUpdateVersion,
    confirmUpdatePrompt,
  } = maintenance;
  const canOpenDismissedUpdatePrompt = computed(() => {
    const version = updateCheckReport.value?.package?.version;
    return Boolean(
      version &&
        (dismissedUpdateVersion.value === version || settingsSkippedUpdateVersion.value === version),
    );
  });

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
    retryAutoPageSlower,
    stopLiveCapture,
    pollCaptureStatus,
    applyCaptureStatus,
    ensureCapturePolling,
    clearCapturePolling,
    canRetryAutoPageSlower,
    nextPageRecordMinWaitMs,
  } = createCaptureActions({
    activeProfileName,
    locale,
    captureMode,
    captureStatus,
    captureStalledDialogOpen,
    captureActionBusy,
    capturePollInFlight,
    lastReport,
    statusText,
    errorText,
    isCaptureActive,
    isWorkflowBusy,
    t,
    formatError: formatters.formatError,
    formatCaptureState: formatters.formatCaptureState,
    formatCaptureMode: formatters.formatCaptureMode,
    saveRecordViewPrefs,
    setActiveProfileName,
    refreshAll,
  });

  const {
    openDiagnosticPrompt,
    cancelDiagnosticPrompt,
    confirmDiagnosticPrompt,
    startPendingAdminDiagnostic,
    cancelDiagnostic,
    pollDiagnosticStatus,
    applyDiagnosticStatus,
    ensureDiagnosticPolling,
    clearDiagnosticPolling,
  } = createDiagnosticActions({
    diagnosticPromptOpen,
    diagnosticStatus,
    diagnosticActionBusy,
    diagnosticPollInFlight,
    statusText,
    errorText,
    isDiagnosticActive,
    isWorkflowBusy,
    t,
    formatError: formatters.formatError,
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
    clearDiagnosticPolling();
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
    if (shouldSkipFilterWatch()) return;
    normalizeAfterFilterWatch();
    resetPageAfterFilterWatch();
    saveRecordViewPrefs();
    void loadRecords();
  }, { flush: "sync" });
  watch(pageIndex, () => {
    if (shouldSkipPageWatch()) return;
    void loadRecords();
  }, { flush: "sync" });
  watch(recordAdvancedFiltersOpen, () => {
    if (shouldSkipPrefsWatch()) return;
    saveRecordViewPrefs();
  }, { flush: "sync" });
  watch(visibleRecordColumns, () => {
    if (shouldSkipColumnWatch()) return;
    normalizeAfterFilterWatch();
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
      const startedPendingDiagnostic = startedPendingCapture ? false : await startPendingAdminDiagnostic();
      if (!startedPendingCapture && !startedPendingDiagnostic && settings.check_updates_on_startup) {
        void checkForUpdates({ silent: true });
      }
    } catch (error) {
      errorText.value = formatters.formatError(error);
    } finally {
      busy.value = false;
    }
  }

  async function updateRuntimeSettings(patch: SettingsPatch, options: { refreshData?: boolean } = {}) {
    await runTask(t("status.settingsUpdated"), async () => {
      const settings = await api.updateSettings(patch);
      saveRecordViewPrefs();
      applySettings(settings);
      if (options.refreshData) {
        await loadProfiles();
        await refreshAll();
      }
    });
  }

  async function setUiLocale(value: string) {
    if (value === uiLocale.value) return;
    await updateRuntimeSettings({ ui_locale: value });
  }

  async function setDataLocale(value: string) {
    if (value === locale.value) return;
    await updateRuntimeSettings({ locale: value }, { refreshData: true });
  }

  async function setUpdateChannel(value: string) {
    if (value === settingsUpdateChannel.value) return;
    await updateRuntimeSettings({ update_channel: value });
  }

  async function setCheckUpdatesOnStartup(value: boolean) {
    if (value === settingsCheckUpdates.value) return;
    await updateRuntimeSettings({ check_updates_on_startup: value });
  }

  async function openAboutLink(target: AboutLinkTarget) {
    try {
      await api.openAboutLink(target);
    } catch {
      // External link failures are intentionally silent.
    }
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
    if (!isRecordPrefsReady()) {
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

  function showDashboardFiveStarRecords() {
    const scope = selectedDashboardScope.value;
    const mode = latestFiveStarWallModeForPool(scope.pool_kind);
    withApplyingRecordPrefs(() => {
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
    });
    normalizeRecordFilterSelection();
    saveRecordViewPrefs();
    void loadRecords();
  }

  function resetRecordFilters() {
    recordState.actions.resetRecordFilters(() => {
      void loadRecords();
    });
  }

  return reactive({
    t, navItems, kindOrder, kindLabels, activeView, profiles, activeProfileName, newProfileName, profileRenameSource, profileRenameName, profileDeleteTarget, locale, uiLocale, locales, uiLocales, summary, selectedPoolKind, selectedDashboardScope, detail, detailLoading, records, recordTotal, filterOptions, importPath, importMode,
    exportPath, exportMode, backupPath, restorePath, captureMode, captureAutoPageEnabled, captureFullUpdateEnabled, effectiveCaptureMode, lastReport, lastBackup, lastRestore, updateStatus, updateCheckReport, updatePromptOpen, canOpenDismissedUpdatePrompt, assetUrlCache, captureStatus, captureStalledDialogOpen, captureActionBusy,
    capturePollInFlight, diagnosticPromptOpen, diagnosticStatus, diagnosticActionBusy, diagnosticPollInFlight, busy, statusText, errorText, setChartEl, recordPoolKind, recordBannerIds, itemRarities, focusedRarities, rateUpResults, rollBuckets, itemKinds, forkResultMarks, forkPityBadges, dateFrom, dateTo, search,
    sortDirection, pageSize, pageIndex, recordPageJumpOpen, recordPageJumpInput, visibleRecordColumns, recordColumnOptions, visibleRecordGridTemplate, isRecordColumnVisible, recordPageSizes, recordAdvancedFiltersOpen, latestFiveStarWallMode, activeRecordFilterCount, recordBannerOptions, itemRarityOptions, focusedRarityOptions, rateUpResultSelectOptions, rollBucketOptions, itemKindOptions, showForkRecordFilters, forkResultMarkSelectOptions, forkPityBadgeSelectOptions, settingsUpdateChannel, settingsCheckUpdates, dataOperationSummary, activeProfile, allPoolSummaries, bannerSummaries, selectedPoolBannerSummaries, selectedSummary, selectedScopeLabel, isDashboardPoolScope, selectedDetailTitle, hasItemRankingRows, rankingRarityOptions, itemRankingShares, recordPageStart, recordPageEnd, recordPageCount, canPrevPage,
    canNextPage, canFirstPage, canLastPage, bannersForRecordKind, isCaptureActive, isDiagnosticActive, isWorkflowBusy, captureTitle, captureSubtitle, autoPageStatusLine, captureModeLabel, showDashboardBannerRail, showLatestFiveStarWallModeToggle, visibleLatestFiveStarHits, displayedLatestFiveStarHits, fiveWallExpanded, latestFiveStarEmptyText, rankingDialogOpen, rankingDialogTitle, bootstrap, startPendingAdminCapture, startPendingAdminDiagnostic, loadProfiles, createProfile, startRenameProfile, cancelRenameProfile, saveProfileRename, requestDeleteProfile, cancelDeleteProfile, confirmDeleteProfile, selectProfile, setUiLocale, setDataLocale, setUpdateChannel, setCheckUpdatesOnStartup, openAboutLink, refreshAll, selectDashboardPool, selectDashboardBanner, isSelectedDashboardPool, isSelectedDashboardBanner, loadDetail,
    loadFilterOptions, loadRecords, resetRecordFilters, pickImportFile, runImport, startPreferredCapture, startLiveCapture, startFullCapture, retryAutoPageSlower, canRetryAutoPageSlower, nextPageRecordMinWaitMs, closeCaptureStalledDialog, setCaptureAutoPageEnabled, setCaptureFullUpdateEnabled, stopLiveCapture, pollCaptureStatus, applyCaptureStatus, ensureCapturePolling, clearCapturePolling, pickExportFile, runExport, pickBackupFile, runBackup, pickRestoreFile, runRestore,
    openDiagnosticPrompt, cancelDiagnosticPrompt, confirmDiagnosticPrompt, cancelDiagnostic, pollDiagnosticStatus, applyDiagnosticStatus, ensureDiagnosticPolling, clearDiagnosticPolling, loadUpdaterStatus, checkForUpdates, openUpdatePrompt, cancelUpdatePrompt, skipUpdateVersion, confirmUpdatePrompt, runTask, renderChart, goToRecordPage, goToFirstRecordPage, goToLastRecordPage, openRecordPageJump, closeRecordPageJump, confirmRecordPageJump, ...formatters,
    formatPityRatio, summaryProgressLabel, pullCurrency, recordRarityClass, latestFiveStarForPool, latestFiveStarNameForPool, toggleLatestFiveStarWallMode, latestFiveStarWallToggleLabel, toggleFiveWallExpanded, toggleRankingRarity, fiveWallPityTone, fiveWallDistance, showDashboardFiveStarRecords, selectedRarityShares, itemVisualUrl, bannerVisualUrl, hasRecordVisual, hasItemVisual, hasBannerVisual, recordsHaveAnyVisual, resolveVisibleAssets, openRankingDialog, closeRankingDialog,
  });
}

export type AppState = ReturnType<typeof useApp>;
