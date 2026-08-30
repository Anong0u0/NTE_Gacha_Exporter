import type {
  AppApi,
  AssetResolveRequest,
  CaptureMode,
  CaptureStartOptions,
  CaptureStatus,
  AutoPageStatus,
  DashboardSelection,
  DiagnosticStatus,
  DiagnosticMode,
  PoolKind,
  RecordFilter,
  SettingsPatch,
  UpdatePackage,
} from "./types";
import { mockOverview, mockRecordPage, mockReport, mockSelectionDetail } from "./mock/analysis";
import {
  mockCaptureSessions,
  mockFilterOptionsForScenario,
  mockProfile,
  mockScenario,
  mockRecordsForScenario,
  mockSummaryForScenario,
} from "./mock-data";

const MOCK_APP_VERSION = __NTE_APP_VERSION__;
const mockProfiles = [{ ...mockProfile }];
let mockActiveProfileName = mockProfile.name;
let mockLocale = "zh-Hant";
let mockUiLocale = "zh-Hant";
let mockUpdateChannel = "stable";
let mockCheckUpdatesOnStartup = true;
let mockSkippedUpdateVersion: string | null = null;
let mockCaptureAutoPageEnabled = true;
let mockCaptureFullUpdateEnabled = false;
let mockCaptureWinDivertBackendEnabled = false;
let mockWindDivertInstalled = false;
const mockDiagnosticSessions = new Map<string, { polls: number; stopped: boolean; duration: number; mode: DiagnosticMode }>();

function validateMockProfileName(name: string) {
  const profileName = name.trim();
  const upper = profileName.toUpperCase();
  const reserved = ["CON", "PRN", "AUX", "NUL"].includes(upper) || /^(COM|LPT)[1-9]$/.test(upper);
  if (!profileName || profileName.length > 40) throw new Error("profile name length must be 1..40");
  if (!/^[A-Za-z0-9_-]+$/.test(profileName)) throw new Error("profile name must use ASCII letters, digits, _ or -");
  if (reserved) throw new Error("profile name must not use a reserved Windows device name");
  if (mockProfiles.some((profile) => profile.name.toLowerCase() === profileName.toLowerCase())) {
    throw new Error(`profile already exists: ${profileName}`);
  }
  return profileName;
}


function mockSettings() {
  return {
    active_profile: mockActiveProfileName,
    locale: mockLocale,
    ui_locale: mockUiLocale,
    update_channel: mockUpdateChannel,
    check_updates_on_startup: mockCheckUpdatesOnStartup,
    skipped_update_version: mockSkippedUpdateVersion,
    capture_auto_page_enabled: mockCaptureAutoPageEnabled,
    capture_full_update_enabled: mockCaptureFullUpdateEnabled,
    capture_windivert_backend_enabled: mockCaptureWinDivertBackendEnabled,
  };
}

function mockWindDivertStatus() {
  const installDir = "mock-root/drivers/windivert";
  return {
    platform_supported: true,
    installed: mockWindDivertInstalled,
    version: "2.2.2-A",
    install_dir: installDir,
    dll_path: `${installDir}/WinDivert.dll`,
    sys_path: `${installDir}/WinDivert64.sys`,
    license_path: `${installDir}/LICENSE`,
    download_url: "https://github.com/basil00/WinDivert/releases/download/v2.2.2/WinDivert-2.2.2-A.zip",
    zip_sha256: "63cb41763bb4b20f600b6de04e991a9c2be73279e317d4d82f237b150c5f3f15",
    loadable: mockWindDivertInstalled,
    error: mockWindDivertInstalled ? null : "WinDivert is not installed",
  };
}

