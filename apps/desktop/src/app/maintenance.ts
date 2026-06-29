import type { Ref } from "vue";
import {
  api,
  type Settings,
  type UpdateCheckReport,
  type UpdateStatus,
} from "../api";
import type { I18nKey } from "./i18n";

type MaintenanceDeps = {
  updateStatus: Ref<UpdateStatus | null>;
  updateCheckReport: Ref<UpdateCheckReport | null>;
  settingsUpdateChannel: Ref<string>;
  settingsSkippedUpdateVersion: Ref<string | null>;
  updatePromptOpen: Ref<boolean>;
  dismissedUpdateVersion: Ref<string | null>;
  statusText: Ref<string>;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
  applySettings(settings: Settings): void;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
};

export function createMaintenanceActions(deps: MaintenanceDeps) {
  async function loadUpdaterStatus() {
    deps.updateStatus.value = await api.updaterStatus();
  }

  function availableUpdateVersion() {
    return deps.updateCheckReport.value?.package?.version ?? null;
  }

  function openUpdatePrompt() {
    if (!deps.updateCheckReport.value?.package) return;
    deps.updatePromptOpen.value = true;
  }

  function markUpdateDismissed() {
    const version = availableUpdateVersion();
    if (version) deps.dismissedUpdateVersion.value = version;
  }

  async function checkForUpdates(showStatus = true) {
    await deps.runTask(showStatus ? deps.t("status.updateCheckCompleted") : deps.statusText.value, async () => {
      deps.updateCheckReport.value = await api.updaterCheck(deps.settingsUpdateChannel.value);
      await loadUpdaterStatus();
      const version = availableUpdateVersion();
      if (!version) return;
      if (showStatus || version !== deps.settingsSkippedUpdateVersion.value) {
        deps.updatePromptOpen.value = true;
      }
    });
  }

  function cancelUpdatePrompt() {
    markUpdateDismissed();
    deps.updatePromptOpen.value = false;
  }

  async function skipUpdateVersion() {
    const version = availableUpdateVersion();
    if (!version) return;
    deps.updatePromptOpen.value = false;
    deps.dismissedUpdateVersion.value = version;
    await deps.runTask(deps.t("status.updateSkipped"), async () => {
      const settings = await api.updateSettings({ skipped_update_version: version });
      deps.applySettings(settings);
    });
  }

  async function confirmUpdatePrompt() {
    const packageInfo = deps.updateCheckReport.value?.package;
    if (!packageInfo) return;
    deps.updatePromptOpen.value = false;
    deps.dismissedUpdateVersion.value = packageInfo.version;
    await deps.runTask(deps.t("status.updateRestarting"), async () => {
      await api.updaterDownloadAndStage(packageInfo);
      await loadUpdaterStatus();
      await api.updaterInstallStaged(packageInfo.version, true);
    });
  }

  return {
    loadUpdaterStatus,
    checkForUpdates,
    openUpdatePrompt,
    cancelUpdatePrompt,
    skipUpdateVersion,
    confirmUpdatePrompt,
  };
}
