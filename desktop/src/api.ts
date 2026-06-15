import { invoke } from "@tauri-apps/api/core";

export type PoolKind = "monopoly_limited" | "monopoly_standard" | "fork_lottery";
export type RecordSortKey = "time" | "pool" | "item" | "rarity" | "record_type";
export type SortDirection = "asc" | "desc";
export type CaptureMode = "live_only" | "auto_page_incremental" | "auto_page_full";

export type Settings = {
  active_profile: string;
  locale: string;
  update_channel: string;
  check_updates_on_startup: boolean;
};

export type SettingsPatch = {
  active_profile?: string | null;
  locale?: string | null;
  update_channel?: string | null;
  check_updates_on_startup?: boolean | null;
};

export type Profile = {
  name: string;
  created_at: string;
  updated_at: string;
  active: boolean;
};

export type ImportReport = {
  profile_name: string;
  source_kind: string;
  source_path?: string | null;
  records_seen: number;
  records_inserted: number;
  records_skipped: number;
  completed_at: string;
};

export type BackupReport = {
  path: string;
  profile_count: number;
  record_count: number;
  created_at: string;
};

export type RestoreReport = {
  source_path: string;
  profiles_seen: number;
  profiles_created: number;
  profiles_merged: number;
  records_seen: number;
  records_inserted: number;
  records_skipped: number;
  settings_restored: boolean;
  completed_at: string;
};

export type CaptureCounters = {
  packets_seen: number;
  decoded_packets: number;
  dropped_packets: number;
};

export type CaptureTarget = {
  pid?: string;
  interface?: string;
  ports?: number[];
  bpf?: string;
};

export type AutoPageStatus = {
  state: string;
  message: string;
  kind: string;
  step?: string | null;
  pool?: string | null;
  current_page?: number | null;
  total_pages?: number | null;
  technical_detail?: string | null;
  completed_pools?: string[];
  skipped_pools?: string[];
};

export type CaptureStatus = {
  session_id: string;
  state: "starting" | "running" | "stopping" | "completed" | "failed" | string;
  mode: CaptureMode | string;
  records_count: number;
  latest_records: Record<string, unknown>[];
  counters: CaptureCounters;
  started_at: number;
  updated_at: number;
  target?: CaptureTarget | null;
  auto_page?: AutoPageStatus | null;
  raw_path?: string | null;
  error?: { code: string; message: string } | null;
  import_report?: ImportReport | null;
};

export type DisplayRecord = {
  record_id: string;
  record_type: string;
  time?: string | null;
  pool_kind: PoolKind;
  pool_id: string;
  pool_label: string;
  item_id: string;
  item_name: string;
  rarity?: number | null;
  count?: number | null;
  roll_points?: number | null;
  secondary_item_id?: string | null;
  secondary_item_name?: string | null;
  secondary_count?: number | null;
};

export type PoolKindSummary = {
  pool_kind: PoolKind;
  label: string;
  total_pulls: number;
  hit_count: number;
  current_pity: number;
  current_guarantee: boolean;
  hard_pity: number;
  average_5star_pity?: number | null;
  min_5star_pity?: number | null;
  max_5star_pity?: number | null;
  early_hit_count: number;
  up_count: number;
  off_rate_count: number;
  observed_up_rate?: number | null;
  latest_5star?: DisplayRecord | null;
};

export type RarityBucket = {
  rarity: number;
  count: number;
  percent: number;
};

export type ItemRank = {
  item_id: string;
  item_name: string;
  rarity?: number | null;
  count: number;
};

export type DashboardOverview = {
  profile: Profile;
  last_run?: ImportReport | null;
  total_records: number;
  pool_kinds: PoolKindSummary[];
  rarity_distribution: RarityBucket[];
  item_ranking: ItemRank[];
  latest_records: DisplayRecord[];
};

export type FiveStarRecord = {
  record: DisplayRecord;
  pity_distance: number;
  result: "up" | "off_rate";
  guarantee_before: boolean;
  guarantee_after: boolean;
};