export const mockApi: AppApi = {
  async getSettings() {
    return mockSettings();
  },
  async updateSettings(patch: SettingsPatch) {
    mockActiveProfileName = patch.active_profile ?? mockActiveProfileName;
    mockLocale = patch.locale ?? mockLocale;
    mockUiLocale = patch.ui_locale ?? mockUiLocale;
    mockUpdateChannel = patch.update_channel ?? mockUpdateChannel;
    mockCheckUpdatesOnStartup = patch.check_updates_on_startup ?? mockCheckUpdatesOnStartup;
    mockSkippedUpdateVersion = patch.skipped_update_version ?? mockSkippedUpdateVersion;
    mockCaptureAutoPageEnabled = patch.capture_auto_page_enabled ?? mockCaptureAutoPageEnabled;
    mockCaptureFullUpdateEnabled = patch.capture_full_update_enabled ?? mockCaptureFullUpdateEnabled;
    mockCaptureWinDivertBackendEnabled = patch.capture_windivert_backend_enabled ?? mockCaptureWinDivertBackendEnabled;
    if (!mockCaptureAutoPageEnabled) mockCaptureFullUpdateEnabled = false;
    if (mockCaptureFullUpdateEnabled) mockCaptureAutoPageEnabled = true;
    return mockSettings();
  },
  async listProfiles() {
    return mockProfiles.map((profile) => ({ ...profile, active: profile.name === mockActiveProfileName }));
  },
  async createProfile(name: string) {
    const profileName = validateMockProfileName(name);
    const profile = { name: profileName, created_at: "0", updated_at: "0", active: false };
    mockProfiles.push(profile);
    return profile;
  },
  async setActiveProfile(profileName: string) {
    mockActiveProfileName = profileName;
    return mockSettings();
  },
  async renameProfile(oldName: string, newName: string) {
    const profile = mockProfiles.find((item) => item.name === oldName);
    if (!profile) throw new Error(`profile not found: ${oldName}`);
    if (oldName === newName) return { ...profile, active: profile.name === mockActiveProfileName };
    profile.name = validateMockProfileName(newName);
    profile.updated_at = "0";
    if (mockActiveProfileName === oldName) mockActiveProfileName = newName;
    return { ...profile, active: profile.name === mockActiveProfileName };
  },
  async deleteProfile(profileName: string) {
    const index = mockProfiles.findIndex((profile) => profile.name === profileName);
    if (index < 0) throw new Error(`profile not found: ${profileName}`);
    if (mockProfiles.length <= 1) throw new Error("cannot delete the last profile");
    mockProfiles.splice(index, 1);
    if (mockActiveProfileName === profileName) mockActiveProfileName = mockProfiles[0].name;
    return mockSettings();
  },
  async importPublicJson(profileName: string, path: string) {
    return mockReport(profileName, "public_json", path);
  },
  async importRawJsonl(profileName: string, path: string) {
    return mockReport(profileName, "raw_jsonl", path);
  },
  async profileAnalysisView(_profileName: string, selection: DashboardSelection, recordFilter: RecordFilter) {
    return {
      overview: await mockOverview(),
      selected_detail: mockSelectionDetail(selection),
      record_filter_options: mockFilterOptionsForScenario(),
      record_page: mockRecordPage(recordFilter),
    };
  },
  async dashboardOverview() {
    return mockOverview();
  },
  async poolKindDetail(_profileName: string, poolKind: PoolKind) {
    const summary = mockSummaryForScenario().find((item) => item.pool_kind === poolKind) ?? mockSummaryForScenario()[0];
    const detail = mockSelectionDetail({ kind: "pool_kind", pool_kind: poolKind });
    return {
      summary,
      five_star_history: detail.five_star_history,
      five_star_wall_history: detail.five_star_wall_history,
    };
  },
  async dashboardSelectionDetail(_profileName: string, selection: DashboardSelection) {
    return mockSelectionDetail(selection);
  },
  async dashboardScopeDetail(_profileName: string, selection: DashboardSelection) {
    return mockSelectionDetail(selection);
  },
  async listRecords(_profileName: string, filter: RecordFilter) {
    return mockRecordPage(filter);
  },
  async recordPage(_profileName: string, filter: RecordFilter) {
    return mockRecordPage(filter);
  },
  async recordFilterOptions() {
    return mockFilterOptionsForScenario();
  },
  async exportPublicJson() {
    return undefined;
  },
  async exportCsv() {
    return undefined;
  },
  async createBackup(path?: string | null) {
    return {
      path: path ?? "data/backups/backup-mock.zip",
      profile_count: 1,
      record_count: mockRecordsForScenario().length,
      created_at: String(Date.now()),
    };
  },
  async restoreBackup(path: string) {
    return {
      source_path: path,
      profiles_seen: 1,
      profiles_created: 0,
      profiles_merged: 1,
      records_seen: mockRecordsForScenario().length,
      records_inserted: 1,
      records_skipped: 1,
      settings_restored: true,
      completed_at: String(Date.now()),
    };
  },
  async mapsList() {
    return { locales: ["zh-Hant", "en", "ja"] };
  },
  async uiLocaleList() {
    return { locales: ["en", "zh-CN", "zh-Hant"] };
  },
  async systemLocale() {
    return "zh-TW";
  },
  async openAboutLink() {
    return undefined;
  },
  async doctorRun() {
    return { ok: true, exit_code: 0, lines: ["mock doctor ok"] };
  },
  async updaterStatus() {
    return {
      portable_root: "mock-root",
      current_version: MOCK_APP_VERSION,
      supported_layout: true,
      staged_version: null,
      rollback_version: null,
    };
  },
  async updaterCheck() {
    return {
      current_version: MOCK_APP_VERSION,
      channel: "stable",
      available: false,
      changelog: [],
      package: null,
    };
  },
  async updaterDownloadAndStage(packageInfo: UpdatePackage) {
    return {
      package: packageInfo,
      archive_path: `mock/${packageInfo.asset_name}`,
      staging_path: `mock/staging/${packageInfo.version}`,
    };
  },
  async updaterInstallStaged() {
    return undefined;
  },
  async assetsResolveRefs(refs: AssetResolveRequest[]) {
    return refs.map((ref) => ({
      ...ref,
      url: mockAssetDataUrl(ref.asset_ref, ref.kind ?? "asset"),
    }));
  },
  async windivertStatus() {
    return mockWindDivertStatus();
  },
  async windivertInstall() {
    mockWindDivertInstalled = true;
    const status = mockWindDivertStatus();
    return {
      status,
      downloaded: true,
      verified_sha256: status.zip_sha256,
      installed_files: [status.dll_path, status.sys_path, status.license_path],
    };
  },
  async requestAdminCaptureStart() {
    return false;
  },
  async takePendingAdminCapture() {
    return null;
  },
  async requestAdminDiagnosticStart() {
    return false;
  },
  async takePendingAdminDiagnostic() {
    return null;
  },
  async captureStart(profileName: string, locale?: string, mode: CaptureMode = "live_only", options?: CaptureStartOptions) {
    const sessionId = `mock-capture-${Date.now()}`;
    const resolvedLocale = locale?.trim() ? locale : mockLocale;
    mockCaptureSessions.set(sessionId, { profileName, locale: resolvedLocale, polls: 0, stopped: false, mode, options });
    return mockCaptureStatus(sessionId);
  },
  async captureStatus(sessionId: string) {
    const session = mockCaptureSessions.get(sessionId);
    if (session) {
      session.polls += 1;
    }
    return mockCaptureStatus(sessionId);
  },
  async captureStop(sessionId: string) {
    const session = mockCaptureSessions.get(sessionId);
    if (session) {
      session.stopped = true;
      session.polls = Math.max(session.polls, 2);
    }
    return mockCaptureStatus(sessionId);
  },
  async diagnosticStart(durationSeconds = 20, mode: DiagnosticMode = "pktmon") {
    const sessionId = `mock-diagnostic-${Date.now()}`;
    mockDiagnosticSessions.set(sessionId, { polls: 0, stopped: false, duration: durationSeconds, mode });
    return mockDiagnosticStatus(sessionId);
  },
  async diagnosticStatus(sessionId: string) {
    const session = mockDiagnosticSessions.get(sessionId);
    if (session) {
      session.polls += 1;
    }
    return mockDiagnosticStatus(sessionId);
  },
  async diagnosticCancel(sessionId: string) {
    const session = mockDiagnosticSessions.get(sessionId);
    if (session) {
      session.stopped = true;
      session.polls = Math.max(session.polls, 2);
    }
    return mockDiagnosticStatus(sessionId);
  },
};


