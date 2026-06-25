import type { ComputedRef, Ref } from "vue";

import { api, type CaptureMode, type CaptureStatus, type ImportReport, type PendingAdminCapture } from "../api";
import type { I18nKey } from "./i18n";

type CaptureActionsDeps = {
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  captureMode: Ref<CaptureMode>;
  captureStatus: Ref<CaptureStatus | null>;
  captureActionBusy: Ref<boolean>;
  capturePollInFlight: Ref<boolean>;
  lastReport: Ref<ImportReport | null>;
  statusText: Ref<string>;
  errorText: Ref<string>;
  isCaptureActive: ComputedRef<boolean>;
  isWorkflowBusy: ComputedRef<boolean>;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
  formatError(error: unknown): string;
  formatCaptureState(value?: string | null): string;
  formatCaptureMode(value?: string | null): string;
  saveRecordViewPrefs(profileName?: string): void;
  setActiveProfileName(profileName: string): void;
  refreshAll(): Promise<void>;
};

export function createCaptureActions(deps: CaptureActionsDeps) {
  let capturePollTimer: ReturnType<typeof setInterval> | null = null;

  async function startPendingAdminCapture() {
    const pending = await api.takePendingAdminCapture();
    if (!pending) return false;
    deps.saveRecordViewPrefs();
    deps.setActiveProfileName(pending.profile_name);
    deps.locale.value = pending.locale;
    deps.captureMode.value = pending.mode;
    await startLiveCapture({ skipAdminRequest: true, pending });
    return true;
  }

  async function startLiveCapture(options: { skipAdminRequest?: boolean; pending?: PendingAdminCapture } = {}) {
    if ((deps.isWorkflowBusy.value && !options.skipAdminRequest) || !deps.activeProfileName.value) return;
    deps.captureActionBusy.value = true;
    deps.errorText.value = "";
    try {
      if (!options.skipAdminRequest) {
        const relaunching = await api.requestAdminCaptureStart(deps.activeProfileName.value, deps.locale.value, deps.captureMode.value);
        if (relaunching) {
          deps.statusText.value = deps.t("status.waitingAdmin");
          return;
        }
      }
      await applyCaptureStatus(await api.captureStart(deps.activeProfileName.value, deps.locale.value, deps.captureMode.value));
      deps.statusText.value = options.pending
        ? deps.t("capture.modeResumed", { mode: deps.formatCaptureMode(deps.captureMode.value) })
        : deps.t("status.captureStarted", { mode: deps.formatCaptureMode(deps.captureMode.value) });
      if (deps.isCaptureActive.value) {
        ensureCapturePolling();
      }
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.captureActionBusy.value = false;
    }
  }

  async function startFullCapture() {
    deps.captureMode.value = "auto_page_full";
    await startLiveCapture();
  }

  async function stopLiveCapture() {
    const sessionId = deps.captureStatus.value?.session_id;
    if (!sessionId || !deps.isCaptureActive.value || deps.captureActionBusy.value) return;
    deps.captureActionBusy.value = true;
    deps.errorText.value = "";
    try {
      await applyCaptureStatus(await api.captureStop(sessionId));
      deps.statusText.value = deps.t("status.captureStopping");
      if (deps.isCaptureActive.value) {
        ensureCapturePolling();
      }
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.captureActionBusy.value = false;
    }
  }

  async function pollCaptureStatus() {
    const sessionId = deps.captureStatus.value?.session_id;
    if (!sessionId || deps.capturePollInFlight.value) return;
    deps.capturePollInFlight.value = true;
    try {
      await applyCaptureStatus(await api.captureStatus(sessionId));
    } catch (error) {
      clearCapturePolling();
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.capturePollInFlight.value = false;
    }
  }

  async function applyCaptureStatus(status: CaptureStatus) {
    deps.captureStatus.value = status;
    if (status.state === "completed") {
      clearCapturePolling();
      if (status.import_report) {
        deps.lastReport.value = status.import_report;
      }
      await deps.refreshAll();
      deps.statusText.value = status.import_report ? deps.t("status.captureMerged") : deps.t("status.captureCompleted");
    } else if (status.state === "failed") {
      clearCapturePolling();
      deps.errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : deps.t("status.captureFailed");
    } else {
      deps.statusText.value = deps.formatCaptureState(status.state);
    }
  }

  function ensureCapturePolling() {
    if (capturePollTimer) return;
    capturePollTimer = setInterval(() => {
      void pollCaptureStatus();
    }, 1000);
  }

  function clearCapturePolling() {
    if (!capturePollTimer) return;
    clearInterval(capturePollTimer);
    capturePollTimer = null;
  }

  return {
    startPendingAdminCapture,
    startLiveCapture,
    startFullCapture,
    stopLiveCapture,
    pollCaptureStatus,
    applyCaptureStatus,
    ensureCapturePolling,
    clearCapturePolling,
  };
}