export type PoolKindDetail = {
  summary: PoolKindSummary;
  five_star_history: FiveStarRecord[];
};

export type RecordFilter = {
  pool_kind?: PoolKind | null;
  pool_id?: string | null;
  record_type?: string | null;
  date_from?: string | null;
  date_to?: string | null;
  search?: string | null;
  sort_key?: RecordSortKey | null;
  sort_direction?: SortDirection | null;
  limit?: number;
  offset?: number;
};

export type RecordList = {
  total: number;
  records: DisplayRecord[];
};

export type RecordPoolOption = {
  pool_id: string;
  pool_kind: PoolKind;
  label: string;
  count: number;
};

export type RecordTypeOption = {
  record_type: string;
  count: number;
};

export type RecordFilterOptions = {
  pools: RecordPoolOption[];
  record_types: RecordTypeOption[];
};

export type DoctorReport = {
  ok: boolean;
  exit_code: number;
  lines: string[];
};

export type MapLocaleList = {
  locales: string[];
};

export type UpdateChannel = "stable" | "beta";

export type UpdatePackage = {
  version: string;
  channel: UpdateChannel;
  release_url: string;
  asset_name: string;
  download_url: string;
  sha256: string;
  size: number;
};

export type UpdateStatus = {
  portable_root: string;
  current_version: string;
  supported_layout: boolean;
  staged_version?: string | null;
  rollback_version?: string | null;
};

export type UpdateCheckReport = {
  current_version: string;
  channel: UpdateChannel;
  available: boolean;
  package?: UpdatePackage | null;
};

export type UpdateStageReport = {
  package: UpdatePackage;
  archive_path: string;
  staging_path: string;
};

export type AppApi = {
  getSettings(): Promise<Settings>;
  updateSettings(patch: SettingsPatch): Promise<Settings>;
  listProfiles(): Promise<Profile[]>;
  createProfile(name: string): Promise<Profile>;
  setActiveProfile(profileName: string): Promise<Settings>;
  importPublicJson(profileName: string, path: string): Promise<ImportReport>;
  importRawJsonl(profileName: string, path: string, locale?: string): Promise<ImportReport>;
  dashboardOverview(profileName: string, locale?: string): Promise<DashboardOverview>;
  poolKindDetail(profileName: string, poolKind: PoolKind, locale?: string): Promise<PoolKindDetail>;
  listRecords(profileName: string, filter: RecordFilter, locale?: string): Promise<RecordList>;
  recordFilterOptions(profileName: string, locale?: string): Promise<RecordFilterOptions>;
  exportPublicJson(profileName: string, path: string, locale?: string): Promise<void>;
  exportCsv(profileName: string, path: string, locale?: string): Promise<void>;
  createBackup(path?: string | null): Promise<BackupReport>;
  restoreBackup(path: string): Promise<RestoreReport>;
  mapsList(): Promise<MapLocaleList>;
  doctorRun(): Promise<DoctorReport>;
  sidecarPing(): Promise<unknown>;
  updaterStatus(): Promise<UpdateStatus>;
  updaterCheck(channel?: string): Promise<UpdateCheckReport>;
  updaterDownloadAndStage(packageInfo: UpdatePackage): Promise<UpdateStageReport>;
  updaterInstallStaged(version: string, relaunch?: boolean): Promise<void>;
  captureStart(profileName: string, locale?: string, mode?: CaptureMode): Promise<CaptureStatus>;
  captureStatus(sessionId: string): Promise<CaptureStatus>;
  captureStop(sessionId: string): Promise<CaptureStatus>;
};

const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

const mockProfile: Profile = {
  name: "default",
  created_at: "0",
  updated_at: "0",
  active: true,
};