function mockCaptureStatus(sessionId: string): CaptureStatus {
  const session = mockCaptureSessions.get(sessionId);
  if (session && shouldMockCaptureStalled(session)) {
    return mockCaptureStalledStatus(sessionId, session);
  }
  if (session && shouldMockCapturePartial(session)) {
    return mockCapturePartialStatus(sessionId, session);
  }
  const completed = Boolean(session?.stopped || (session && session.polls >= 2));
  const profileName = session?.profileName ?? mockProfile.name;
  const locale = session?.locale ?? mockLocale;
  const mode = session?.mode ?? "live_only";
  const rawPath = "data/runs/raw-mock.jsonl";
  const records = mockRecordsForScenario();
  const recordsCount = completed ? records.length : Math.min(2, Math.max(0, session?.polls ?? 0));
  return {
    session_id: sessionId,
    state: completed ? "completed" : session?.polls ? "running" : "starting",
    profile_name: profileName,
    locale,
    mode,
    records_count: recordsCount,
    latest_records: records.slice(0, recordsCount),
    counters: {
      packets_seen: completed ? 24 : 8,
      decoded_packets: completed ? 3 : 1,
      dropped_packets: 0,
    },
    started_at: Date.now() / 1000 - 6,
    updated_at: Date.now() / 1000,
    target: {
      pid: "1234",
      interface: "mock0",
      ports: [30230],
      bpf: "port 30230",
    },
    auto_page: mockAutoPageStatus(mode, session?.polls ?? 0, completed),
    raw_path: rawPath,
    error: null,
    import_report: completed ? mockReport(profileName, mockCaptureSourceKind(mode), rawPath) : null,
  };
}

