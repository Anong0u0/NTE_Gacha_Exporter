import type { ComputedRef, Ref } from "vue";

import {
  api,
  type CaptureMode,
  type Settings,
  type SettingsPatch,
} from "../api";
import type { I18nKey } from "./i18n";

type SettingsActionsDeps = {
  locale: Ref<string>;
  uiLocale: Ref<string>;
  settingsUpdateChannel: Ref<string>;
  settingsCheckUpdates: Ref<boolean>;
  settingsSkippedUpdateVersion: Ref<string | null>;
  captureAutoPageEnabled: Ref<boolean>;
  captureFullUpdateEnabled: Ref<boolean>;
  captureWinDivertBackendEnabled: Ref<boolean>;
  captureMode: Ref<CaptureMode>;
  effectiveCaptureMode: ComputedRef<CaptureMode>;
  errorText: Ref<string>;
  setActiveProfileName(profileName: string): void;
  saveRecordViewPrefs(profileName?: string): void;
  loadProfiles(): Promise<void>;
  refreshAll(): Promise<void>;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
  formatError(error: unknown): string;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
};

export function createSettingsActions(deps: SettingsActionsDeps) {
  function applySettings(settings: Settings) {
    deps.setActiveProfileName(settings.active_profile);
    deps.locale.value = settings.locale;
    deps.uiLocale.value = settings.ui_locale || deps.uiLocale.value;
    deps.settingsUpdateChannel.value = settings.update_channel;
    deps.settingsCheckUpdates.value = settings.check_updates_on_startup;
    deps.settingsSkippedUpdateVersion.value = settings.skipped_update_version ?? null;
    deps.captureAutoPageEnabled.value = settings.capture_auto_page_enabled;
    deps.captureFullUpdateEnabled.value =
      settings.capture_auto_page_enabled && settings.capture_full_update_enabled;
    deps.captureWinDivertBackendEnabled.value = settings.capture_windivert_backend_enabled;
    deps.captureMode.value = deps.effectiveCaptureMode.value;
  }

  async function saveCaptureSettings() {
    try {
      const settings = await api.updateSettings({
        capture_auto_page_enabled: deps.captureAutoPageEnabled.value,
        capture_full_update_enabled: deps.captureFullUpdateEnabled.value,
        capture_windivert_backend_enabled: deps.captureWinDivertBackendEnabled.value,
      });
      applySettings(settings);
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    }
  }

  function setCaptureAutoPageEnabled(value: boolean) {
    deps.captureAutoPageEnabled.value = value;
    if (!value) deps.captureFullUpdateEnabled.value = false;
    deps.captureMode.value = deps.effectiveCaptureMode.value;
    void saveCaptureSettings();
  }

  function setCaptureFullUpdateEnabled(value: boolean) {
    deps.captureFullUpdateEnabled.value = value;
    if (value) deps.captureAutoPageEnabled.value = true;
    deps.captureMode.value = deps.effectiveCaptureMode.value;
    void saveCaptureSettings();
  }

  function setCaptureWinDivertBackendEnabled(value: boolean) {
    deps.captureWinDivertBackendEnabled.value = value;
    void saveCaptureSettings();
  }

  async function updateRuntimeSettings(
    patch: SettingsPatch,
    options: { refreshData?: boolean } = {},
  ) {
    await deps.runTask(deps.t("status.settingsUpdated"), async () => {
      const settings = await api.updateSettings(patch);
      deps.saveRecordViewPrefs();
      applySettings(settings);
      if (options.refreshData) {
        await deps.loadProfiles();
        await deps.refreshAll();
      }
    });
  }

  async function setUiLocale(value: string) {
    if (value === deps.uiLocale.value) return;
    await updateRuntimeSettings({ ui_locale: value });
  }

  async function setDataLocale(value: string) {
    if (value === deps.locale.value) return;
    await updateRuntimeSettings({ locale: value }, { refreshData: true });
  }

  async function setUpdateChannel(value: string) {
    if (value === deps.settingsUpdateChannel.value) return;
    await updateRuntimeSettings({ update_channel: value });
  }

  async function setCheckUpdatesOnStartup(value: boolean) {
    if (value === deps.settingsCheckUpdates.value) return;
    await updateRuntimeSettings({ check_updates_on_startup: value });
  }

  return {
    applySettings,
    setCaptureAutoPageEnabled,
    setCaptureFullUpdateEnabled,
    setCaptureWinDivertBackendEnabled,
    updateRuntimeSettings,
    setUiLocale,
    setDataLocale,
    setUpdateChannel,
    setCheckUpdatesOnStartup,
  };
}
