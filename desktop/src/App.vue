<script setup lang="ts">
import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { init, use, type ECharts } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  Activity,
  ChevronLeft,
  ChevronRight,
  CircleStop,
  Database,
  Download,
  HardDriveDownload,
  HardDriveUpload,
  FileDown,
  FileJson,
  FolderInput,
  History,
  Plus,
  RadioTower,
  RefreshCw,
  Search,
  Settings,
  Stethoscope,
  Upload,
} from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  api,
  type BackupReport,
  type BannerSummary,
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
  type RateUpResult,
  type SortDirection,
  type Profile,
  type RestoreReport,
  type UpdateCheckReport,
  type UpdateStageReport,
  type UpdateStatus,
} from "./api";

use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

type ViewId = "dashboard" | "records" | "import_export" | "settings";
type ImportMode = "raw" | "public";
type ExportMode = "json" | "csv";
type PoolKindFilter = PoolKind | "all";
type HitRarityFilter = "" | "4" | "5";
type RateUpFilter = "" | RateUpResult;

const navItems = [
  { id: "dashboard" as const, label: "Dashboard", icon: Activity },
  { id: "records" as const, label: "Records", icon: History },
  { id: "import_export" as const, label: "Import/Export", icon: FolderInput },
  { id: "settings" as const, label: "Settings", icon: Settings },
];

const kindOrder: PoolKind[] = ["monopoly_limited", "monopoly_standard", "fork_lottery"];
const kindLabels: Record<PoolKind, string> = {
  monopoly_limited: "Limited",
  monopoly_standard: "Standard",
  fork_lottery: "Fork",
};

const activeView = ref<ViewId>("dashboard");
const profiles = ref<Profile[]>([]);
const activeProfileName = ref("default");
const newProfileName = ref("");
const locale = ref("zh-Hant");
const locales = ref<string[]>(["zh-Hant"]);
const summary = ref<DashboardOverview | null>(null);
const selectedPoolKind = ref<PoolKind>("monopoly_limited");
const selectedBannerId = ref("");
const detail = ref<PoolKindDetail | null>(null);
const records = ref<DisplayRecord[]>([]);
const recordTotal = ref(0);
const filterOptions = ref<RecordFilterOptions>({ pools: [], banners: [], record_types: [] });
const importPath = ref("");
const importMode = ref<ImportMode>("raw");
const exportPath = ref("");
const exportMode = ref<ExportMode>("json");
const backupPath = ref("");
const restorePath = ref("");
const captureMode = ref<CaptureMode>("auto_page_incremental");
const lastReport = ref<ImportReport | null>(null);
const lastBackup = ref<BackupReport | null>(null);
const lastRestore = ref<RestoreReport | null>(null);
const doctorReport = ref<DoctorReport | null>(null);
const updateStatus = ref<UpdateStatus | null>(null);
const updateCheckReport = ref<UpdateCheckReport | null>(null);
const stagedUpdate = ref<UpdateStageReport | null>(null);
const captureStatus = ref<CaptureStatus | null>(null);
const captureActionBusy = ref(false);
const capturePollInFlight = ref(false);
const busy = ref(false);
const statusText = ref("Ready");
const errorText = ref("");
const chartEl = ref<HTMLElement | null>(null);
let chart: ECharts | null = null;
let capturePollTimer: ReturnType<typeof setInterval> | null = null;

const recordPoolKind = ref<PoolKindFilter>("all");
const recordPoolId = ref("");
const recordBannerId = ref("");
const recordType = ref("");
const hitRarity = ref<HitRarityFilter>("");
const rateUpResult = ref<RateUpFilter>("");
const pity5Min = ref("");
const pity5Max = ref("");
const pity4Min = ref("");
const pity4Max = ref("");
const dateFrom = ref("");
const dateTo = ref("");
const search = ref("");
const sortKey = ref<RecordSortKey>("time");
const sortDirection = ref<SortDirection>("desc");
const pageSize = ref(50);
const pageIndex = ref(0);
const settingsUpdateChannel = ref("stable");
const settingsCheckUpdates = ref(false);

