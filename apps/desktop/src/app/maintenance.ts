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
};

export function createMaintenanceActions(deps: MaintenanceDeps) {
  async function pingRuntime() {
    await deps.runTask("Runtime responded", () => api.runtimePing());
  }

  async function runDoctor() {
    await deps.runTask("Doctor completed", async () => {
      deps.doctorReport.value = await api.doctorRun();
    });
  }

  async function loadUpdaterStatus() {
    deps.updateStatus.value = await api.updaterStatus();
  }

  async function checkForUpdates(showStatus = true) {
    await deps.runTask(showStatus ? "Update check completed" : deps.statusText.value, async () => {
      deps.updateCheckReport.value = await api.updaterCheck(deps.settingsUpdateChannel.value);
      await loadUpdaterStatus();
    });
  }

  async function downloadUpdate() {
    const packageInfo = deps.updateCheckReport.value?.package;
    if (!packageInfo) return;
    await deps.runTask("Update downloaded", async () => {
      deps.stagedUpdate.value = await api.updaterDownloadAndStage(packageInfo);
      await loadUpdaterStatus();
    });
  }

  async function installUpdate() {
    const version = deps.stagedUpdate.value?.package.version ?? deps.updateStatus.value?.staged_version;
    if (version) await deps.runTask("Restarting for update", () => api.updaterInstallStaged(version, true));
  }

  async function loadAssetsPackStatus() {
    deps.assetsPackStatus.value = await api.assetsPackStatus();
  }

  async function checkAssetsPack() {
    await deps.runTask("Assets pack check completed", async () => {
      deps.assetsPackCheckReport.value = await api.assetsPackCheck(deps.settingsUpdateChannel.value);
      await loadAssetsPackStatus();
    });
  }

  async function downloadAssetsPack() {
    const packageInfo = deps.assetsPackCheckReport.value?.package;
    if (!packageInfo) return;
    await deps.runTask("Assets pack installed", async () => {
      deps.lastAssetsPackInstall.value = await api.assetsPackDownloadAndInstall(packageInfo);
      deps.assetUrlCache.value = {};
      await loadAssetsPackStatus();
      await deps.resolveVisibleAssets();
    });
  }

  async function removeAssetsPack() {
    await deps.runTask("Assets pack removed", async () => {
      deps.assetsPackStatus.value = await api.assetsPackRemove();
      deps.assetsPackCheckReport.value = null;
      deps.lastAssetsPackInstall.value = null;
      deps.assetUrlCache.value = {};
    });
  }

  return {
    pingRuntime,
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
