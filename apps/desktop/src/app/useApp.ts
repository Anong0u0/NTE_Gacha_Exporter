import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { open, save } from "@tauri-apps/plugin-dialog";
import { nextTick, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import {
  api,
  type AssetsPackCheckReport,
  type AssetsPackInstallReport,
  type AssetsPackStatus,
  type BackupReport,
  type CaptureMode,
  type CaptureStatus,
  type DashboardOverview,
  type DisplayRecord,
  type DoctorReport,
  type ImportReport,
  type PendingAdminCapture,
  type PoolKind,
  type PoolKindDetail,
  type RecordFilter,
  type RecordFilterOptions,
  type RecordSortKey,
  type SortDirection,
  type Profile,
  type RestoreReport,
  type UpdateCheckReport,
  type UpdateStageReport,
  type UpdateStatus,
} from "../api";

import { createAssetTools } from "./assets";
import { createChartTools } from "./chart";
import { createAppComputed } from "./computed";
import { createTranslator, uiLocaleDisplayName } from "./i18n";
import { createMaintenanceActions } from "./maintenance";
import { navItems, type ViewId } from "./navigation";
import { kindOrder, type ExportMode, type HitRarityFilter, type ImportMode, type PoolKindFilter, type RateUpFilter } from "./options";
import { createTaskRunner } from "./task";
import { bannerMeta, bannerTitle, captureRecordMeta, captureRecordName, formatBannerWindow, formatCaptureMode, formatCaptureState, formatError, formatGuarantee, formatPity, formatPullNo, formatResult, formatTime, numberOrDash, parseOptionalNumber, percent } from "./viewHelpers";

use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

export function useApp() {
  const activeView = ref<ViewId>("dashboard"), profiles = ref<Profile[]>([]), activeProfileName = ref("default"), newProfileName = ref("");
  const profileRenameSource = ref(""), profileRenameName = ref(""), profileDeleteTarget = ref("");
  const locale = ref("en"), uiLocale = ref("en"), locales = ref<string[]>(["en"]), uiLocales = ref<string[]>(["en"]), summary = ref<DashboardOverview | null>(null);
  const selectedPoolKind = ref<PoolKind>("monopoly_limited"), selectedBannerId = ref(""), detail = ref<PoolKindDetail | null>(null);
  const records = ref<DisplayRecord[]>([]), recordTotal = ref(0), filterOptions = ref<RecordFilterOptions>({ pools: [], banners: [], record_types: [] });
  const importPath = ref(""), importMode = ref<ImportMode>("raw"), exportPath = ref(""), exportMode = ref<ExportMode>("json");
  const backupPath = ref(""), restorePath = ref(""), captureMode = ref<CaptureMode>("live_only");
  const lastReport = ref<ImportReport | null>(null), lastBackup = ref<BackupReport | null>(null), lastRestore = ref<RestoreReport | null>(null), doctorReport = ref<DoctorReport | null>(null);
  const updateStatus = ref<UpdateStatus | null>(null), updateCheckReport = ref<UpdateCheckReport | null>(null), stagedUpdate = ref<UpdateStageReport | null>(null);
  const assetsPackStatus = ref<AssetsPackStatus | null>(null), assetsPackCheckReport = ref<AssetsPackCheckReport | null>(null), lastAssetsPackInstall = ref<AssetsPackInstallReport | null>(null);
  const assetUrlCache = ref<Record<string, string>>({}), captureStatus = ref<CaptureStatus | null>(null), captureActionBusy = ref(false), capturePollInFlight = ref(false);
  const busy = ref(false), statusText = ref(""), errorText = ref(""), chartEl = ref<HTMLElement | null>(null);
  let capturePollTimer: ReturnType<typeof setInterval> | null = null;
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
  const bannerTitleText = (banner?: Parameters<typeof bannerTitle>[0]) => bannerTitle(banner, t);
  const bannerMetaText = (banner?: Parameters<typeof bannerMeta>[0]) => bannerMeta(banner, t);
  const formatBannerWindowText = (start?: string | null, end?: string | null) => formatBannerWindow(start, end, t);
  const formatPityText = (record: DisplayRecord) => formatPity(record);
  const formatGuaranteeText = (record: DisplayRecord) => formatGuarantee(record, t);
  const formatCaptureStateText = (value?: string | null) => formatCaptureState(value, t);
  const formatCaptureModeText = (value?: string | null) => formatCaptureMode(value, t);
  const uiLocaleName = (value: string) => uiLocaleDisplayName(value, t);

  function setChartEl(element: unknown) {
    chartEl.value = element instanceof HTMLElement ? element : null;
  }
  const { renderChart, disposeChart } = createChartTools(chartEl, summary);

  const recordPoolKind = ref<PoolKindFilter>("all"), recordPoolId = ref(""), recordBannerId = ref(""), recordType = ref("");
  const hitRarity = ref<HitRarityFilter>(""), rateUpResult = ref<RateUpFilter>(""), pity5Min = ref(""), pity5Max = ref("");
  const pity4Min = ref(""), pity4Max = ref(""), dateFrom = ref(""), dateTo = ref(""), search = ref("");
  const sortKey = ref<RecordSortKey>("time"), sortDirection = ref<SortDirection>("desc"), pageSize = ref(50), pageIndex = ref(0);
  const settingsUpdateChannel = ref("stable"), settingsCheckUpdates = ref(false);

  const {
    activeProfile,
    allPoolSummaries,
    trackedPoolCount,
    bannerSummaries,
    trackedBannerCount,
    selectedSummary,
    selectedBanner,
    latest,
    phaseSummaries,
    recordPageStart,
    recordPageEnd,
    canPrevPage,
    canNextPage,
    poolsForRecordKind,
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
    selectedBannerId,
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
  const runTask = createTaskRunner({ busy, statusText, errorText, formatError });

  const {
    assetRefsCount,
    itemVisualUrl,
    bannerVisualUrl,
    selectedBannerPortraitUrls,
    hasRecordVisual,
    hasBannerVisual,
    hasSelectedBannerVisuals,
    recordsHaveAnyVisual,
    resolveVisibleAssets,
  } = createAssetTools({
    assetUrlCache,
    bannerSummaries,
    latest,
    records,
    detail,
    selectedBanner,
    errorText,
    formatError,
  });
  const maintenance = createMaintenanceActions({
    doctorReport, updateStatus, updateCheckReport, stagedUpdate, assetsPackStatus, assetsPackCheckReport,
    lastAssetsPackInstall, assetUrlCache, settingsUpdateChannel, statusText, runTask, resolveVisibleAssets, t,
  });
  const { pingRuntime, runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate, loadAssetsPackStatus, checkAssetsPack, downloadAssetsPack, removeAssetsPack } = maintenance;

  onMounted(async () => {
    await bootstrap();
  });

  onBeforeUnmount(() => {
    clearCapturePolling();
    disposeChart();
  });

  watch(() => summary.value?.rarity_distribution, async () => { await nextTick(); renderChart(); }, { deep: true });
  watch(selectedPoolKind, () => void loadDetail());
  watch([recordPoolKind, recordPoolId, recordBannerId, recordType, hitRarity, rateUpResult, pity5Min, pity5Max, pity4Min, pity4Max, dateFrom, dateTo, search, sortKey, sortDirection, pageSize], () => {
    pageIndex.value = 0;
    void loadRecords();
  });
  watch(pageIndex, () => void loadRecords());
  watch(recordPoolKind, () => {
    if (recordPoolId.value && !poolsForRecordKind.value.some((pool) => pool.pool_id === recordPoolId.value)) recordPoolId.value = "";
    if (recordBannerId.value && !bannersForRecordKind.value.some((banner) => banner.banner_id === recordBannerId.value)) recordBannerId.value = "";
  });

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
      activeProfileName.value = settings.active_profile;
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

  async function startPendingAdminCapture() {
    const pending = await api.takePendingAdminCapture();
    if (!pending) return false;
    activeProfileName.value = pending.profile_name;
    locale.value = pending.locale;
    captureMode.value = pending.mode;
    await startLiveCapture({ skipAdminRequest: true, pending });
    return true;
  }

  async function loadProfiles() {
    profiles.value = await api.listProfiles();
    if (!profiles.value.some((profile) => profile.name === activeProfileName.value) && profiles.value.length > 0) {
      activeProfileName.value = profiles.value[0].name;
    }
  }

  async function createProfile() {
    const name = newProfileName.value.trim();
    if (!name) return;
    await runTask(t("status.profileCreated"), async () => {
      const profile = await api.createProfile(name);
      newProfileName.value = "";
      profileDeleteTarget.value = "";
      await api.setActiveProfile(profile.name);
      activeProfileName.value = profile.name;
      await loadProfiles();
      await refreshAll();
    });
  }

  function startRenameProfile(profile: Profile) {
    profileRenameSource.value = profile.name;
    profileRenameName.value = profile.name;
    profileDeleteTarget.value = "";
  }

  function cancelRenameProfile() {
    profileRenameSource.value = "";
    profileRenameName.value = "";
  }

  async function saveProfileRename() {
    const oldName = profileRenameSource.value;
    const newName = profileRenameName.value.trim();
    if (!oldName || !newName) return;
    if (oldName === newName) {
      cancelRenameProfile();
      return;
    }
    await runTask(t("status.profileRenamed"), async () => {
      const profile = await api.renameProfile(oldName, newName);
      if (activeProfileName.value === oldName || profile.active) {
        activeProfileName.value = profile.name;
      }
      profileDeleteTarget.value = "";
      cancelRenameProfile();
      await loadProfiles();
      await refreshAll();
    });
  }

  function requestDeleteProfile(profile: Profile) {
    if (profiles.value.length <= 1) return;
    profileRenameSource.value = "";
    profileRenameName.value = "";
    profileDeleteTarget.value = profile.name;
  }

  function cancelDeleteProfile() {
    profileDeleteTarget.value = "";
  }

  async function confirmDeleteProfile(profile: Profile) {
    if (profiles.value.length <= 1) return;
    await runTask(t("status.profileDeleted"), async () => {
      const settings = await api.deleteProfile(profile.name);
      activeProfileName.value = settings.active_profile;
      locale.value = settings.locale;
      uiLocale.value = settings.ui_locale || uiLocale.value;
      settingsUpdateChannel.value = settings.update_channel;
      settingsCheckUpdates.value = settings.check_updates_on_startup;
      if (profileRenameSource.value === profile.name) {
        cancelRenameProfile();
      }
      profileDeleteTarget.value = "";
      await loadProfiles();
      await refreshAll();
    });
  }

  async function selectProfile(profileName = activeProfileName.value) {
    const previousProfileName = activeProfileName.value;
    if (!profileName || profileName === previousProfileName) return;
    activeProfileName.value = profileName;
    profileDeleteTarget.value = "";
    cancelRenameProfile();
    await runTask(t("status.profileSelected"), async () => {
      try {
        const settings = await api.updateSettings({ active_profile: profileName });
        locale.value = settings.locale;
        uiLocale.value = settings.ui_locale || uiLocale.value;
        settingsUpdateChannel.value = settings.update_channel;
        settingsCheckUpdates.value = settings.check_updates_on_startup;
        await loadProfiles();
        await refreshAll();
      } catch (error) {
        activeProfileName.value = previousProfileName;
        throw error;
      }
    });
  }

  async function saveSettings() {
    await runTask(t("status.settingsUpdated"), async () => {
      const settings = await api.updateSettings({
        locale: locale.value,
        ui_locale: uiLocale.value,
        update_channel: settingsUpdateChannel.value,
        check_updates_on_startup: settingsCheckUpdates.value,
      });
      activeProfileName.value = settings.active_profile;
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
    const firstBanner = summary.value.banners.find((banner) => banner.total_pulls > 0)?.banner_id;
    selectedPoolKind.value = firstActive ?? selectedPoolKind.value;
    selectedBannerId.value = firstBanner ?? selectedBannerId.value;
    await Promise.all([loadDetail(), loadFilterOptions(), loadRecords()]);
    statusText.value = t("status.dashboardUpdated");
    await resolveVisibleAssets();
    await nextTick();
    renderChart();
  }

  async function loadDetail() {
    if (!activeProfileName.value) return;
    detail.value = await api.poolKindDetail(activeProfileName.value, selectedPoolKind.value, locale.value);
    await resolveVisibleAssets();
  }

  async function loadFilterOptions() {
    if (!activeProfileName.value) return;
    filterOptions.value = await api.recordFilterOptions(activeProfileName.value, locale.value);
  }

  async function loadRecords() {
    if (!activeProfileName.value) return;
    const filter: RecordFilter = {
      pool_kind: recordPoolKind.value === "all" ? null : recordPoolKind.value,
      pool_id: recordPoolId.value || null,
      banner_id: recordBannerId.value || null,
      record_type: recordType.value || null,
      hit_rarity: hitRarity.value ? Number(hitRarity.value) : null,
      rate_up_result: rateUpResult.value || null,
      pity_5_min: parseOptionalNumber(pity5Min.value),
      pity_5_max: parseOptionalNumber(pity5Max.value),
      pity_4_min: parseOptionalNumber(pity4Min.value),
      pity_4_max: parseOptionalNumber(pity4Max.value),
      date_from: dateFrom.value || null,
      date_to: dateTo.value || null,
      search: search.value || null,
      sort_key: sortKey.value,
      sort_direction: sortDirection.value,
      limit: pageSize.value,
      offset: pageIndex.value * pageSize.value,
    };
    const result = await api.listRecords(activeProfileName.value, filter, locale.value);
    records.value = result.records;
    recordTotal.value = result.total;
    await resolveVisibleAssets();
  }

  async function pickImportFile(mode: ImportMode) {
    importMode.value = mode;
    const selected = await open({
      title: mode === "raw" ? t("import.rawJsonl") : t("import.publicJson"),
      multiple: false,
      filters:
        mode === "raw"
          ? [{ name: "Raw JSONL", extensions: ["jsonl"] }]
          : [{ name: "Public JSON", extensions: ["json"] }],
    });
    if (typeof selected === "string") {
      importPath.value = selected;
      await runImport();
    }
  }

  async function runImport() {
    const path = importPath.value.trim();
    if (!path) return;
    await runTask(t("status.importCompleted"), async () => {
      lastReport.value =
        importMode.value === "raw"
          ? await api.importRawJsonl(activeProfileName.value, path, locale.value)
          : await api.importPublicJson(activeProfileName.value, path);
      await refreshAll();
    });
  }

  async function startLiveCapture(options: { skipAdminRequest?: boolean; pending?: PendingAdminCapture } = {}) {
    if ((isWorkflowBusy.value && !options.skipAdminRequest) || !activeProfileName.value) return;
    captureActionBusy.value = true;
    errorText.value = "";
    try {
      if (!options.skipAdminRequest) {
        const relaunching = await api.requestAdminCaptureStart(activeProfileName.value, locale.value, captureMode.value);
        if (relaunching) {
          statusText.value = t("status.waitingAdmin");
          return;
        }
      }
      await applyCaptureStatus(await api.captureStart(activeProfileName.value, locale.value, captureMode.value));
      statusText.value = options.pending
        ? t("capture.modeResumed", { mode: formatCaptureModeText(captureMode.value) })
        : t("status.captureStarted", { mode: formatCaptureModeText(captureMode.value) });
      if (isCaptureActive.value) {
        ensureCapturePolling();
      }
    } catch (error) {
      errorText.value = formatError(error);
    } finally {
      captureActionBusy.value = false;
    }
  }

  async function startFullCapture() {
    captureMode.value = "auto_page_full";
    await startLiveCapture();
  }

  async function stopLiveCapture() {
    const sessionId = captureStatus.value?.session_id;
    if (!sessionId || !isCaptureActive.value || captureActionBusy.value) return;
    captureActionBusy.value = true;
    errorText.value = "";
    try {
      await applyCaptureStatus(await api.captureStop(sessionId));
      statusText.value = t("status.captureStopping");
      if (isCaptureActive.value) {
        ensureCapturePolling();
      }
    } catch (error) {
      errorText.value = formatError(error);
    } finally {
      captureActionBusy.value = false;
    }
  }

  async function pollCaptureStatus() {
    const sessionId = captureStatus.value?.session_id;
    if (!sessionId || capturePollInFlight.value) return;
    capturePollInFlight.value = true;
    try {
      await applyCaptureStatus(await api.captureStatus(sessionId));
    } catch (error) {
      clearCapturePolling();
      errorText.value = formatError(error);
    } finally {
      capturePollInFlight.value = false;
    }
  }

  async function applyCaptureStatus(status: CaptureStatus) {
    captureStatus.value = status;
    if (status.state === "completed") {
      clearCapturePolling();
      if (status.import_report) {
        lastReport.value = status.import_report;
      }
      await refreshAll();
      statusText.value = status.import_report ? t("status.captureMerged") : t("status.captureCompleted");
    } else if (status.state === "failed") {
      clearCapturePolling();
      errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : t("status.captureFailed");
    } else {
      statusText.value = formatCaptureStateText(status.state);
    }
  }

  function ensureCapturePolling() {
    if (capturePollTimer) return;
    capturePollTimer = setInterval(() => {
      void pollCaptureStatus();
    }, 1000);
  }

  function clearCapturePolling() {
    if (!capturePollTimer) return;
    clearInterval(capturePollTimer);
    capturePollTimer = null;
  }

  async function pickExportFile(mode: ExportMode) {
    exportMode.value = mode;
    const selected = await save({
      title: mode === "json" ? t("import.publicJson") : "CSV",
      defaultPath: mode === "json" ? `${activeProfileName.value}-history.json` : `${activeProfileName.value}-history.csv`,
      filters:
        mode === "json"
          ? [{ name: "Public JSON", extensions: ["json"] }]
          : [{ name: "CSV", extensions: ["csv"] }],
    });
    if (typeof selected === "string") {
      exportPath.value = selected;
      await runExport();
    }
  }

  async function runExport() {
    const path = exportPath.value.trim();
    if (!path) return;
    await runTask(t("status.exportCompleted"), async () => {
      if (exportMode.value === "json") {
        await api.exportPublicJson(activeProfileName.value, path, locale.value);
      } else {
        await api.exportCsv(activeProfileName.value, path, locale.value);
      }
    });
  }

  async function pickBackupFile() {
    const selected = await save({
      title: t("import.createBackup"),
      defaultPath: `${activeProfileName.value}-nte-data-backup.zip`,
      filters: [{ name: "Backup zip", extensions: ["zip"] }],
    });
    if (typeof selected === "string") {
      backupPath.value = selected;
      await runBackup();
    }
  }

  async function runBackup() {
    const path = backupPath.value.trim();
    await runTask(t("status.backupCreated"), async () => {
      lastBackup.value = await api.createBackup(path || null);
    });
  }

  async function pickRestoreFile() {
    const selected = await open({
      title: t("import.restoreBackup"),
      multiple: false,
      filters: [{ name: "Backup zip", extensions: ["zip"] }],
    });
    if (typeof selected === "string") {
      restorePath.value = selected;
      await runRestore();
    }
  }

  async function runRestore() {
    const path = restorePath.value.trim();
    if (!path) return;
    await runTask(t("status.backupRestored"), async () => {
      lastRestore.value = await api.restoreBackup(path);
      const settings = await api.getSettings();
      activeProfileName.value = settings.active_profile;
      locale.value = settings.locale;
      uiLocale.value = settings.ui_locale || uiLocale.value;
      settingsUpdateChannel.value = settings.update_channel;
      settingsCheckUpdates.value = settings.check_updates_on_startup;
      await loadProfiles();
      await refreshAll();
    });
  }

  return reactive({
    t, uiLocaleName, navItems, kindOrder, kindLabels, activeView, profiles, activeProfileName, newProfileName, profileRenameSource, profileRenameName, profileDeleteTarget, locale, uiLocale, locales, uiLocales, summary, selectedPoolKind, selectedBannerId, detail, records, recordTotal, filterOptions, importPath, importMode,
    exportPath, exportMode, backupPath, restorePath, captureMode, lastReport, lastBackup, lastRestore, doctorReport, updateStatus, updateCheckReport, stagedUpdate, assetsPackStatus, assetsPackCheckReport, lastAssetsPackInstall, assetUrlCache, captureStatus, captureActionBusy,
    capturePollInFlight, busy, statusText, errorText, setChartEl, recordPoolKind, recordPoolId, recordBannerId, recordType, hitRarity, rateUpResult, pity5Min, pity5Max, pity4Min, pity4Max, dateFrom, dateTo, search,
    sortKey, sortDirection, pageSize, pageIndex, settingsUpdateChannel, settingsCheckUpdates, activeProfile, allPoolSummaries, trackedPoolCount, bannerSummaries, trackedBannerCount, selectedSummary, selectedBanner, latest, phaseSummaries, recordPageStart, recordPageEnd, canPrevPage,
    canNextPage, poolsForRecordKind, bannersForRecordKind, isCaptureActive, isWorkflowBusy, captureTitle, captureSubtitle, autoPageStatusLine, captureModeLabel, assetsPackSummary, bootstrap, startPendingAdminCapture, loadProfiles, createProfile, startRenameProfile, cancelRenameProfile, saveProfileRename, requestDeleteProfile, cancelDeleteProfile, confirmDeleteProfile, selectProfile, saveSettings, refreshAll, loadDetail,
    loadFilterOptions, loadRecords, pickImportFile, runImport, startLiveCapture, startFullCapture, stopLiveCapture, pollCaptureStatus, applyCaptureStatus, ensureCapturePolling, clearCapturePolling, pickExportFile, runExport, pickBackupFile, runBackup, pickRestoreFile, runRestore, pingRuntime,
    runDoctor, loadUpdaterStatus, checkForUpdates, downloadUpdate, installUpdate, loadAssetsPackStatus, checkAssetsPack, downloadAssetsPack, removeAssetsPack, runTask, renderChart, percent, numberOrDash, parseOptionalNumber, formatTime, formatResult: formatResultText, bannerTitle: bannerTitleText, bannerMeta: bannerMetaText,
    formatBannerWindow: formatBannerWindowText, formatPullNo, formatPity: formatPityText, formatGuarantee: formatGuaranteeText, assetRefsCount, itemVisualUrl, bannerVisualUrl, selectedBannerPortraitUrls, hasRecordVisual, hasBannerVisual, hasSelectedBannerVisuals, recordsHaveAnyVisual, resolveVisibleAssets, formatCaptureState: formatCaptureStateText, formatCaptureMode: formatCaptureModeText, captureRecordName, captureRecordMeta, formatError,
  });
}

export type AppState = ReturnType<typeof useApp>;
