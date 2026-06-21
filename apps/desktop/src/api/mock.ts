import type {
  AppApi,
  AssetResolveRequest,
  AssetResolveResult,
  AssetsPackPackage,
  AssetsPackCheckReport,
  AssetsPackInstallReport,
  AssetsPackStatus,
  CaptureMode,
  CaptureStatus,
  DashboardSelection,
  DashboardSelectionDetail,
  ImportReport,
  PoolKind,
  PoolKindSummary,
  RecordFilter,
  SettingsPatch,
  UpdatePackage,
  UpdateCheckReport,
  UpdateStageReport,
  UpdateStatus,
} from "./types";
import {
  mockBanners,
  mockCaptureSessions,
  mockFilterOptions,
  mockProfile,
  mockRecords,
  mockSummary,
  mockTimeStats,
} from "./mock-data";

const MOCK_APP_VERSION = __NTE_APP_VERSION__;
const mockProfiles = [{ ...mockProfile }];
let mockActiveProfileName = mockProfile.name;
let mockLocale = "zh-Hant";
let mockUiLocale = "zh-Hant";
let mockUpdateChannel = "stable";
let mockCheckUpdatesOnStartup = false;

function mockSelectionDetail(selection: DashboardSelection): DashboardSelectionDetail {
  const records = mockRecords.filter((record) =>
    selection.kind === "pool_kind"
      ? record.pool_kind === selection.pool_kind
      : record.pool_kind === selection.pool_kind && record.derived.banner_id === selection.banner_id,
  );
  const baseSummary =
    selection.kind === "pool_kind"
      ? mockSummary.find((item) => item.pool_kind === selection.pool_kind)
      : mockBanners.find((banner) => banner.banner_id === selection.banner_id);
  const label =
    selection.kind === "pool_kind"
      ? (baseSummary as PoolKindSummary | undefined)?.label
      : mockBanners.find((banner) => banner.banner_id === selection.banner_id)?.title;
  const poolKind = selection.pool_kind;
  const fallback = mockSummary.find((item) => item.pool_kind === poolKind) ?? mockSummary[0];
  const fiveStarRecords = records.filter((record) => record.derived.hit_rarity === 5);
  const fourStarRecords = records.filter((record) => record.derived.hit_rarity === 4);
  const summary: PoolKindSummary = {
    ...fallback,
    label: label ?? fallback.label,
    total_pulls: records.length,
    hit_count: fiveStarRecords.length,
    four_star_count: fourStarRecords.length,
    latest_5star: fiveStarRecords[0] ?? null,
  };
  const rarityCounts = new Map<number, number>();
  for (const record of records) {
    if (record.rarity != null) rarityCounts.set(record.rarity, (rarityCounts.get(record.rarity) ?? 0) + 1);
  }
  const knownTotal = [...rarityCounts.values()].reduce((total, count) => total + count, 0);
  const rarity_distribution = [...rarityCounts.entries()]
    .sort(([left], [right]) => right - left)
    .map(([rarity, count]) => ({ rarity, count, percent: knownTotal ? count / knownTotal : 0 }));
  const itemCounts = new Map<string, { item_name: string; rarity?: number | null; count: number }>();
  for (const record of records) {
    const entry = itemCounts.get(record.item_id) ?? {
      item_name: record.item_name,
      rarity: record.rarity,
      count: 0,
    };
    entry.count += 1;
    itemCounts.set(record.item_id, entry);
  }
  const item_ranking = [...itemCounts.entries()]
    .map(([item_id, item]) => ({ item_id, ...item }))
    .sort((left, right) => right.count - left.count || (right.rarity ?? 0) - (left.rarity ?? 0) || left.item_name.localeCompare(right.item_name))
    .slice(0, 20);

  return {
    summary,
    five_star_history: fiveStarRecords.map((record) => ({
      record,
      pity_distance: record.derived.pity_5_before + 1,
      result: record.derived.rate_up_result,
      guarantee_before: record.derived.guarantee_5_before,
      guarantee_after: record.derived.guarantee_5_after,
    })),
    four_star_history: fourStarRecords.map((record) => ({
      record,
      pity_distance: record.derived.pity_4_before + 1,
      result: record.derived.rate_up_result,
      guarantee_before: record.derived.guarantee_4_before,
      guarantee_after: record.derived.guarantee_4_after,
    })),
    rarity_distribution,
    item_ranking,
  };
}