const activeProfile = computed(() => profiles.value.find((profile) => profile.name === activeProfileName.value));
const allPoolSummaries = computed(() => summary.value?.pool_kinds ?? []);
const trackedPoolCount = computed(() => allPoolSummaries.value.filter((pool) => pool.total_pulls > 0).length);
const bannerSummaries = computed(() => summary.value?.banners ?? []);
const trackedBannerCount = computed(() => bannerSummaries.value.filter((banner) => banner.total_pulls > 0).length);
const selectedSummary = computed(
  () => allPoolSummaries.value.find((item) => item.pool_kind === selectedPoolKind.value) ?? null,
);
const selectedBanner = computed(() => {
  const byId = bannerSummaries.value.find((banner) => banner.banner_id === selectedBannerId.value);
  if (byId) return byId;
  return (
    bannerSummaries.value.find((banner) => banner.pool_kind === selectedPoolKind.value && banner.total_pulls > 0) ??
    bannerSummaries.value[0] ??
    null
  );
});
const latest = computed(() => summary.value?.latest_records ?? []);
const phaseSummaries = computed(() => summary.value?.time_stats.phases ?? []);
const recordPageStart = computed(() => (recordTotal.value === 0 ? 0 : pageIndex.value * pageSize.value + 1));
const recordPageEnd = computed(() => Math.min(recordTotal.value, (pageIndex.value + 1) * pageSize.value));
const canPrevPage = computed(() => pageIndex.value > 0);
const canNextPage = computed(() => recordPageEnd.value < recordTotal.value);
const poolsForRecordKind = computed(() =>
  filterOptions.value.pools.filter((pool) => recordPoolKind.value === "all" || pool.pool_kind === recordPoolKind.value),
);
const bannersForRecordKind = computed(() =>
  filterOptions.value.banners.filter(
    (banner) => recordPoolKind.value === "all" || banner.pool_kind === recordPoolKind.value,
  ),
);
const isCaptureActive = computed(() => {
  const state = captureStatus.value?.state;
  return state === "starting" || state === "running" || state === "stopping";
});
const isWorkflowBusy = computed(() => busy.value || isCaptureActive.value || captureActionBusy.value);
const captureTitle = computed(() => {
  if (!captureStatus.value) return summary.value?.total_records ? "Merge new records" : "Import records to start tracking";
  if (captureStatus.value.state === "completed") return "Live capture completed";
  if (captureStatus.value.state === "failed") return "Live capture failed";
  if (captureStatus.value.state === "stopping") return "Stopping live capture";
  return "Live capture running";
});
const captureSubtitle = computed(() => {
  if (!captureStatus.value) {
    return summary.value?.last_run
      ? `${summary.value.last_run.records_inserted} inserted, ${summary.value.last_run.records_skipped} skipped`
      : "Live capture, raw replay, and public JSON merge into this profile.";
  }
  if (captureStatus.value.import_report) {
    return `${captureStatus.value.import_report.records_inserted} inserted, ${captureStatus.value.import_report.records_skipped} skipped`;
  }
  if (captureStatus.value.error) {
    return captureStatus.value.error.message;
  }
  return `${captureStatus.value.records_count} records seen`;
});
const autoPageStatusLine = computed(() => {
  const auto = captureStatus.value?.auto_page;
  if (!auto) return "";
  const page =
    auto.current_page && auto.total_pages ? ` page=${auto.current_page}/${auto.total_pages}` : "";
  const pool = auto.pool ? ` ${auto.pool}` : "";
  return `${auto.message}${pool}${page}`;
});
const captureModeLabel = computed(() => formatCaptureMode(captureStatus.value?.mode ?? captureMode.value));

onMounted(async () => {
  await bootstrap();
});

onBeforeUnmount(() => {
  clearCapturePolling();
  chart?.dispose();
});

watch(
  () => summary.value?.rarity_distribution,
  async () => {
    await nextTick();
    renderChart();
  },
  { deep: true },
);

watch(selectedPoolKind, () => {
  void loadDetail();
});

watch(
  [
    recordPoolKind,
    recordPoolId,
    recordBannerId,
    recordType,
    hitRarity,
    rateUpResult,
    pity5Min,
    pity5Max,
    pity4Min,
    pity4Max,
    dateFrom,
    dateTo,
    search,
    sortKey,
    sortDirection,
    pageSize,
  ],
  () => {
  pageIndex.value = 0;
  void loadRecords();
  },
);

watch(pageIndex, () => {
  void loadRecords();
});

watch(recordPoolKind, () => {
  if (recordPoolId.value && !poolsForRecordKind.value.some((pool) => pool.pool_id === recordPoolId.value)) {
    recordPoolId.value = "";
  }
  if (recordBannerId.value && !bannersForRecordKind.value.some((banner) => banner.banner_id === recordBannerId.value)) {
    recordBannerId.value = "";
  }
});

