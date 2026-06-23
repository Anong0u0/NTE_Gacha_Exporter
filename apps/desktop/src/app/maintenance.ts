import type { Ref } from "vue";
import {
  api,
  type AssetsPackCheckReport,
  type AssetsPackInstallReport,
  type AssetsPackStatus,
  type DoctorReport,
  type UpdateCheckReport,
  type UpdateStageReport,
  type UpdateStatus,
} from "../api";
import type { I18nKey } from "./i18n";

type MaintenanceDeps = {
  doctorReport: Ref<DoctorReport | null>;
  updateStatus: Ref<UpdateStatus | null>;
  updateCheckReport: Ref<UpdateCheckReport | null>;
  stagedUpdate: Ref<UpdateStageReport | null>;
  assetsPackStatus: Ref<AssetsPackStatus | null>;
  assetsPackCheckReport: Ref<AssetsPackCheckReport | null>;
  lastAssetsPackInstall: Ref<AssetsPackInstallReport | null>;
  assetUrlCache: Ref<Record<string, string>>;
  settingsUpdateChannel: Ref<string>;
  statusText: Ref<string>;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
  resolveVisibleAssets(): Promise<void>;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
};

export function createMaintenanceActions(deps: MaintenanceDeps) {
  async function runDoctor() {
    await deps.runTask(deps.t("status.doctorCompleted"), async () => {
      deps.doctorReport.value = await api.doctorRun();
    });
  }

  async function loadUpdaterStatus() {
    deps.updateStatus.value = await api.updaterStatus();
  }

  async function checkForUpdates(showStatus = true) {
    await deps.runTask(showStatus ? deps.t("status.updateCheckCompleted") : deps.statusText.value, async () => {
      deps.updateCheckReport.value = await api.updaterCheck(deps.settingsUpdateChannel.value);
      await loadUpdaterStatus();
    });
  }

  async function downloadUpdate() {
    const packageInfo = deps.updateCheckReport.value?.package;
    if (!packageInfo) return;
    await deps.runTask(deps.t("status.updateDownloaded"), async () => {
      deps.stagedUpdate.value = await api.updaterDownloadAndStage(packageInfo);
      await loadUpdaterStatus();
    });
  }

  async function installUpdate() {
    const version = deps.stagedUpdate.value?.package.version ?? deps.updateStatus.value?.staged_version;
    if (version) await deps.runTask(deps.t("status.updateRestarting"), () => api.updaterInstallStaged(version, true));
  }

  async function loadAssetsPackStatus() {
    deps.assetsPackStatus.value = await api.assetsPackStatus();
  }

  async function checkAssetsPack() {
    await deps.runTask(deps.t("status.assetsCheckCompleted"), async () => {
      deps.assetsPackCheckReport.value = await api.assetsPackCheck(deps.settingsUpdateChannel.value);
      await loadAssetsPackStatus();
    });
  }

  async function downloadAssetsPack() {
    const packageInfo = deps.assetsPackCheckReport.value?.package;
    if (!packageInfo) return;
    await deps.runTask(deps.t("status.assetsInstalled"), async () => {
      deps.lastAssetsPackInstall.value = await api.assetsPackDownloadAndInstall(packageInfo);
      deps.assetUrlCache.value = {};
      await loadAssetsPackStatus();
      await deps.resolveVisibleAssets();
    });
  }

  async function removeAssetsPack() {
    await deps.runTask(deps.t("status.assetsRemoved"), async () => {
      deps.assetsPackStatus.value = await api.assetsPackRemove();
      deps.assetsPackCheckReport.value = null;
      deps.lastAssetsPackInstall.value = null;
      deps.assetUrlCache.value = {};
    });
  }

  return {
    runDoctor,
    loadUpdaterStatus,
    checkForUpdates,
    downloadUpdate,
    installUpdate,
    loadAssetsPackStatus,
    checkAssetsPack,
    downloadAssetsPack,
    removeAssetsPack,
  };
}
