import type { Ref } from "vue";
import {
  api,
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
  settingsUpdateChannel: Ref<string>;
  statusText: Ref<string>;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
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

  return {
    runDoctor,
    loadUpdaterStatus,
    checkForUpdates,
    downloadUpdate,
    installUpdate,
  };
}
