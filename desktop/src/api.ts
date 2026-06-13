import { invoke } from "@tauri-apps/api/core";

export type Profile = {
  id: number;
  name: string;
  created_at: string;
  updated_at: string;
};

export type ImportReport = {
  profile_id: number;
  run_id: number;
  source_kind: string;
  source_path?: string | null;
  records_seen: number;
  records_inserted: number;
  records_skipped: number;
};

export type PoolSummary = {
  pool_id: string;
  pool_name: string;
  group_label: string;
  record_count: number;
  hit_count: number;
  current_pity?: number | null;
  pity_limit?: number | null;
  rule_source?: string | null;
  last_time?: string | null;
  last_item_name?: string | null;
};

export type TypeSummary = {
  record_type: string;
  record_count: number;
};

export type TimelineBucket = {
  day: string;
  record_count: number;
};

export type LatestRecord = {
  record_id: string;
  record_type: string;
  time?: string | null;
  pool_id?: string | null;
  pool_name?: string | null;
  item_id: string;
  item_name?: string | null;
  count?: number | null;
  roll_label?: string | null;
};

export type DashboardSummary = {
  profile: Profile;
  total_records: number;
  pools: PoolSummary[];
  by_record_type: TypeSummary[];
  timeline: TimelineBucket[];
  latest_records: LatestRecord[];
};

export type RecordFilter = {
  pool_id?: string | null;
  record_type?: string | null;
  search?: string | null;
  limit?: number;
  offset?: number;
};

export type StoredRecord = {
  record_id: string;
  record_type: string;
  time?: string | null;
  pool_id?: string | null;
  pool_name?: string | null;
  item_id: string;
  item_name?: string | null;
  count?: number | null;
  roll_points?: number | null;
  roll_label?: string | null;
  secondary_item_id?: string | null;
  secondary_item_name?: string | null;
  secondary_count?: number | null;
};

export type RecordList = {
  total: number;
  records: StoredRecord[];
};

export type AppApi = {
  listProfiles(): Promise<Profile[]>;
  createProfile(name: string): Promise<Profile>;
  refreshRules(locale?: string): Promise<void>;
  importPublicJson(profileId: number, path: string): Promise<ImportReport>;
  importRawJsonl(profileId: number, path: string, locale?: string): Promise<ImportReport>;
  dashboardSummary(profileId: number): Promise<DashboardSummary>;
  listRecords(profileId: number, filter: RecordFilter): Promise<RecordList>;
  exportProfileJson(profileId: number, path: string): Promise<void>;
  exportProfileCsv(profileId: number, path: string): Promise<void>;
  sidecarPing(): Promise<unknown>;
};

const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

const mockProfile: Profile = {
  id: 1,
  name: "Default",
  created_at: "0",
  updated_at: "0",
};

const mockRecords: StoredRecord[] = [
  {
    record_id: "mock:4",
    record_type: "gacha",
    time: "2026-01-09 21:40:00",
    pool_id: "CardPool_Character",
    pool_name: "Limited Signal",
    item_id: "rare_1",
    item_name: "Sigrid",
    count: 1,
    roll_label: "74",
  },
  {
    record_id: "mock:3",
    record_type: "gacha",
    time: "2026-01-08 19:22:00",
    pool_id: "CardPool_Character",
    pool_name: "Limited Signal",
    item_id: "common_2",
    item_name: "Training Log",
    count: 1,
    roll_label: "73",
  },
  {
    record_id: "mock:2",
    record_type: "gacha",
    time: "2026-01-07 20:11:00",
    pool_id: "CardPool_Weapon",
    pool_name: "Arc Calibration",
    item_id: "weapon_1",
    item_name: "Vector Blade",
    count: 1,
    roll_label: "12",
  },
  {
    record_id: "mock:1",
    record_type: "gacha",
    time: "2026-01-06 18:10:00",
    pool_id: "CardPool_Character",
    pool_name: "Limited Signal",
    item_id: "common_1",
    item_name: "Field Module",
    count: 1,
    roll_label: "72",
  },
];

const mockApi: AppApi = {
  async listProfiles() {
    return [mockProfile];
  },
  async createProfile(name: string) {
    return { ...mockProfile, id: Date.now(), name };
  },
  async refreshRules() {
    return undefined;
  },
  async importPublicJson(profileId: number, path: string) {
    return mockReport(profileId, "json", path);
  },
  async importRawJsonl(profileId: number, path: string) {
    return mockReport(profileId, "raw_jsonl", path);
  },
  async dashboardSummary() {
    return {
      profile: mockProfile,
      total_records: 182,
      pools: [
        {
          pool_id: "CardPool_Character",
          pool_name: "Limited Signal",
          group_label: "Limited",
          record_count: 146,
          hit_count: 2,
          current_pity: 73,
          pity_limit: 80,
          rule_source: "mock",
          last_time: "2026-01-09 21:40:00",
          last_item_name: "Sigrid",
        },
        {
          pool_id: "CardPool_Weapon",
          pool_name: "Arc Calibration",
          group_label: "Weapon",
          record_count: 36,
          hit_count: 1,
          current_pity: 12,
          pity_limit: 80,
          rule_source: "mock",
          last_time: "2026-01-07 20:11:00",
          last_item_name: "Vector Blade",
        },
      ],
      by_record_type: [{ record_type: "gacha", record_count: 182 }],
      timeline: [
        { day: "2026-01-06", record_count: 18 },
        { day: "2026-01-07", record_count: 28 },
        { day: "2026-01-08", record_count: 51 },
        { day: "2026-01-09", record_count: 85 },
      ],
      latest_records: mockRecords,
    };
  },
  async listRecords(_profileId: number, filter: RecordFilter) {
    const search = filter.search?.toLowerCase().trim();
    const records = mockRecords.filter((record) => {
      if (filter.pool_id && record.pool_id !== filter.pool_id) return false;
      if (search && !`${record.item_name} ${record.item_id}`.toLowerCase().includes(search)) return false;
      return true;
    });
    return { total: records.length, records };
  },
  async exportProfileJson() {
    return undefined;
  },
  async exportProfileCsv() {
    return undefined;
  },
  async sidecarPing() {
    return { ok: true, mock: true };
  },
};

const tauriApi: AppApi = {
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  createProfile: (name) => invoke<Profile>("create_profile", { name }),
  refreshRules: (locale) => invoke<void>("refresh_rules", { locale }),
  importPublicJson: (profileId, path) =>
    invoke<ImportReport>("import_public_json", { profileId, path }),
  importRawJsonl: (profileId, path, locale) =>
    invoke<ImportReport>("import_raw_jsonl", { profileId, path, locale }),
  dashboardSummary: (profileId) => invoke<DashboardSummary>("dashboard_summary", { profileId }),
  listRecords: (profileId, filter) => invoke<RecordList>("list_records", { profileId, filter }),
  exportProfileJson: (profileId, path) => invoke<void>("export_profile_json", { profileId, path }),
  exportProfileCsv: (profileId, path) => invoke<void>("export_profile_csv", { profileId, path }),
  sidecarPing: () => invoke<unknown>("sidecar_ping"),
};

function mockReport(profileId: number, sourceKind: string, sourcePath: string): ImportReport {
  return {
    profile_id: profileId,
    run_id: Date.now(),
    source_kind: sourceKind,
    source_path: sourcePath,
    records_seen: 4,
    records_inserted: 4,
    records_skipped: 0,
  };
}

export const api: AppApi = isTauri() ? tauriApi : mockApi;

