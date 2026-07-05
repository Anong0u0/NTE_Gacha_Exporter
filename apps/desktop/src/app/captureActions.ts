import { computed, ref, type ComputedRef, type Ref } from "vue";

import { api, type CaptureMode, type CaptureStartOptions, type CaptureStatus, type ImportReport, type PendingAdminCapture } from "../api";
import type { I18nKey } from "./i18n";

const CAPTURE_WINDOW_STALLED_CODE = "auto_page_capture_window_stalled";
const VPN_PROXY_SUSPECTED_CODE = "vpn_proxy_suspected";
const WINDIVERT_UNAVAILABLE_CODE = "windivert_unavailable";
const WINDIVERT_NO_DECODE_CODE = "windivert_no_decode";
const DEFAULT_PAGE_RECORD_MIN_WAIT_MS = 300;
const PAGE_RECORD_RETRY_STEP_MS = 200;
const MAX_PAGE_RECORD_MIN_WAIT_MS = 1500;

export type RecoveryDialogActionId =
  | "close"
  | "retry_pktmon"
  | "retry_auto_page_slower"
  | "install_windivert_retry"
  | "reinstall_windivert"
  | "trusted_retry"
  | "disable_windivert";

export type CaptureRecoveryState = {
  kind: "stalled" | "vpn_proxy_suspected" | "windivert_unavailable" | "windivert_no_decode";
  technicalDetail: string;
  actions: { id: RecoveryDialogActionId; label: string; primary?: boolean }[];
  busyStage?: "checking" | "downloading" | "verifying" | "installing" | "ready" | "failed" | null;
};