const mockRecords: DisplayRecord[] = [
  {
    record_id: "mock-4",
    record_type: "monopoly",
    time: "2026-01-09 21:40:00",
    pool_kind: "monopoly_limited",
    pool_id: "CardPool_Character",
    pool_label: "Limited Board",
    item_id: "rare_1",
    item_name: "Sigrid",
    rarity: 5,
    count: 1,
    roll_points: 74,
  },
  {
    record_id: "mock-3",
    record_type: "monopoly",
    time: "2026-01-08 19:22:00",
    pool_kind: "monopoly_limited",
    pool_id: "CardPool_Character",
    pool_label: "Limited Board",
    item_id: "common_2",
    item_name: "Training Log",
    rarity: 3,
    count: 1,
    roll_points: 73,
  },
  {
    record_id: "mock-2",
    record_type: "fork",
    time: "2026-01-07 20:11:00",
    pool_kind: "fork_lottery",
    pool_id: "ForkLottery_AnHunQu",
    pool_label: "Arc Research",
    item_id: "fork_1",
    item_name: "Rose",
    rarity: 5,
    count: 1,
  },
];

const mockFilterOptions: RecordFilterOptions = {
  pools: [
    { pool_id: "CardPool_Character", pool_kind: "monopoly_limited", label: "Limited Board", count: 146 },
    { pool_id: "ForkLottery_AnHunQu", pool_kind: "fork_lottery", label: "Arc Research", count: 36 },
  ],
  record_types: [
    { record_type: "monopoly", count: 146 },
    { record_type: "fork", count: 36 },
  ],
};

const mockCaptureSessions = new Map<
  string,
  { profileName: string; polls: number; stopped: boolean; mode: CaptureMode }
>();

const mockSummary: PoolKindSummary[] = [
  {
    pool_kind: "monopoly_limited",
    label: "Limited Board",
    total_pulls: 146,
    hit_count: 2,
    current_pity: 73,
    current_guarantee: false,
    hard_pity: 90,
    average_5star_pity: 72.5,
    min_5star_pity: 71,
    max_5star_pity: 74,
    early_hit_count: 2,
    up_count: 2,
    off_rate_count: 0,
    observed_up_rate: 1,
    latest_5star: mockRecords[0],
  },
  {
    pool_kind: "fork_lottery",
    label: "Arc Research",
    total_pulls: 36,
    hit_count: 1,
    current_pity: 12,
    current_guarantee: false,
    hard_pity: 80,
    average_5star_pity: 24,
    min_5star_pity: 24,
    max_5star_pity: 24,
    early_hit_count: 1,
    up_count: 1,
    off_rate_count: 0,
    observed_up_rate: 1,
    latest_5star: mockRecords[2],
  },
];

