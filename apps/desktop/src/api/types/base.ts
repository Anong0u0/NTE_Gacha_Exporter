export type PoolKind = "monopoly_limited" | "monopoly_standard" | "fork_lottery";
export type AssetRefs = Record<string, unknown>;
export type SortDirection = "asc" | "desc";
export type CaptureMode = "live_only" | "auto_page_incremental" | "auto_page_full";
export type AboutLinkTarget = "github" | "discord";

export type Settings = {
  active_profile: string;
  locale: string;
  ui_locale: string;
  update_channel: string;
  check_updates_on_startup: boolean;
  skipped_update_version?: string | null;
  capture_auto_page_enabled: boolean;
  capture_full_update_enabled: boolean;
  capture_windivert_backend_enabled: boolean;
};

export type SettingsPatch = {
  active_profile?: string | null;
  locale?: string | null;
  ui_locale?: string | null;
  update_channel?: string | null;
  check_updates_on_startup?: boolean | null;
  skipped_update_version?: string | null;
  capture_auto_page_enabled?: boolean | null;
  capture_full_update_enabled?: boolean | null;
  capture_windivert_backend_enabled?: boolean | null;
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

export type DoctorReport = {
  ok: boolean;
  exit_code: number;
  lines: string[];
};

export type MapLocaleList = {
  locales: string[];
};