export const mockApi: AppApi = {
  async getSettings() {
    return {
      active_profile: mockActiveProfileName,
      locale: mockLocale,
      ui_locale: mockUiLocale,
      update_channel: mockUpdateChannel,
      check_updates_on_startup: mockCheckUpdatesOnStartup,
    };
  },
  async updateSettings(patch: SettingsPatch) {
    mockActiveProfileName = patch.active_profile ?? mockActiveProfileName;
    mockLocale = patch.locale ?? mockLocale;
    mockUiLocale = patch.ui_locale ?? mockUiLocale;
    mockUpdateChannel = patch.update_channel ?? mockUpdateChannel;
    mockCheckUpdatesOnStartup = patch.check_updates_on_startup ?? mockCheckUpdatesOnStartup;
    return {
      active_profile: mockActiveProfileName,
      locale: mockLocale,
      ui_locale: mockUiLocale,
      update_channel: mockUpdateChannel,
      check_updates_on_startup: mockCheckUpdatesOnStartup,
    };
  },
  async listProfiles() {
    return mockProfiles.map((profile) => ({ ...profile, active: profile.name === mockActiveProfileName }));
  },
  async createProfile(name: string) {
    const profile = { name, created_at: "0", updated_at: "0", active: false };
    mockProfiles.push(profile);
    return profile;
  },
  async setActiveProfile(profileName: string) {
    mockActiveProfileName = profileName;
    return {
      active_profile: profileName,
      locale: mockLocale,
      ui_locale: mockUiLocale,
      update_channel: mockUpdateChannel,
      check_updates_on_startup: mockCheckUpdatesOnStartup,
    };
  },
  async renameProfile(oldName: string, newName: string) {
    const profile = mockProfiles.find((item) => item.name === oldName);
    if (!profile) throw new Error(`profile not found: ${oldName}`);
    profile.name = newName;
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
    return {
      active_profile: mockActiveProfileName,
      locale: mockLocale,
      ui_locale: mockUiLocale,
      update_channel: mockUpdateChannel,
      check_updates_on_startup: mockCheckUpdatesOnStartup,
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
          roll_points_total: 0,
          known_roll_point_records: 0,
          missing_roll_point_records: 0,
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
          not_applicable_rate_up_count: 0,
          unknown_rate_up_count: 0,
          observed_up_rate: null,
          latest_5star: null,
          current_4star_pity: 0,
          hard_pity_4: null,
          average_4star_pity: null,
          min_4star_pity: null,
          max_4star_pity: null,
          four_star_count: 0,
          rate_up_4_count: 0,
          off_rate_4_count: 0,
          not_applicable_rate_up_4_count: 0,
          unknown_rate_up_4_count: 0,
          rule_resolution_status: "fallback_pool_kind",
          average_roll_points_to_5star: null,
          average_roll_points_to_4star: null,
          roll_point_cost_samples_5star: 0,
          roll_point_cost_samples_4star: 0,
        },
      ],
      banners: mockBanners,
      time_stats: mockTimeStats,
      rarity_distribution: [
        { rarity: 5, count: 3, percent: 0.016 },
        { rarity: 4, count: 18, percent: 0.099 },
        { rarity: 3, count: 161, percent: 0.885 },
      ],
      item_ranking: [
        { item_id: "common_2", item_name: "Training Log", rarity: 3, count: 44 },
        { item_id: "rare_1", item_name: "Sigrid", rarity: 5, count: 2 },
      ],
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
      four_star_history: [],
    };
  },
  async dashboardSelectionDetail(_profileName: string, selection: DashboardSelection) {
    return mockSelectionDetail(selection);
  },
  async listRecords(_profileName: string, filter: RecordFilter) {
    const search = filter.search?.toLowerCase().trim();
    let records = mockRecords.filter((record) => {
      if (filter.pool_kind && record.pool_kind !== filter.pool_kind) return false;
      if (filter.pool_id && record.pool_id !== filter.pool_id) return false;
      if (filter.banner_id && record.derived.banner_id !== filter.banner_id) return false;
      if (filter.record_type && record.record_type !== filter.record_type) return false;
      if (filter.rarity && record.rarity !== filter.rarity) return false;
      if (filter.hit_rarity && record.derived.hit_rarity !== filter.hit_rarity) return false;
      if (filter.rate_up_result && record.derived.rate_up_result !== filter.rate_up_result) return false;
      if (filter.pity_5_min != null && record.derived.pity_5_before < filter.pity_5_min) return false;
      if (filter.pity_5_max != null && record.derived.pity_5_before > filter.pity_5_max) return false;
      if (filter.pity_4_min != null && record.derived.pity_4_before < filter.pity_4_min) return false;
      if (filter.pity_4_max != null && record.derived.pity_4_before > filter.pity_4_max) return false;
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
  async systemLocale() {
    return "zh-TW";
  },
  async doctorRun() {
    return { ok: true, exit_code: 0, lines: ["mock doctor ok"] };
  },
  async runtimePing() {
    return { ok: true, runtime: "rust" };
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
  async assetsPackStatus() {
    return {
      installed: true,
      compatible: true,
      current_app_version: MOCK_APP_VERSION,
      expected_map_hash: "mock-map-hash",
      installed_app_version: MOCK_APP_VERSION,
      installed_map_hash: "mock-map-hash",
      source_commit: "mock-source",
      file_count: 4,
      install_path: "mock/data/assets-pack/current",
    };
  },
  async assetsPackCheck() {
    return {
      current_app_version: MOCK_APP_VERSION,
      expected_map_hash: "mock-map-hash",
      channel: "stable",
      installed: true,
      compatible: true,
      package: null,
    };
  },
  async assetsPackDownloadAndInstall(packageInfo: AssetsPackPackage) {
    return {
      app_version: packageInfo.app_version,
      map_hash: packageInfo.map_hash,
      source_commit: packageInfo.source_commit,
      file_count: packageInfo.file_count,
      install_path: "mock/data/assets-pack/current",
    };
  },
  async assetsPackRemove() {
    return {
      installed: false,
      compatible: false,
      current_app_version: MOCK_APP_VERSION,
      expected_map_hash: "mock-map-hash",
      installed_app_version: null,
      installed_map_hash: null,
      source_commit: null,
      file_count: 0,
      install_path: "mock/data/assets-pack/current",
    };
  },
  async assetsResolveRefs(refs: AssetResolveRequest[]) {
    return refs.map((ref) => ({
      ...ref,
      url: mockAssetDataUrl(ref.asset_ref, ref.kind ?? "asset"),
    }));
  },
  async requestAdminCaptureStart() {
    return false;
  },
  async takePendingAdminCapture() {
    return null;
  },
  async captureStart(profileName: string, _locale?: string, mode: CaptureMode = "live_only") {
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
  const mode = session?.mode ?? "live_only";
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