type CaptureActionsDeps = {
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  captureMode: Ref<CaptureMode>;
  captureWinDivertBackendEnabled: Ref<boolean>;
  captureStatus: Ref<CaptureStatus | null>;
  captureRecoveryState: Ref<CaptureRecoveryState | null>;
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
    deps.captureRecoveryState.value = null;
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
      if (shouldUseWinDivert(captureOptions)) {
        await ensureWinDivertInstalled();
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
      if (errorCode(error) === WINDIVERT_UNAVAILABLE_CODE) {
        deps.captureRecoveryState.value = windivertUnavailableRecovery(deps.errorText.value);
      }
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
    deps.captureRecoveryState.value = null;
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
      deps.captureRecoveryState.value = null;
      if (status.import_report) {
        deps.lastReport.value = status.import_report;
      }
      await deps.refreshAll();
      deps.statusText.value = status.import_report ? deps.t("status.captureMerged") : deps.t("status.captureCompleted");
      if (status.target?.interface === "windivert") {
        deps.captureWinDivertBackendEnabled.value = true;
      }
    } else if (status.state === "failed") {
      clearCapturePolling();
      deps.errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : deps.t("status.captureFailed");
      deps.captureRecoveryState.value = recoveryStateForStatus(status);
    } else if (status.state === "cancelled") {
      clearCapturePolling();
      deps.captureRecoveryState.value = null;
      deps.errorText.value = "";
      deps.statusText.value = deps.t("capture.cancelled");
    } else {
      deps.captureRecoveryState.value = null;
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
    const options = isAutoPageMode(mode)
      ? { page_record_min_wait_ms: pageRecordMinWaitMs.value }
      : undefined;
    if (deps.captureWinDivertBackendEnabled.value) {
      return { ...(options ?? {}), capture_backend: "windivert" };
    }
    return options;
  }

  function retryCaptureMode(): CaptureMode {
    const mode = deps.captureStatus.value?.mode ?? deps.captureMode.value;
    return isAutoPageMode(mode) ? mode : deps.captureMode.value;
  }

  function isAutoPageMode(mode: CaptureMode) {
    return mode === "auto_page_incremental" || mode === "auto_page_full";
  }

  function recoveryStateForStatus(status: CaptureStatus): CaptureRecoveryState | null {
    const technicalDetail = status.error ? `${status.error.code}: ${status.error.message}` : deps.t("status.captureFailed");
    switch (status.error?.code) {
      case CAPTURE_WINDOW_STALLED_CODE:
        return {
          kind: "stalled",
          technicalDetail,
          actions: [
            { id: "close", label: deps.t("common.close") },
            { id: "retry_auto_page_slower", label: deps.t("capture.slowRetry", { ms: nextPageRecordMinWaitMs.value }), primary: true },
          ],
        };
      case VPN_PROXY_SUSPECTED_CODE:
        return {
          kind: "vpn_proxy_suspected",
          technicalDetail,
          actions: [
            { id: "close", label: deps.t("common.cancel") },
            { id: "retry_pktmon", label: deps.t("capture.vpnProxyRetryPktmon") },
            { id: "install_windivert_retry", label: deps.t("capture.installWinDivertRetry"), primary: true },
          ],
        };
      case WINDIVERT_UNAVAILABLE_CODE:
        return windivertUnavailableRecovery(technicalDetail);
      case WINDIVERT_NO_DECODE_CODE:
        return {
          kind: "windivert_no_decode",
          technicalDetail,
          actions: [
            { id: "close", label: deps.t("common.close") },
            { id: "disable_windivert", label: deps.t("capture.disableWinDivert"), primary: true },
          ],
        };
      default:
        return null;
    }
  }

  function windivertUnavailableRecovery(technicalDetail: string): CaptureRecoveryState {
    return {
      kind: "windivert_unavailable",
      technicalDetail,
      busyStage: null,
      actions: [
        { id: "reinstall_windivert", label: deps.t("capture.redownloadWinDivert") },
        { id: "trusted_retry", label: deps.t("capture.trustedRetry"), primary: true },
        { id: "disable_windivert", label: deps.t("capture.disableWinDivert") },
      ],
    };
  }

  async function runRecoveryDialogAction(id: RecoveryDialogActionId) {
    if (deps.captureActionBusy.value && id !== "close") return;
    switch (id) {
      case "close":
        closeCaptureRecoveryDialog();
        return;
      case "retry_auto_page_slower":
        await retryAutoPageSlower();
        return;
      case "retry_pktmon":
        await retryWithBackend("pktmon");
        return;
      case "install_windivert_retry":
        await installWinDivertAndRetry();
        return;
      case "reinstall_windivert":
        await reinstallWinDivert();
        return;
      case "trusted_retry":
        await retryWithBackend("windivert");
        return;
      case "disable_windivert":
        await disableWinDivertBackend();
        return;
    }
  }

  function closeCaptureRecoveryDialog() {
    deps.captureRecoveryState.value = null;
  }

  async function retryWithBackend(backend: "pktmon" | "windivert") {
    const retryMode = retryCaptureMode();
    deps.captureMode.value = retryMode;
    await startLiveCapture({
      captureOptions: withBackendOverride(retryMode, backend),
    });
  }

  async function installWinDivertAndRetry() {
    await reinstallWinDivert();
    if (deps.captureRecoveryState.value?.busyStage === "failed") return;
    await retryWithBackend("windivert");
  }

  async function reinstallWinDivert() {
    deps.captureActionBusy.value = true;
    try {
      await setInstallStage("checking");
      await api.windivertStatus(false);
      await setInstallStage("downloading");
      const report = await api.windivertInstall();
      await setInstallStage("verifying");
      if (!report.verified_sha256) throw new Error("WinDivert verification failed");
      await setInstallStage("installing");
      await api.windivertStatus(false);
      await setInstallStage("ready");
    } catch (error) {
      await setInstallStage("failed");
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.captureActionBusy.value = false;
    }
  }

  async function ensureWinDivertInstalled() {
    const status = await api.windivertStatus(false);
    if (status.installed) return;
    await api.windivertInstall();
  }

  async function disableWinDivertBackend() {
    deps.captureActionBusy.value = true;
    try {
      const settings = await api.updateSettings({ capture_windivert_backend_enabled: false });
      deps.captureWinDivertBackendEnabled.value = settings.capture_windivert_backend_enabled;
      deps.captureRecoveryState.value = null;
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.captureActionBusy.value = false;
    }
  }

  async function setInstallStage(stage: NonNullable<CaptureRecoveryState["busyStage"]>) {
    if (deps.captureRecoveryState.value) {
      deps.captureRecoveryState.value = { ...deps.captureRecoveryState.value, busyStage: stage };
    }
    await Promise.resolve();
  }

  function withBackendOverride(mode: CaptureMode, backend: "pktmon" | "windivert"): CaptureStartOptions {
    return {
      ...(isAutoPageMode(mode) ? { page_record_min_wait_ms: pageRecordMinWaitMs.value } : {}),
      capture_backend: backend,
    };
  }

  function shouldUseWinDivert(options?: CaptureStartOptions) {
    if (options?.capture_backend === "pktmon") return false;
    return options?.capture_backend === "windivert" || deps.captureWinDivertBackendEnabled.value;
  }

  function errorCode(error: unknown) {
    return typeof error === "object" && error !== null && "code" in error && typeof error.code === "string"
      ? error.code
      : null;
  }

  return {
    canRetryAutoPageSlower,
    nextPageRecordMinWaitMs,
    startPendingAdminCapture,
    startLiveCapture,
    startFullCapture,
    retryAutoPageSlower,
    runRecoveryDialogAction,
    closeCaptureRecoveryDialog,
    stopLiveCapture,
    pollCaptureStatus,
    applyCaptureStatus,
    ensureCapturePolling,
    clearCapturePolling,
  };
}