function mockAutoPageStatus(
  mode: CaptureMode,
  polls: number,
  completed: boolean,
): AutoPageStatus | null {
  if (mode === "live_only" || (!completed && polls === 0)) return null;
  if (!completed) {
    return {
      state: "running",
      message: "page next",
      kind: "page",
      step: "limitedBoardPages",
      pool: "limited",
      current_page: 2,
      total_pages: 3,
      technical_detail: "",
      elapsed_seconds: 1,
    };
  }

  const fullUpdate = mode === "auto_page_full";
  const pagesByPool = fullUpdate
    ? { limited: 3, standard: 3, fork: 3 }
    : { limited: 3, standard: 3, fork: 1 };
  return {
    state: "completed",
    message: "auto page completed",
    reason: null,
    capture_drain_warning: null,
    completed_pools: fullUpdate ? ["limited", "standard", "fork"] : ["limited", "standard"],
    skipped_pools: fullUpdate ? [] : ["fork"],
    visited_pages: fullUpdate ? 9 : 7,
    visited_pages_by_pool: pagesByPool,
    required_pages_by_pool: { ...pagesByPool },
    decoded_pages_by_pool: { ...pagesByPool },
    missing_pages_by_pool: {},
    last_page_by_pool: { ...pagesByPool },
  };
}

function shouldMockCaptureStalled(session: { mode: CaptureMode; options?: CaptureStartOptions }) {
  return mockScenario() === "capture-stalled"
    && session.mode !== "live_only"
    && (session.options?.page_record_min_wait_ms ?? 300) < 500;
}

function shouldMockCapturePartial(session: { mode: CaptureMode; options?: CaptureStartOptions }) {
  return mockScenario() === "capture-partial"
    && session.mode !== "live_only"
    && (session.options?.page_record_min_wait_ms ?? 300) < 500;
}

function mockCapturePartialStatus(
  sessionId: string,
  session: { profileName: string; locale: string; stopped: boolean; mode: CaptureMode },
): CaptureStatus {
  const rawPath = "data/runs/raw-mock-partial.jsonl";
  const records = mockRecordsForScenario();
  const partial = session.stopped;
  const fullUpdate = session.mode === "auto_page_full";
  const completedPools = fullUpdate ? ["limited", "standard", "fork"] : ["limited", "standard"];
  const skippedPools = fullUpdate ? [] : ["fork"];
  const requiredPagesByPool: Record<string, number> = fullUpdate
    ? { limited: 55, standard: 60, fork: 45 }
    : { limited: 55, standard: 60 };
  const missingPagesByPool: Record<string, number> = fullUpdate
    ? { limited: 2, standard: 1, fork: 2 }
    : { limited: 2, standard: 1 };
  const report = {
    ...mockReport(session.profileName, mockCaptureSourceKind(session.mode), rawPath),
    records_seen: 155,
    records_inserted: 5,
    records_skipped: 150,
  };
  return {
    session_id: sessionId,
    state: partial ? "partial" : "running",
    profile_name: session.profileName,
    locale: session.locale,
    mode: session.mode,
    records_count: partial ? 155 : 150,
    latest_records: records.slice(0, 5),
    counters: {
      packets_seen: 771,
      decoded_packets: 155,
      dropped_packets: 0,
    },
    started_at: Date.now() / 1000 - 6,
    updated_at: Date.now() / 1000,
    target: {
      pid: "1234",
      interface: "mock0",
      ports: [30230],
      bpf: "port 30230",
    },
    auto_page: partial
      ? {
          state: "partial",
          message: "capture stopped before packet drain completed",
          completed_pools: completedPools,
          skipped_pools: skippedPools,
          visited_pages: 160,
          visited_pages_by_pool: { limited: 55, standard: 60, fork: 45 },
          required_pages_by_pool: requiredPagesByPool,
          decoded_pages_by_pool: { limited: 53, standard: 59, fork: 43 },
          missing_pages_by_pool: missingPagesByPool,
          capture_drain_warning: null,
          last_page_by_pool: { limited: 55, standard: 60, fork: 45 },
          reason: {
            code: "capture_stopped_during_drain",
            message: "capture stopped before packet drain completed",
            retryable_with_slower_paging: true,
            support_path: "mock/support/capture-partial.json",
          },
        }
      : {
          state: "draining",
          message: "waiting for capture drain",
          kind: "draining",
          completed_pools: completedPools,
          skipped_pools: skippedPools,
          visited_pages: 160,
          visited_pages_by_pool: { limited: 55, standard: 60, fork: 45 },
          required_pages_by_pool: requiredPagesByPool,
          decoded_pages_by_pool: { limited: 53, standard: 59, fork: 43 },
          missing_pages_by_pool: missingPagesByPool,
          last_decoded_at: Date.now() / 1000,
        },
    raw_path: rawPath,
    error: null,
    import_report: partial ? report : null,
  };
}

