import { computed, reactive, ref } from "vue";
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
  type UpdateCheckReport,
  type UpdateStatus,
} from "../api";

import { createAssetTools } from "./assets";
import { createCaptureActions, type CaptureRecoveryState } from "./captureActions";
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
import { createAppRuntime } from "./runtime";
import { createSettingsActions } from "./settingsActions";
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
  const assetUrlCache = ref<Record<string, string>>({}), captureStatus = ref<CaptureStatus | null>(null), captureRecoveryState = ref<CaptureRecoveryState | null>(null), captureActionBusy = ref(false), capturePollInFlight = ref(false);
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
    latestFiveStarWallModes, latestFiveStarDistanceModes,
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
  const captureAutoPageEnabled = ref(true), captureFullUpdateEnabled = ref(false), captureWinDivertBackendEnabled = ref(false);
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
    latestFiveStarDistanceModes,
    t,
    saveRecordViewPrefs,
  });
  const { rankingDialogOpen } = dashboardUi.refs;
  const {
    selectedSummary, selectedScopeLabel, isDashboardPoolScope, selectedDetailTitle, hasItemRankingRows,
    rankingRarityOptions, itemRankingShares, rankingDialogTitle, selectedRarityShares, latestFiveStarWallMode,
    latestFiveStarDistanceMode, showLatestFiveStarWallModeToggle, showLatestFiveStarDistanceModeToggle,
    visibleLatestFiveStarHits, displayedLatestFiveStarGroups, fiveWallExpanded, latestFiveStarEmptyText,
    showDashboardBannerRail,
  } = dashboardUi.computed;
  const {
    latestFiveStarForPool, latestFiveStarNameForPool, toggleLatestFiveStarWallMode, latestFiveStarWallToggleLabel,
    toggleLatestFiveStarDistanceMode, latestFiveStarDistanceModeLabel, toggleFiveWallExpanded, toggleRankingRarity,
    fiveWallPityTone, fiveWallGroupItemLabel, summaryProgressLabel, pullCurrency, formatPityRatio, recordRarityClass,
    openRankingDialog, closeRankingDialog,
  } = dashboardUi.actions;
  const { latestFiveStarWallModeForPool } = dashboardUi.internal;
  const runTask = createTaskRunner({ busy, statusText, errorText, formatError: formatters.formatError });

  let loadProfilesImpl: () => Promise<void> = async () => {};
  let refreshAllImpl: () => Promise<void> = async () => {};
  const loadProfilesProxy = () => loadProfilesImpl();
  const refreshAllProxy = () => refreshAllImpl();
  const {
    applySettings,
    setCaptureAutoPageEnabled,
    setCaptureFullUpdateEnabled,
    setCaptureWinDivertBackendEnabled,
    setUiLocale,
    setDataLocale,
    setUpdateChannel,
    setCheckUpdatesOnStartup,
  } = createSettingsActions({
    locale,
    uiLocale,
    settingsUpdateChannel,
    settingsCheckUpdates,
    settingsSkippedUpdateVersion,
    captureAutoPageEnabled,
    captureFullUpdateEnabled,
    captureWinDivertBackendEnabled,
    captureMode,
    effectiveCaptureMode,
    errorText,
    setActiveProfileName,
    saveRecordViewPrefs,
    loadProfiles: loadProfilesProxy,
    refreshAll: refreshAllProxy,
    runTask,
    formatError: formatters.formatError,
    t,
  });

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
    refreshAll: refreshAllProxy,
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
    runRecoveryDialogAction,
    closeCaptureRecoveryDialog,
  } = createCaptureActions({
    activeProfileName,
    locale,
    captureMode,
    captureWinDivertBackendEnabled,
    captureStatus,
    captureRecoveryState,
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
    refreshAll: refreshAllProxy,
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
    loadProfiles: loadProfilesProxy,
    refreshAll: refreshAllProxy,
  });

  async function openAboutLink(target: AboutLinkTarget) {
    try {
      await api.openAboutLink(target);
    } catch {
      // External link failures are intentionally silent.
    }
  }

  const {
    bootstrap,
    refreshAll,
    loadFilterOptions,
    loadRecords,
  } = createAppRuntime({
    busy,
    statusText,
    errorText,
    activeProfileName,
    locale,
    uiLocale,
    locales,
    uiLocales,
    summary,
    selectedDashboardScope,
    selectedPoolKind,
    detail,
    detailLoading,
    filterOptions,
    records,
    recordTotal,
    chartEl,
    rankingDialogOpen,
    recordPoolKind,
    recordBannerIds,
    itemRarities,
    focusedRarities,
    rateUpResults,
    rollBuckets,
    itemKinds,
    forkResultMarks,
    forkPityBadges,
    dateFrom,
    dateTo,
    search,
    sortDirection,
    pageSize,
    pageIndex,
    recordAdvancedFiltersOpen,
    visibleRecordColumns,
    captureStatus,
    diagnosticStatus,
    applySettings,
    loadProfiles: loadProfilesProxy,
    loadUpdaterStatus,
    startPendingAdminCapture,
    startPendingAdminDiagnostic,
    checkForUpdates,
    clearCapturePolling,
    clearDiagnosticPolling,
    disposeChart,
    renderChart,
    currentRecordFilter,
    recordFilterKey,
    saveRecordViewPrefs,
    normalizeDashboardScope,
    loadDetail,
    resolveVisibleAssets,
    isRecordPrefsReady,
    applyRecordViewPrefs,
    normalizeRecordFilterSelection,
    normalizeAfterFilterWatch,
    resetPageAfterFilterWatch,
    shouldSkipFilterWatch,
    shouldSkipPageWatch,
    shouldSkipPrefsWatch,
    shouldSkipColumnWatch,
    formatError: formatters.formatError,
    t,
  });
  loadProfilesImpl = loadProfiles;
  refreshAllImpl = refreshAll;

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
    exportPath, exportMode, backupPath, restorePath, captureMode, captureAutoPageEnabled, captureFullUpdateEnabled, captureWinDivertBackendEnabled, effectiveCaptureMode, lastReport, lastBackup, lastRestore, updateStatus, updateCheckReport, updatePromptOpen, canOpenDismissedUpdatePrompt, assetUrlCache, captureStatus, captureRecoveryState, captureActionBusy,
    capturePollInFlight, diagnosticPromptOpen, diagnosticStatus, diagnosticActionBusy, diagnosticPollInFlight, busy, statusText, errorText, setChartEl, recordPoolKind, recordBannerIds, itemRarities, focusedRarities, rateUpResults, rollBuckets, itemKinds, forkResultMarks, forkPityBadges, dateFrom, dateTo, search,
    sortDirection, pageSize, pageIndex, recordPageJumpOpen, recordPageJumpInput, visibleRecordColumns, recordColumnOptions, visibleRecordGridTemplate, isRecordColumnVisible, recordPageSizes, recordAdvancedFiltersOpen, latestFiveStarWallMode, latestFiveStarDistanceMode, activeRecordFilterCount, recordBannerOptions, itemRarityOptions, focusedRarityOptions, rateUpResultSelectOptions, rollBucketOptions, itemKindOptions, showForkRecordFilters, forkResultMarkSelectOptions, forkPityBadgeSelectOptions, settingsUpdateChannel, settingsCheckUpdates, dataOperationSummary, activeProfile, allPoolSummaries, bannerSummaries, selectedPoolBannerSummaries, selectedSummary, selectedScopeLabel, isDashboardPoolScope, selectedDetailTitle, hasItemRankingRows, rankingRarityOptions, itemRankingShares, recordPageStart, recordPageEnd, recordPageCount, canPrevPage,
    canNextPage, canFirstPage, canLastPage, bannersForRecordKind, isCaptureActive, isDiagnosticActive, isWorkflowBusy, captureTitle, captureSubtitle, autoPageStatusLine, captureModeLabel, showDashboardBannerRail, showLatestFiveStarWallModeToggle, showLatestFiveStarDistanceModeToggle, visibleLatestFiveStarHits, displayedLatestFiveStarGroups, fiveWallExpanded, latestFiveStarEmptyText, rankingDialogOpen, rankingDialogTitle, bootstrap, startPendingAdminCapture, startPendingAdminDiagnostic, loadProfiles, createProfile, startRenameProfile, cancelRenameProfile, saveProfileRename, requestDeleteProfile, cancelDeleteProfile, confirmDeleteProfile, selectProfile, setUiLocale, setDataLocale, setUpdateChannel, setCheckUpdatesOnStartup, openAboutLink, refreshAll, selectDashboardPool, selectDashboardBanner, isSelectedDashboardPool, isSelectedDashboardBanner, loadDetail,
    loadFilterOptions, loadRecords, resetRecordFilters, pickImportFile, runImport, startPreferredCapture, startLiveCapture, startFullCapture, retryAutoPageSlower, canRetryAutoPageSlower, nextPageRecordMinWaitMs, closeCaptureRecoveryDialog, runRecoveryDialogAction, setCaptureAutoPageEnabled, setCaptureFullUpdateEnabled, setCaptureWinDivertBackendEnabled, stopLiveCapture, pollCaptureStatus, applyCaptureStatus, ensureCapturePolling, clearCapturePolling, pickExportFile, runExport, pickBackupFile, runBackup, pickRestoreFile, runRestore,
    openDiagnosticPrompt, cancelDiagnosticPrompt, confirmDiagnosticPrompt, cancelDiagnostic, pollDiagnosticStatus, applyDiagnosticStatus, ensureDiagnosticPolling, clearDiagnosticPolling, loadUpdaterStatus, checkForUpdates, openUpdatePrompt, cancelUpdatePrompt, skipUpdateVersion, confirmUpdatePrompt, runTask, renderChart, goToRecordPage, goToFirstRecordPage, goToLastRecordPage, openRecordPageJump, closeRecordPageJump, confirmRecordPageJump, ...formatters,
    formatPityRatio, summaryProgressLabel, pullCurrency, recordRarityClass, latestFiveStarForPool, latestFiveStarNameForPool, toggleLatestFiveStarWallMode, latestFiveStarWallToggleLabel, toggleLatestFiveStarDistanceMode, latestFiveStarDistanceModeLabel, toggleFiveWallExpanded, toggleRankingRarity, fiveWallPityTone, fiveWallGroupItemLabel, showDashboardFiveStarRecords, selectedRarityShares, itemVisualUrl, bannerVisualUrl, hasRecordVisual, hasItemVisual, hasBannerVisual, recordsHaveAnyVisual, resolveVisibleAssets, openRankingDialog, closeRankingDialog,
  });
}

export type AppState = ReturnType<typeof useApp>;