async function bootstrap() {
  busy.value = true;
  try {
    const [settings, maps] = await Promise.all([api.getSettings(), api.mapsList()]);
    locale.value = settings.locale;
    settingsUpdateChannel.value = settings.update_channel;
    settingsCheckUpdates.value = settings.check_updates_on_startup;
    locales.value = maps.locales;
    activeProfileName.value = settings.active_profile;
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
  await runTask("Profile created", async () => {
    const profile = await api.createProfile(name);
    newProfileName.value = "";
    await api.setActiveProfile(profile.name);
    activeProfileName.value = profile.name;
    await loadProfiles();
    await refreshAll();
  });
}

async function selectProfile() {
  await runTask("Profile selected", async () => {
    const settings = await api.updateSettings({ active_profile: activeProfileName.value });
    locale.value = settings.locale;
    settingsUpdateChannel.value = settings.update_channel;
    settingsCheckUpdates.value = settings.check_updates_on_startup;
    await loadProfiles();
    await refreshAll();
  });
}

async function saveSettings() {
  await runTask("Settings updated", async () => {
    const settings = await api.updateSettings({
      active_profile: activeProfileName.value,
      locale: locale.value,
      update_channel: settingsUpdateChannel.value,
      check_updates_on_startup: settingsCheckUpdates.value,
    });
    activeProfileName.value = settings.active_profile;
    locale.value = settings.locale;
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
  statusText.value = "Dashboard updated";
  await nextTick();
  renderChart();
}

async function loadDetail() {
  if (!activeProfileName.value) return;
  detail.value = await api.poolKindDetail(activeProfileName.value, selectedPoolKind.value, locale.value);
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
}

async function pickImportFile(mode: ImportMode) {
  importMode.value = mode;
  const selected = await open({
    title: mode === "raw" ? "Select raw JSONL" : "Select public JSON",
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
  await runTask("Import completed", async () => {
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
    if (!options.skipAdminRequest && captureMode.value !== "live_only") {
      const relaunching = await api.requestAdminCaptureStart(activeProfileName.value, locale.value, captureMode.value);
      if (relaunching) {
        statusText.value = "Waiting for administrator window";
        return;
      }
    }
    await applyCaptureStatus(await api.captureStart(activeProfileName.value, locale.value, captureMode.value));
    statusText.value = options.pending
      ? `${formatCaptureMode(captureMode.value)} resumed as administrator`
      : `${formatCaptureMode(captureMode.value)} started`;
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
    statusText.value = "Live capture stopping";
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
    statusText.value = status.import_report ? "Live capture merged" : "Live capture completed";
  } else if (status.state === "failed") {
    clearCapturePolling();
    errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : "Live capture failed";
  } else {
    statusText.value = formatCaptureState(status.state);
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
    title: mode === "json" ? "Export public JSON" : "Export CSV",
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
  await runTask("Export completed", async () => {
    if (exportMode.value === "json") {
      await api.exportPublicJson(activeProfileName.value, path, locale.value);
    } else {
      await api.exportCsv(activeProfileName.value, path, locale.value);
    }
  });
}

async function pickBackupFile() {
  const selected = await save({
    title: "Create backup",
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
  await runTask("Backup created", async () => {
    lastBackup.value = await api.createBackup(path || null);
  });
}

async function pickRestoreFile() {
  const selected = await open({
    title: "Restore backup",
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
  await runTask("Backup restored", async () => {
    lastRestore.value = await api.restoreBackup(path);
    const settings = await api.getSettings();
    activeProfileName.value = settings.active_profile;
    locale.value = settings.locale;
    settingsUpdateChannel.value = settings.update_channel;
    settingsCheckUpdates.value = settings.check_updates_on_startup;
    await loadProfiles();
    await refreshAll();
  });
}

async function pingSidecar() {
  await runTask("Sidecar responded", () => api.sidecarPing());
}

async function runDoctor() {
  await runTask("Doctor completed", async () => {
    doctorReport.value = await api.doctorRun();
  });
}

async function loadUpdaterStatus() {
  updateStatus.value = await api.updaterStatus();
}

async function checkForUpdates(showStatus = true) {
  await runTask(showStatus ? "Update check completed" : statusText.value, async () => {
    updateCheckReport.value = await api.updaterCheck(settingsUpdateChannel.value);
    await loadUpdaterStatus();
  });
}

async function downloadUpdate() {
  const packageInfo = updateCheckReport.value?.package;
  if (!packageInfo) return;
  await runTask("Update downloaded", async () => {
    stagedUpdate.value = await api.updaterDownloadAndStage(packageInfo);
    await loadUpdaterStatus();
  });
}

async function installUpdate() {
  const version = stagedUpdate.value?.package.version ?? updateStatus.value?.staged_version;
  if (!version) return;
  await runTask("Restarting for update", () => api.updaterInstallStaged(version, true));
}

async function runTask(done: string, task: () => Promise<unknown>) {
  busy.value = true;
  errorText.value = "";
  try {
    await task();
    statusText.value = done;
  } catch (error) {
    errorText.value = formatError(error);
  } finally {
    busy.value = false;
  }
}

function renderChart() {
  if (!chartEl.value || !summary.value) return;
  chart ??= init(chartEl.value);
  chart.setOption({
    animationDuration: 220,
    grid: { top: 12, right: 10, bottom: 24, left: 34 },
    tooltip: { trigger: "axis" },
    xAxis: {
      type: "category",
      data: summary.value.rarity_distribution.map((bucket) => `${bucket.rarity}★`),
      axisTick: { show: false },
    },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: "#e1e6e2" } },
    },
    series: [
      {
        type: "bar",
        data: summary.value.rarity_distribution.map((bucket) => bucket.count),
        itemStyle: { color: "#2d6d64", borderRadius: [3, 3, 0, 0] },
      },
    ],
  });
}

function percent(value?: number | null) {
  if (value == null) return "-";
  return `${Math.round(value * 1000) / 10}%`;
}

function numberOrDash(value?: number | null) {
  return value == null ? "-" : String(Math.round(value * 10) / 10);
}

function parseOptionalNumber(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
}

function formatTime(value?: string | null) {
  return value || "-";
}

function formatResult(value: string) {
  if (value === "unknown") return "Unknown";
  if (value === "not_applicable") return "N/A";
  return value === "off_rate" ? "Off-rate" : "UP";
}

function bannerTitle(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  return banner?.title || banner?.banner_id || "Unknown banner";
}

function bannerMeta(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  const parts = [banner?.version, banner?.phase, banner?.source_confidence].filter(Boolean);
  if (parts.length) return parts.join(" · ");
  return banner && "status" in banner ? banner.status : "unknown";
}

function formatBannerWindow(start?: string | null, end?: string | null) {
  if (!start && !end) return "window unknown";
  return `${start ?? "unknown"} -> ${end ?? "ongoing"}`;
}

function formatPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? "-";
}

function formatPity(record: DisplayRecord) {
  return `5★ ${record.derived.pity_5_before}->${record.derived.pity_5_after} · 4★ ${record.derived.pity_4_before}->${record.derived.pity_4_after}`;
}

function formatGuarantee(record: DisplayRecord) {
  const before = record.derived.guarantee_5_before ? "G before" : "normal";
  const after = record.derived.guarantee_5_after ? "G after" : "normal";
  return `${before} / ${after}`;
}

function assetRefEntries(assetRefs?: Record<string, unknown> | null, preferredKeys: string[] = []) {
  if (!assetRefs) return [];
  const all = Object.entries(assetRefs);
  const preferred = preferredKeys
    .map((key) => all.find(([candidate]) => candidate === key))
    .filter((entry): entry is [string, unknown] => Boolean(entry));
  const rest = all.filter(([key]) => !preferredKeys.includes(key));
  return [...preferred, ...rest].flatMap(([key, value]) => {
    if (Array.isArray(value)) {
      return value.map((item, index) => ({ key: `${key}[${index}]`, value: item }));
    }
    return [{ key, value }];
  });
}

function shortAssetRef(value: unknown) {
  const raw = typeof value === "string" ? value : JSON.stringify(value);
  if (!raw) return "-";
  const compact = raw.split("/").pop() ?? raw;
  return compact.length > 34 ? `${compact.slice(0, 31)}...` : compact;
}

function assetRefTitle(value: unknown) {
  return typeof value === "string" ? value : JSON.stringify(value);
}

function assetRefsCount(assetRefs?: Record<string, unknown> | null) {
  return assetRefEntries(assetRefs).length;
}

function itemAssetEntries(record: DisplayRecord) {
  return assetRefEntries(record.item_asset_refs, ["portrait", "icon", "head_icon"]);
}

function bannerAssetEntries(record: DisplayRecord | BannerSummary) {
  const assetRefs = "banner" in record ? record.banner.asset_refs : record.asset_refs;
  return assetRefEntries(assetRefs, [
    "image",
    "background",
    "icon",
    "featured_portraits",
  ]);
}

function formatCaptureState(value?: string | null) {
  if (!value) return "-";
  if (value === "starting") return "Starting";
  if (value === "running") return "Running";
  if (value === "stopping") return "Stopping";
  if (value === "completed") return "Completed";
  if (value === "failed") return "Failed";
  return value;
}

function formatCaptureMode(value?: string | null) {
  if (value === "live_only") return "Live only";
  if (value === "auto_page_full") return "Full update";
  return "Auto-page";
}

function captureRecordName(record: Record<string, unknown>) {
  return String(record.item_name ?? record.item_id ?? "-");
}

function captureRecordMeta(record: Record<string, unknown>) {
  return String(record.pool_name ?? record.pool_id ?? record.record_type ?? "-");
}

function formatError(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const apiError = error as { code?: string; message?: string };
    return apiError.code ? `${apiError.code}: ${apiError.message ?? ""}` : (apiError.message ?? String(error));
  }
  return error instanceof Error ? error.message : String(error);
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">NTE</div>
        <div>
          <strong>Gacha Exporter</strong>
          <span>local tracker</span>
        </div>
      </div>

      <label class="field">
        <span>Profile</span>
        <select v-model="activeProfileName" :disabled="isWorkflowBusy" @change="selectProfile">
          <option v-for="profile in profiles" :key="profile.name" :value="profile.name">
            {{ profile.name }}
          </option>
        </select>
      </label>

      <form class="inline-form" @submit.prevent="createProfile">
        <input v-model="newProfileName" placeholder="new_profile" />
        <button type="submit" :disabled="isWorkflowBusy || !newProfileName.trim()" title="Create profile">
          <Plus :size="16" />
        </button>
      </form>

      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.id"
          :class="{ active: activeView === item.id }"
          type="button"
          @click="activeView = item.id"
        >
          <component :is="item.icon" :size="18" />
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </aside>

    <main class="workspace">
      <header class="topbar">
        <div>
          <span class="eyebrow">{{ activeProfile?.name ?? activeProfileName }}</span>
          <h1>{{ navItems.find((item) => item.id === activeView)?.label }}</h1>
        </div>
        <div class="topbar-actions">
          <button type="button" :disabled="isWorkflowBusy" title="Refresh" @click="runTask('Dashboard updated', refreshAll)">
            <RefreshCw :size="17" />
          </button>
          <div class="status" :class="{ error: errorText }">{{ errorText || statusText }}</div>
        </div>
      </header>

      <section v-if="activeView === 'dashboard'" class="view-stack">
        <section class="update-band">
          <div>
            <span class="eyebrow">Update Data</span>
            <h2>{{ captureTitle }}</h2>
            <p>{{ captureSubtitle }}</p>
            <div v-if="captureStatus" class="capture-summary">
              <div class="capture-stats">
                <span>{{ captureModeLabel }}</span>
                <span>{{ formatCaptureState(captureStatus.state) }}</span>
                <span>{{ captureStatus.counters.packets_seen }} packets</span>
                <span>{{ captureStatus.counters.decoded_packets }} decoded</span>
                <span>{{ captureStatus.counters.dropped_packets }} dropped</span>
              </div>
              <div v-if="autoPageStatusLine" class="capture-target">{{ autoPageStatusLine }}</div>
              <div v-if="captureStatus.auto_page" class="capture-stats">
                <span>{{ captureStatus.auto_page.completed_pools?.length ?? 0 }} pools done</span>
                <span>{{ captureStatus.auto_page.skipped_pools?.length ?? 0 }} pools skipped</span>
              </div>
              <div v-if="captureStatus.raw_path" class="capture-target">{{ captureStatus.raw_path }}</div>
              <div v-if="captureStatus.target" class="capture-target">
                {{ captureStatus.target.pid ?? "-" }} · {{ captureStatus.target.interface ?? "-" }}
              </div>
              <div v-if="captureStatus.latest_records.length" class="capture-latest">
                <div v-for="record in captureStatus.latest_records.slice(-3)" :key="String(record.record_id ?? record.item_id ?? captureRecordName(record))">
                  <span>{{ captureRecordName(record) }}</span>
                  <small>{{ captureRecordMeta(record) }}</small>
                </div>
              </div>
            </div>
          </div>
          <div class="action-row">
            <div class="segmented mode-toggle">
              <button
                type="button"
                :class="{ active: captureMode === 'auto_page_incremental' }"
                :disabled="isWorkflowBusy"
                @click="captureMode = 'auto_page_incremental'"
              >
                Auto-page
              </button>
              <button
                type="button"
                :class="{ active: captureMode === 'live_only' }"
                :disabled="isWorkflowBusy"
                @click="captureMode = 'live_only'"
              >
                Live only
              </button>
            </div>
            <button class="primary" type="button" :disabled="isWorkflowBusy" @click="startLiveCapture()">
              <RadioTower :size="17" />
              <span>Update Data</span>
            </button>
            <button
              type="button"
              :disabled="isWorkflowBusy"
              @click="startFullCapture"
            >
              <RefreshCw :size="17" />
              <span>Full update</span>
            </button>
            <button type="button" :disabled="!isCaptureActive || captureActionBusy" @click="stopLiveCapture">
              <CircleStop :size="17" />
              <span>Stop</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pickImportFile('raw')">
              <Upload :size="17" />
              <span>Raw JSONL</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pickImportFile('public')">
              <FileJson :size="17" />
              <span>Public JSON</span>
            </button>
          </div>
        </section>

        <section class="metrics-grid">
          <div class="metric">
            <span>Total pulls</span>
            <strong>{{ summary?.total_records ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>Tracked banners</span>
            <strong>{{ trackedBannerCount }}</strong>
          </div>
          <div class="metric">
            <span>Total roll points</span>
            <strong>{{ summary?.resource.total_roll_points ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>Selected 5★ pity</span>
            <strong>{{ selectedBanner?.current_5star_pity ?? selectedSummary?.current_pity ?? 0 }}</strong>
          </div>
        </section>

        <section class="pool-strip">
          <button
            v-for="pool in allPoolSummaries"
            :key="pool.pool_kind"
            :class="{ active: selectedPoolKind === pool.pool_kind }"
            type="button"
            @click="selectedPoolKind = pool.pool_kind"
          >
            <span>
              <strong>{{ pool.label }}</strong>
              <small>{{ kindLabels[pool.pool_kind] }} · {{ pool.total_pulls }} pulls</small>
            </span>
            <span class="pity">{{ pool.current_pity }}/{{ pool.hard_pity }}</span>
            <span class="state">{{ pool.current_guarantee ? "Guaranteed" : "Normal" }}</span>
            <span class="pool-latest">Latest 5★ · {{ pool.latest_5star?.item_name ?? "-" }}</span>
          </button>
        </section>

        <section class="banner-grid">
          <button
            v-for="banner in bannerSummaries"
            :key="banner.banner_id"
            class="banner-card"
            :class="{ active: selectedBanner?.banner_id === banner.banner_id }"
            type="button"
            @click="
              selectedBannerId = banner.banner_id;
              selectedPoolKind = banner.pool_kind;
            "
          >
            <span class="banner-card-head">
              <span>
                <strong>{{ banner.title }}</strong>
                <small>{{ kindLabels[banner.pool_kind] }} · {{ banner.banner_type ?? "banner" }}</small>
              </span>
              <span class="confidence-badge">{{ banner.source_confidence ?? "unknown" }}</span>
            </span>
            <span class="banner-window">{{ formatBannerWindow(banner.start_at, banner.end_at) }}</span>
            <span class="banner-stats">
              <span>{{ banner.total_pulls }} pulls</span>
              <span>{{ banner.roll_points_total }} roll</span>
              <span>5★ {{ banner.current_5star_pity }}</span>
              <span>4★ {{ banner.current_4star_pity }}</span>
            </span>
            <span class="banner-hit-line">
              5★ {{ banner.five_star_count }} · 4★ {{ banner.four_star_count }} · UP {{ banner.rate_up_5_count }}/{{ banner.off_rate_5_count }}
            </span>
            <span class="asset-ref-list">
              <span
                v-for="entry in bannerAssetEntries(banner).slice(0, 3)"
                :key="`${banner.banner_id}-${entry.key}`"
                class="asset-ref-chip"
                :title="assetRefTitle(entry.value)"
              >
                {{ entry.key }}: {{ shortAssetRef(entry.value) }}
              </span>
              <span v-if="bannerAssetEntries(banner).length === 0" class="muted">No refs</span>
            </span>
          </button>
        </section>

        <section class="split wide-left">
          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ selectedSummary?.label ?? "Pool" }}</span>
                <h2>Pool detail</h2>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>Total pulls</span><strong>{{ selectedSummary?.total_pulls ?? 0 }}</strong></div>
              <div><span>5★ hits</span><strong>{{ selectedSummary?.hit_count ?? 0 }}</strong></div>
              <div><span>Average pity</span><strong>{{ numberOrDash(selectedSummary?.average_5star_pity) }}</strong></div>
              <div><span>Shortest</span><strong>{{ numberOrDash(selectedSummary?.min_5star_pity) }}</strong></div>
              <div><span>Longest</span><strong>{{ numberOrDash(selectedSummary?.max_5star_pity) }}</strong></div>
              <div><span>UP rate</span><strong>{{ percent(selectedSummary?.observed_up_rate) }}</strong></div>
            </div>
            <div class="record-table detail-table">
              <div class="record-header five-star-header">
                <span>Time</span>
                <span>Item</span>
                <span>Pool</span>
                <span>Pity</span>
                <span>Result</span>
                <span>Guarantee</span>
              </div>
              <div v-for="hit in detail?.five_star_history ?? []" :key="hit.record.record_id" class="record-line five-star-line">
                <span>{{ formatTime(hit.record.time) }}</span>
                <span>{{ hit.record.item_name }}</span>
                <span>{{ hit.record.pool_label }}</span>
                <span>{{ hit.pity_distance }}</span>
                <span>{{ formatResult(hit.result) }}</span>
                <span>{{ hit.guarantee_before ? "Before" : "-" }} / {{ hit.guarantee_after ? "After" : "-" }}</span>
              </div>
              <div v-if="!detail?.five_star_history.length" class="empty-row">No 5★ records in this pool.</div>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">Rarity</span>
                <h2>Known distribution</h2>
              </div>
            </div>
            <div ref="chartEl" class="chart"></div>
            <div class="rank-list">
              <div v-for="item in summary?.item_ranking ?? []" :key="item.item_id">
                <span>{{ item.item_name }}</span>
                <strong>{{ item.count }}</strong>
              </div>
            </div>
          </div>
        </section>

        <section class="split">
          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ selectedBanner ? bannerMeta(selectedBanner) : "Banner" }}</span>
                <h2>Selected banner</h2>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>Title</span><strong class="stat-text">{{ selectedBanner?.title ?? "-" }}</strong></div>
              <div><span>Pulls</span><strong>{{ selectedBanner?.total_pulls ?? 0 }}</strong></div>
              <div><span>Roll points</span><strong>{{ selectedBanner?.roll_points_total ?? 0 }}</strong></div>
              <div><span>Avg 5★ pity</span><strong>{{ numberOrDash(selectedBanner?.average_5star_pity) }}</strong></div>
              <div><span>Avg 4★ pity</span><strong>{{ numberOrDash(selectedBanner?.average_4star_pity) }}</strong></div>
              <div><span>Latest hit</span><strong class="stat-text">{{ selectedBanner?.latest_hit?.item_name ?? "-" }}</strong></div>
            </div>
            <div class="derived-chip-row">
              <span class="derived-chip">5★ UP {{ selectedBanner?.rate_up_5_count ?? 0 }}</span>
              <span class="derived-chip">5★ off {{ selectedBanner?.off_rate_5_count ?? 0 }}</span>
              <span class="derived-chip">4★ UP {{ selectedBanner?.rate_up_4_count ?? 0 }}</span>
              <span class="derived-chip">missing roll {{ selectedBanner?.missing_roll_point_records ?? 0 }}</span>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">Resource</span>
                <h2>Roll points</h2>
              </div>
            </div>
            <div class="resource-grid">
              <div><span>Total</span><strong>{{ summary?.resource.total_roll_points ?? 0 }}</strong></div>
              <div><span>Known records</span><strong>{{ summary?.resource.known_roll_point_records ?? 0 }}</strong></div>
              <div><span>Missing records</span><strong>{{ summary?.resource.missing_roll_point_records ?? 0 }}</strong></div>
            </div>
            <div class="timeline-list compact-list">
              <div v-for="resource in summary?.resource.by_pool_kind ?? []" :key="resource.pool_kind">
                <span>{{ resource.label }}</span>
                <strong>{{ resource.roll_points_total }}</strong>
              </div>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Phase</span>
              <h2>Banner timeline</h2>
            </div>
          </div>
          <div class="timeline-list">
            <div v-for="phase in phaseSummaries" :key="`${phase.version ?? 'v'}-${phase.phase ?? 'p'}`">
              <span>{{ phase.version ?? "unknown" }} · {{ phase.phase ?? "phase" }}</span>
              <small>{{ phase.banner_count }} banners · {{ phase.total_pulls }} pulls · 5★ {{ phase.five_star_count }} · 4★ {{ phase.four_star_count }}</small>
              <strong>{{ phase.roll_points_total }}</strong>
            </div>
            <div v-if="phaseSummaries.length === 0" class="empty-row">No phase stats.</div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Latest</span>
              <h2>Recent records</h2>
            </div>
          </div>
          <div class="record-list compact">
            <div v-for="record in latest" :key="record.record_id" class="record-row">
              <div>
                <strong>{{ record.item_name }}</strong>
                <span>{{ bannerTitle(record.banner) }} · {{ record.rarity ? `${record.rarity}★` : "unknown" }} · pull {{ formatPullNo(record) }}</span>
                <span class="derived-chip">{{ formatResult(record.derived.rate_up_result) }} · {{ formatPity(record) }}</span>
                <span class="asset-ref-list">
                  <span
                    v-for="entry in itemAssetEntries(record).slice(0, 2)"
                    :key="`${record.record_id}-${entry.key}`"
                    class="asset-ref-chip"
                    :title="assetRefTitle(entry.value)"
                  >
                    {{ entry.key }}: {{ shortAssetRef(entry.value) }}
                  </span>
                  <span v-if="itemAssetEntries(record).length === 0" class="muted">No refs</span>
                </span>
              </div>
              <small>{{ formatTime(record.time) }}</small>
            </div>
          </div>
        </section>
      </section>

      <section v-else-if="activeView === 'records'" class="view-stack">
        <section class="toolbar dense">
          <div class="segmented">
            <button :class="{ active: recordPoolKind === 'all' }" type="button" @click="recordPoolKind = 'all'">All</button>
            <button
              v-for="kind in kindOrder"
              :key="kind"
              :class="{ active: recordPoolKind === kind }"
              type="button"
              @click="recordPoolKind = kind"
            >
              {{ kindLabels[kind] }}
            </button>
          </div>
          <label class="search-box">
            <Search :size="17" />
            <input v-model="search" placeholder="Search records" />
          </label>
        </section>

        <section class="filter-grid">
          <label class="field">
            <span>Pool</span>
            <select v-model="recordPoolId">
              <option value="">All pools</option>
              <option v-for="pool in poolsForRecordKind" :key="pool.pool_id" :value="pool.pool_id">
                {{ pool.label }} ({{ pool.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Banner</span>
            <select v-model="recordBannerId">
              <option value="">All banners</option>
              <option v-for="banner in bannersForRecordKind" :key="banner.banner_id" :value="banner.banner_id">
                {{ banner.title }} ({{ banner.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Type</span>
            <select v-model="recordType">
              <option value="">All types</option>
              <option v-for="type in filterOptions.record_types" :key="type.record_type" :value="type.record_type">
                {{ type.record_type }} ({{ type.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Hit rarity</span>
            <select v-model="hitRarity">
              <option value="">All hits</option>
              <option value="5">5★</option>
              <option value="4">4★</option>
            </select>
          </label>
          <label class="field">
            <span>Rate-up</span>
            <select v-model="rateUpResult">
              <option value="">All results</option>
              <option value="up">UP</option>
              <option value="off_rate">Off-rate</option>
              <option value="not_applicable">N/A</option>
              <option value="unknown">Unknown</option>
            </select>
          </label>
          <label class="field">
            <span>From</span>
            <input v-model="dateFrom" type="date" />
          </label>
          <label class="field">
            <span>To</span>
            <input v-model="dateTo" type="date" />
          </label>
          <label class="field">
            <span>Sort</span>
            <select v-model="sortKey">
              <option value="time">Time</option>
              <option value="banner">Banner</option>
              <option value="pool">Pool</option>
              <option value="item">Item</option>
              <option value="rarity">Rarity</option>
              <option value="record_type">Type</option>
              <option value="pull_no">Pull no</option>
              <option value="pity_5">5★ pity</option>
              <option value="pity_4">4★ pity</option>
              <option value="rate_up">Rate-up</option>
            </select>
          </label>
          <label class="field">
            <span>Direction</span>
            <select v-model="sortDirection">
              <option value="desc">Desc</option>
              <option value="asc">Asc</option>
            </select>
          </label>
          <label class="field">
            <span>5★ pity min</span>
            <input v-model="pity5Min" inputmode="numeric" placeholder="0" />
          </label>
          <label class="field">
            <span>5★ pity max</span>
            <input v-model="pity5Max" inputmode="numeric" placeholder="90" />
          </label>
          <label class="field">
            <span>4★ pity min</span>
            <input v-model="pity4Min" inputmode="numeric" placeholder="0" />
          </label>
          <label class="field">
            <span>4★ pity max</span>
            <input v-model="pity4Max" inputmode="numeric" placeholder="10" />
          </label>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ recordPageStart }}-{{ recordPageEnd }} of {{ recordTotal }}</span>
              <h2>History</h2>
            </div>
            <div class="pager">
              <select v-model.number="pageSize">
                <option :value="50">50</option>
                <option :value="100">100</option>
                <option :value="200">200</option>
              </select>
              <button type="button" :disabled="!canPrevPage || isWorkflowBusy" title="Previous page" @click="pageIndex--">
                <ChevronLeft :size="16" />
              </button>
              <button type="button" :disabled="!canNextPage || isWorkflowBusy" title="Next page" @click="pageIndex++">
                <ChevronRight :size="16" />
              </button>
            </div>
          </div>
          <div class="record-table history-table">
            <div class="record-header history-header">
              <span>Time</span>
              <span>Banner</span>
              <span>Item</span>
              <span>Rarity</span>
              <span>Pull</span>
              <span>Pity</span>
              <span>Result</span>
              <span>Rolls</span>
              <span>Assets</span>
            </div>
            <div v-for="record in records" :key="record.record_id" class="record-line history-line">
              <span>{{ formatTime(record.time) }}</span>
              <span>
                <strong>{{ bannerTitle(record.banner) }}</strong>
                <small>{{ bannerMeta(record.banner) }}</small>
              </span>
              <span>
                <strong>{{ record.item_name }}</strong>
                <small v-if="record.secondary_item_name">{{ record.secondary_item_name }} x{{ record.secondary_count ?? 1 }}</small>
              </span>
              <span>{{ record.rarity ? `${record.rarity}★` : "-" }}</span>
              <span>{{ formatPullNo(record) }}</span>
              <span>{{ formatPity(record) }}</span>
              <span>
                <span class="derived-chip">{{ formatResult(record.derived.rate_up_result) }}</span>
                <small>{{ formatGuarantee(record) }}</small>
              </span>
              <span>{{ record.roll_points ?? "-" }}</span>
              <span class="asset-ref-list">
                <span
                  v-for="entry in itemAssetEntries(record).slice(0, 2)"
                  :key="`${record.record_id}-history-${entry.key}`"
                  class="asset-ref-chip"
                  :title="assetRefTitle(entry.value)"
                >
                  {{ entry.key }}: {{ shortAssetRef(entry.value) }}
                </span>
                <span v-if="itemAssetEntries(record).length === 0 && assetRefsCount(record.banner.asset_refs) === 0" class="muted">No refs</span>
                <span v-else-if="assetRefsCount(record.banner.asset_refs)" class="asset-ref-chip">
                  banner refs {{ assetRefsCount(record.banner.asset_refs) }}
                </span>
              </span>
            </div>
            <div v-if="records.length === 0" class="empty-row">No records match current filters.</div>
          </div>
        </section>
      </section>

      <section v-else-if="activeView === 'import_export'" class="view-stack narrow">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Import</span>
              <h2>Update data</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="isWorkflowBusy" @click="pickImportFile('raw')">
              <Upload :size="17" />
              <span>Raw JSONL replay</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pickImportFile('public')">
              <FileJson :size="17" />
              <span>Public JSON</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>Selected import path</span>
              <input v-model="importPath" placeholder="D:\\path\\history.raw.jsonl" />
            </label>
            <select v-model="importMode">
              <option value="raw">Raw JSONL</option>
              <option value="public">Public JSON</option>
            </select>
            <button type="button" :disabled="isWorkflowBusy || !importPath.trim()" @click="runImport">Import</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Export</span>
              <h2>Shareable files</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="isWorkflowBusy" @click="pickExportFile('json')">
              <FileDown :size="17" />
              <span>Public JSON</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pickExportFile('csv')">
              <Download :size="17" />
              <span>CSV</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>Selected export path</span>
              <input v-model="exportPath" placeholder="D:\\path\\history.json" />
            </label>
            <select v-model="exportMode">
              <option value="json">Public JSON</option>
              <option value="csv">CSV</option>
            </select>
            <button type="button" :disabled="isWorkflowBusy || !exportPath.trim()" @click="runExport">Export</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Backup</span>
              <h2>Portable data snapshot</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="isWorkflowBusy" @click="pickBackupFile">
              <HardDriveDownload :size="17" />
              <span>Create backup</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pickRestoreFile">
              <HardDriveUpload :size="17" />
              <span>Restore backup</span>
            </button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>Selected backup path</span>
              <input v-model="backupPath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="isWorkflowBusy" @click="runBackup">Backup</button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>Selected restore path</span>
              <input v-model="restorePath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="isWorkflowBusy || !restorePath.trim()" @click="runRestore">Restore</button>
          </div>
        </section>

        <section v-if="lastReport" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ lastReport.source_kind }}</span>
              <h2>Last import</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Seen</span><strong>{{ lastReport.records_seen }}</strong></div>
            <div><span>Inserted</span><strong>{{ lastReport.records_inserted }}</strong></div>
            <div><span>Skipped</span><strong>{{ lastReport.records_skipped }}</strong></div>
          </div>
        </section>

        <section v-if="lastBackup" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ lastBackup.path }}</span>
              <h2>Last backup</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Profiles</span><strong>{{ lastBackup.profile_count }}</strong></div>
            <div><span>Records</span><strong>{{ lastBackup.record_count }}</strong></div>
          </div>
        </section>

        <section v-if="lastRestore" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ lastRestore.source_path }}</span>
              <h2>Last restore</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Profiles</span><strong>{{ lastRestore.profiles_seen }}</strong></div>
            <div><span>Created</span><strong>{{ lastRestore.profiles_created }}</strong></div>
            <div><span>Merged</span><strong>{{ lastRestore.profiles_merged }}</strong></div>
            <div><span>Inserted</span><strong>{{ lastRestore.records_inserted }}</strong></div>
            <div><span>Skipped</span><strong>{{ lastRestore.records_skipped }}</strong></div>
          </div>
        </section>
      </section>

      <section v-else class="view-stack narrow">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Runtime</span>
              <h2>Settings</h2>
            </div>
          </div>
          <div class="form-grid">
            <label class="field">
              <span>Profile</span>
              <select v-model="activeProfileName" :disabled="isWorkflowBusy">
                <option v-for="profile in profiles" :key="profile.name" :value="profile.name">
                  {{ profile.name }}
                </option>
              </select>
            </label>
            <label class="field">
              <span>Locale</span>
              <select v-model="locale" :disabled="isWorkflowBusy">
                <option v-for="item in locales" :key="item" :value="item">{{ item }}</option>
              </select>
            </label>
            <label class="field">
              <span>Update channel</span>
              <select v-model="settingsUpdateChannel" :disabled="isWorkflowBusy">
                <option value="stable">stable</option>
                <option value="beta">beta</option>
              </select>
            </label>
            <label class="check-field">
              <input v-model="settingsCheckUpdates" type="checkbox" :disabled="isWorkflowBusy" />
              <span>Check updates on startup</span>
            </label>
            <button class="primary" type="button" :disabled="isWorkflowBusy" @click="saveSettings">
              <Settings :size="17" />
              <span>Save settings</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="pingSidecar">
              <Database :size="17" />
              <span>Ping sidecar</span>
            </button>
            <button type="button" :disabled="isWorkflowBusy" @click="runDoctor">
              <Stethoscope :size="17" />
              <span>Doctor</span>
            </button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Updater</span>
              <h2>Portable update</h2>
            </div>
          </div>
          <div class="stat-table compact">
            <div><span>Current</span><strong>{{ updateStatus?.current_version ?? "-" }}</strong></div>
            <div><span>Layout</span><strong>{{ updateStatus?.supported_layout ? "Portable" : "Unsupported" }}</strong></div>
            <div><span>Available</span><strong>{{ updateCheckReport?.package?.version ?? "-" }}</strong></div>
            <div><span>Staged</span><strong>{{ stagedUpdate?.package.version ?? updateStatus?.staged_version ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" :disabled="isWorkflowBusy" @click="checkForUpdates(true)">
              <RefreshCw :size="17" />
              <span>Check updates</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="isWorkflowBusy || !updateCheckReport?.package"
              @click="downloadUpdate"
            >
              <Download :size="17" />
              <span>Download</span>
            </button>
            <button
              type="button"
              :disabled="isWorkflowBusy || !(stagedUpdate?.package.version || updateStatus?.staged_version)"
              @click="installUpdate"
            >
              <HardDriveUpload :size="17" />
              <span>Restart to update</span>
            </button>
          </div>
        </section>

        <section v-if="doctorReport" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">exit {{ doctorReport.exit_code }}</span>
              <h2>Doctor</h2>
            </div>
            <FileJson :size="18" />
          </div>
          <pre>{{ doctorReport.lines.join("\n") }}</pre>
        </section>
      </section>
    </main>
  </div>
</template>