const mockApi: AppApi = {
  async getSettings() {
    return {
      active_profile: "default",
      locale: "zh-Hant",
      update_channel: "stable",
      check_updates_on_startup: false,
    };
  },
  async updateSettings(patch: SettingsPatch) {
    return {
      active_profile: patch.active_profile ?? "default",
      locale: patch.locale ?? "zh-Hant",
      update_channel: patch.update_channel ?? "stable",
      check_updates_on_startup: patch.check_updates_on_startup ?? false,
    };
  },
  async listProfiles() {
    return [mockProfile];
  },
  async createProfile(name: string) {
    return { name, created_at: "0", updated_at: "0", active: false };
  },
  async setActiveProfile(profileName: string) {
    return {
      active_profile: profileName,
      locale: "zh-Hant",
      update_channel: "stable",
      check_updates_on_startup: false,
    };
  },
  async importPublicJson(profileName: string, path: string) {
    return mockReport(profileName, "public_json", path);
  },
  async importRawJsonl(profileName: string, path: string) {
    return mockReport(profileName, "raw_jsonl", path);
  },
  async dashboardOverview() {
    return {
      profile: mockProfile,
      last_run: mockReport("default", "raw_jsonl", "sample.raw.jsonl"),
      total_records: 182,
      pool_kinds: [
        ...mockSummary,
        {
          pool_kind: "monopoly_standard" as const,
          label: "Standard Board",
          total_pulls: 0,
          hit_count: 0,
          current_pity: 0,
          current_guarantee: false,
          hard_pity: 90,
          average_5star_pity: null,
          min_5star_pity: null,
          max_5star_pity: null,
          early_hit_count: 0,
          up_count: 0,
          off_rate_count: 0,
          observed_up_rate: null,
          latest_5star: null,
        },
      ],
      rarity_distribution: [
        { rarity: 5, count: 3, percent: 0.016 },
        { rarity: 4, count: 18, percent: 0.099 },
        { rarity: 3, count: 161, percent: 0.885 },
      ],
      item_ranking: [
        { item_id: "common_2", item_name: "Training Log", rarity: 3, count: 44 },
        { item_id: "rare_1", item_name: "Sigrid", rarity: 5, count: 2 },
      ],
      latest_records: mockRecords,
    };
  },
  async poolKindDetail(_profileName: string, poolKind: PoolKind) {
    const summary = mockSummary.find((item) => item.pool_kind === poolKind) ?? mockSummary[0];
    return {
      summary,
      five_star_history: summary.latest_5star
        ? [
            {
              record: summary.latest_5star,
              pity_distance: Math.round(summary.average_5star_pity ?? 0),
              result: "up",
              guarantee_before: false,
              guarantee_after: false,
            },
          ]
        : [],
    };
  },
  async listRecords(_profileName: string, filter: RecordFilter) {
    const search = filter.search?.toLowerCase().trim();
    let records = mockRecords.filter((record) => {
      if (filter.pool_kind && record.pool_kind !== filter.pool_kind) return false;
      if (filter.pool_id && record.pool_id !== filter.pool_id) return false;
      if (filter.record_type && record.record_type !== filter.record_type) return false;
      if (search && !`${record.item_name} ${record.item_id}`.toLowerCase().includes(search)) return false;
      return true;
    });
    records = [...records].sort((left, right) => String(right.time ?? "").localeCompare(String(left.time ?? "")));
    const offset = filter.offset ?? 0;
    const limit = filter.limit ?? 50;
    return { total: records.length, records: records.slice(offset, offset + limit) };
  },
  async recordFilterOptions() {
    return mockFilterOptions;
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
      record_count: mockRecords.length,
      created_at: String(Date.now()),
    };
  },
  async restoreBackup(path: string) {
    return {
      source_path: path,
      profiles_seen: 1,
      profiles_created: 0,
      profiles_merged: 1,
      records_seen: mockRecords.length,
      records_inserted: 1,
      records_skipped: 1,
      settings_restored: true,
      completed_at: String(Date.now()),
    };
  },
  async mapsList() {
    return { locales: ["zh-Hant", "en", "ja"] };
  },
  async doctorRun() {
    return { ok: true, exit_code: 0, lines: ["mock doctor ok"] };
  },
  async sidecarPing() {
    return { ok: true };
  },
  async updaterStatus() {
    return {
      portable_root: "mock-root",
      current_version: "0.1.0",
      supported_layout: true,
      staged_version: null,
      rollback_version: null,
    };
  },
  async updaterCheck() {
    return {
      current_version: "0.1.0",
      channel: "stable",
      available: false,
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
  async captureStart(profileName: string, _locale?: string, mode: CaptureMode = "auto_page_incremental") {
    const sessionId = `mock-capture-${Date.now()}`;
    mockCaptureSessions.set(sessionId, { profileName, polls: 0, stopped: false, mode });
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
};

const tauriApi: AppApi = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch) => invoke<Settings>("update_settings", { patch }),
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  createProfile: (name) => invoke<Profile>("create_profile", { name }),
  setActiveProfile: (profileName) => invoke<Settings>("set_active_profile", { profileName }),
  importPublicJson: (profileName, path) => invoke<ImportReport>("import_public_json", { profileName, path }),
  importRawJsonl: (profileName, path, locale) =>
    invoke<ImportReport>("import_raw_jsonl", { profileName, path, locale }),
  dashboardOverview: (profileName, locale) => invoke<DashboardOverview>("dashboard_overview", { profileName, locale }),
  poolKindDetail: (profileName, poolKind, locale) =>
    invoke<PoolKindDetail>("pool_kind_detail", { profileName, poolKind, locale }),
  listRecords: (profileName, filter, locale) => invoke<RecordList>("list_records", { profileName, filter, locale }),
  recordFilterOptions: (profileName, locale) =>
    invoke<RecordFilterOptions>("record_filter_options", { profileName, locale }),
  exportPublicJson: (profileName, path, locale) =>
    invoke<void>("export_public_json", { profileName, path, locale }),
  exportCsv: (profileName, path, locale) => invoke<void>("export_csv", { profileName, path, locale }),
  createBackup: (path) => invoke<BackupReport>("create_backup", { path }),
  restoreBackup: (path) => invoke<RestoreReport>("restore_backup", { path }),
  mapsList: () => invoke<MapLocaleList>("maps_list"),
  doctorRun: () => invoke<DoctorReport>("doctor_run"),
  sidecarPing: () => invoke<unknown>("sidecar_ping"),
  updaterStatus: () => invoke<UpdateStatus>("updater_status"),
  updaterCheck: (channel) => invoke<UpdateCheckReport>("updater_check", { channel }),
  updaterDownloadAndStage: (packageInfo) =>
    invoke<UpdateStageReport>("updater_download_and_stage", { package: packageInfo }),
  updaterInstallStaged: (version, relaunch) =>
    invoke<void>("updater_install_staged", { version, relaunch }),
  captureStart: (profileName, locale, mode) => invoke<CaptureStatus>("capture_start", { profileName, locale, mode }),
  captureStatus: (sessionId) => invoke<CaptureStatus>("capture_status", { sessionId }),
  captureStop: (sessionId) => invoke<CaptureStatus>("capture_stop", { sessionId }),
};

