import { computed, ref, type ComputedRef, type Ref } from "vue";

import { api, type CaptureMode, type CaptureStartOptions, type CaptureStatus, type ImportReport, type PendingAdminCapture } from "../api";
import type { I18nKey } from "./i18n";

const CAPTURE_WINDOW_STALLED_CODE = "auto_page_capture_window_stalled";
const DEFAULT_PAGE_RECORD_MIN_WAIT_MS = 300;
const PAGE_RECORD_RETRY_STEP_MS = 200;
const MAX_PAGE_RECORD_MIN_WAIT_MS = 1500;

type CaptureActionsDeps = {
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  captureMode: Ref<CaptureMode>;
  captureStatus: Ref<CaptureStatus | null>;
  captureStalledDialogOpen: Ref<boolean>;
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
  const pageRecordMinWaitMs = ref(DEFAULT_PAGE_RECORD_MIN_WAIT_MS);

  const canRetryAutoPageSlower = computed(() =>
    deps.captureStatus.value?.state === "failed"
      && deps.captureStatus.value.error?.code === CAPTURE_WINDOW_STALLED_CODE
      && !deps.captureActionBusy.value
      && Boolean(deps.activeProfileName.value),
  );

  const nextPageRecordMinWaitMs = computed(() =>
    Math.min(MAX_PAGE_RECORD_MIN_WAIT_MS, pageRecordMinWaitMs.value + PAGE_RECORD_RETRY_STEP_MS),
  );

  async function startPendingAdminCapture() {
    const pending = await api.takePendingAdminCapture();
    if (!pending) return false;
    deps.saveRecordViewPrefs();
    deps.setActiveProfileName(pending.profile_name);
    deps.locale.value = pending.locale;
    deps.captureMode.value = pending.mode;
    pageRecordMinWaitMs.value = pending.options?.page_record_min_wait_ms ?? pageRecordMinWaitMs.value;
    await startLiveCapture({ skipAdminRequest: true, pending, captureOptions: pending.options });
    return true;
  }

  async function startLiveCapture(options: { skipAdminRequest?: boolean; pending?: PendingAdminCapture; captureOptions?: CaptureStartOptions } = {}) {
    if ((deps.isWorkflowBusy.value && !options.skipAdminRequest) || !deps.activeProfileName.value) return;
    deps.captureActionBusy.value = true;
    deps.captureStalledDialogOpen.value = false;
    deps.errorText.value = "";
    try {
      const captureOptions = options.captureOptions ?? captureOptionsForMode(deps.captureMode.value);
      if (!options.skipAdminRequest) {
        const relaunching = await api.requestAdminCaptureStart(deps.activeProfileName.value, deps.locale.value, deps.captureMode.value, captureOptions);
        if (relaunching) {
          deps.statusText.value = deps.t("status.waitingAdmin");
          return;
        }
      }
      await applyCaptureStatus(await api.captureStart(deps.activeProfileName.value, deps.locale.value, deps.captureMode.value, captureOptions));
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

  async function retryAutoPageSlower() {
    if (!canRetryAutoPageSlower.value) return;
    const retryMode = retryCaptureMode();
    if (!isAutoPageMode(retryMode)) return;
    deps.captureStalledDialogOpen.value = false;
    pageRecordMinWaitMs.value = nextPageRecordMinWaitMs.value;
    deps.captureMode.value = retryMode;
    await startLiveCapture({
      captureOptions: {
        page_record_min_wait_ms: pageRecordMinWaitMs.value,
      },
    });
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
      deps.captureStalledDialogOpen.value = false;
      if (status.import_report) {
        deps.lastReport.value = status.import_report;
      }
      await deps.refreshAll();
      deps.statusText.value = status.import_report ? deps.t("status.captureMerged") : deps.t("status.captureCompleted");
    } else if (status.state === "failed") {
      clearCapturePolling();
      deps.errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : deps.t("status.captureFailed");
      deps.captureStalledDialogOpen.value = status.error?.code === CAPTURE_WINDOW_STALLED_CODE;
    } else {
      deps.captureStalledDialogOpen.value = false;
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

  function captureOptionsForMode(mode: CaptureMode): CaptureStartOptions | undefined {
    return isAutoPageMode(mode)
      ? { page_record_min_wait_ms: pageRecordMinWaitMs.value }
      : undefined;
  }

  function retryCaptureMode(): CaptureMode {
    const mode = deps.captureStatus.value?.mode ?? deps.captureMode.value;
    return isAutoPageMode(mode) ? mode : deps.captureMode.value;
  }

  function isAutoPageMode(mode: CaptureMode) {
    return mode === "auto_page_incremental" || mode === "auto_page_full";
  }

  return {
    canRetryAutoPageSlower,
    nextPageRecordMinWaitMs,
    startPendingAdminCapture,
    startLiveCapture,
    startFullCapture,
    retryAutoPageSlower,
    stopLiveCapture,
    pollCaptureStatus,
    applyCaptureStatus,
    ensureCapturePolling,
    clearCapturePolling,
  };
}