function mockCaptureStalledStatus(
  sessionId: string,
  session: { profileName: string; locale: string; mode: CaptureMode },
): CaptureStatus {
  return {
    session_id: sessionId,
    state: "failed",
    profile_name: session.profileName,
    locale: session.locale,
    mode: session.mode,
    records_count: 0,
    latest_records: [],
    counters: {
      packets_seen: 8,
      decoded_packets: 1,
      dropped_packets: 0,
    },
    started_at: Date.now() / 1000 - 6,
    updated_at: Date.now() / 1000,
    target: {
      pid: "1234",
      interface: "mock0",
      ports: [30230],
      bpf: "port 30230",
    },
    auto_page: {
      state: "partial",
      message: "auto page ended with partial data",
      reason: {
        code: "automation_interrupted",
        message: "capture window stalled: pool=limited visited_pages=8 decoded_pages=1 max_visited_pages=7",
        retryable_with_slower_paging: true,
      },
      capture_drain_warning: null,
      completed_pools: [],
      skipped_pools: [],
      visited_pages: 7,
      visited_pages_by_pool: { limited: 7 },
      required_pages_by_pool: { limited: 7 },
      decoded_pages_by_pool: { limited: 1 },
      missing_pages_by_pool: { limited: 6 },
      last_page_by_pool: { limited: 7 },
    },
    raw_path: "data/runs/raw-mock.jsonl",
    error: {
      code: "auto_page_capture_window_stalled",
      message: "capture window stalled: pool=limited visited_pages=8 decoded_pages=1 max_visited_pages=7",
      support_path: "mock/support/capture-stalled.zip",
    },
    import_report: null,
  };
}

function mockCaptureSourceKind(mode: CaptureMode): string {
  switch (mode) {
    case "live_only":
      return "live_capture";
    case "auto_page_incremental":
      return "auto_page_capture";
    case "auto_page_full":
      return "auto_page_full";
  }
}

function mockDiagnosticStatus(sessionId: string): DiagnosticStatus {
  const session = mockDiagnosticSessions.get(sessionId);
  const completed = Boolean(session?.stopped || (session && session.polls >= 2));
  const duration = session?.duration ?? 20;
  const progress = completed ? 1 : session?.polls ? 0.45 : 0.08;
  return {
    session_id: sessionId,
    mode: session?.mode ?? "pktmon",
    state: completed ? "completed" : session?.polls ? "running" : "starting",
    started_at: Date.now() / 1000 - progress * duration,
    updated_at: Date.now() / 1000,
    duration_seconds: duration,
    elapsed_seconds: Math.round(progress * duration),
    stage: completed ? "completed" : session?.polls ? "capturing" : "preparing",
    progress,
    support_zip_path: completed ? "data/support/diagnostic-mock.zip" : null,
    error: null,
    summary: completed
      ? {
          verdict: "only_idle_packets",
          findings: ["external pktmon failed: mock browser mode"],
          packets_seen: 1682,
          decoded_packets: 0,
          dropped_packets: 890,
          duplicate_packets: 693,
          rows_count: 0,
          external_ok: false,
        }
      : null,
  };
}

function mockAssetDataUrl(assetRef: string, kind: string) {
  const hash = Array.from(`${kind}:${assetRef}`).reduce(
    (acc, char) => (acc * 31 + char.charCodeAt(0)) % 360,
    0,
  );
  const accent = (hash + 42) % 360;
  const label = (kind || "asset").slice(0, 10).toUpperCase();
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="256" height="256" viewBox="0 0 256 256"><defs><linearGradient id="g" x1="0" x2="1" y1="0" y2="1"><stop stop-color="hsl(${hash} 56% 42%)"/><stop offset="1" stop-color="hsl(${accent} 66% 64%)"/></linearGradient></defs><rect width="256" height="256" rx="18" fill="url(#g)"/><circle cx="184" cy="70" r="44" fill="rgba(255,255,255,.22)"/><path d="M36 202c24-52 58-78 102-78 35 0 62 18 82 54v24H36z" fill="rgba(255,255,255,.3)"/><text x="128" y="128" text-anchor="middle" dominant-baseline="middle" font-family="Arial, sans-serif" font-size="24" font-weight="700" fill="#fff">${label}</text></svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}
