import { invoke } from "@tauri-apps/api/core";
import type {
  AppApi,
  AssetResolveResult,
  AssetsPackCheckReport,
  AssetsPackInstallReport,
  AssetsPackStatus,
  BackupReport,
  CaptureStatus,
  DashboardOverview,
  DoctorReport,
  ImportReport,
  MapLocaleList,
  PendingAdminCapture,
  PoolKindDetail,
  Profile,
  RecordFilterOptions,
  RecordList,
  RestoreReport,
  Settings,
  UpdateCheckReport,
  UpdateStageReport,
  UpdateStatus,
} from "./types";

export const tauriApi: AppApi = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (patch) => invoke<Settings>("update_settings", { patch }),
  listProfiles: () => invoke<Profile[]>("list_profiles"),
  createProfile: (name) => invoke<Profile>("create_profile", { name }),
  setActiveProfile: (profileName) => invoke<Settings>("set_active_profile", { profileName }),
  renameProfile: (oldName, newName) => invoke<Profile>("rename_profile", { oldName, newName }),
  deleteProfile: (profileName) => invoke<Settings>("delete_profile", { profileName }),
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
  runtimePing: () => invoke<unknown>("runtime_ping"),
  updaterStatus: () => invoke<UpdateStatus>("updater_status"),
  updaterCheck: (channel) => invoke<UpdateCheckReport>("updater_check", { channel }),
  updaterDownloadAndStage: (packageInfo) =>
    invoke<UpdateStageReport>("updater_download_and_stage", { package: packageInfo }),
  updaterInstallStaged: (version, relaunch) =>
    invoke<void>("updater_install_staged", { version, relaunch }),
  assetsPackStatus: () => invoke<AssetsPackStatus>("assets_pack_status"),
  assetsPackCheck: (channel) => invoke<AssetsPackCheckReport>("assets_pack_check", { channel }),
  assetsPackDownloadAndInstall: (packageInfo) =>
    invoke<AssetsPackInstallReport>("assets_pack_download_and_install", { package: packageInfo }),
  assetsPackRemove: () => invoke<AssetsPackStatus>("assets_pack_remove"),
  assetsResolveRefs: (refs) => invoke<AssetResolveResult[]>("assets_resolve_refs", { refs }),
  requestAdminCaptureStart: (profileName, locale, mode) =>
    invoke<boolean>("request_admin_capture_start", { profileName, locale, mode }),
  takePendingAdminCapture: () => invoke<PendingAdminCapture | null>("take_pending_admin_capture"),
  captureStart: (profileName, locale, mode) => invoke<CaptureStatus>("capture_start", { profileName, locale, mode }),
  captureStatus: (sessionId) => invoke<CaptureStatus>("capture_status", { sessionId }),
  captureStop: (sessionId) => invoke<CaptureStatus>("capture_stop", { sessionId }),
};