function mockReport(profileName: string, sourceKind: string, sourcePath: string): ImportReport {
  return {
    profile_name: profileName,
    source_kind: sourceKind,
    source_path: sourcePath,
    records_seen: mockRecords.length,
    records_inserted: mockRecords.length,
    records_skipped: 0,
    completed_at: String(Date.now()),
  };
}

function mockCaptureStatus(sessionId: string): CaptureStatus {
  const session = mockCaptureSessions.get(sessionId);
  const completed = Boolean(session?.stopped || (session && session.polls >= 2));
  const profileName = session?.profileName ?? "default";
  const mode = session?.mode ?? "auto_page_incremental";
  const recordsCount = completed ? mockRecords.length : Math.min(2, Math.max(0, session?.polls ?? 0));
  return {
    session_id: sessionId,
    state: completed ? "completed" : session?.polls ? "running" : "starting",
    mode,
    records_count: recordsCount,
    latest_records: mockRecords.slice(0, recordsCount),
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
    auto_page:
      mode === "live_only"
        ? null
        : {
            state: completed ? "completed" : "running",
            message: completed ? "auto page completed" : session?.polls ? "page next" : "auto page started",
            kind: completed ? "completed" : "page",
            pool: session?.polls ? "limited" : null,
            current_page: session?.polls ? Math.min(2, session.polls + 1) : null,
            total_pages: 3,
            completed_pools: completed ? ["limited", "standard"] : [],
            skipped_pools: mode === "auto_page_incremental" && completed ? ["fork"] : [],
          },
    raw_path: mode === "live_only" ? null : "data/runs/raw-mock.jsonl",
    error: null,
    import_report: completed ? mockReport(profileName, mode === "live_only" ? "live_capture" : mode, "") : null,
  };
}

export const api: AppApi = isTauri() ? tauriApi : mockApi;
