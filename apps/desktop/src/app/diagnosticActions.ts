import type { ComputedRef, Ref } from "vue";

import { api, type DiagnosticStatus, type PendingAdminDiagnostic } from "../api";
import type { I18nKey } from "./i18n";

const DEFAULT_DIAGNOSTIC_DURATION_SECONDS = 30;

type DiagnosticActionsDeps = {
  diagnosticPromptOpen: Ref<boolean>;
  diagnosticStatus: Ref<DiagnosticStatus | null>;
  diagnosticActionBusy: Ref<boolean>;
  diagnosticPollInFlight: Ref<boolean>;
  statusText: Ref<string>;
  errorText: Ref<string>;
  isDiagnosticActive: ComputedRef<boolean>;
  isWorkflowBusy: ComputedRef<boolean>;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
  formatError(error: unknown): string;
};

export function createDiagnosticActions(deps: DiagnosticActionsDeps) {
  let diagnosticPollTimer: ReturnType<typeof setInterval> | null = null;

  function openDiagnosticPrompt() {
    if (deps.isWorkflowBusy.value) return;
    deps.errorText.value = "";
    deps.diagnosticStatus.value = null;
    deps.diagnosticPromptOpen.value = true;
  }

  function cancelDiagnosticPrompt() {
    deps.diagnosticPromptOpen.value = false;
  }

  async function confirmDiagnosticPrompt() {
    await startDiagnostic();
  }

  async function startPendingAdminDiagnostic() {
    const pending = await api.takePendingAdminDiagnostic();
    if (!pending) return false;
    await startDiagnostic({ skipAdminRequest: true, pending });
    return true;
  }

  async function startDiagnostic(options: { skipAdminRequest?: boolean; pending?: PendingAdminDiagnostic } = {}) {
    if ((deps.isWorkflowBusy.value && !options.skipAdminRequest) || deps.diagnosticActionBusy.value) return;
    const durationSeconds = options.pending?.duration_seconds ?? DEFAULT_DIAGNOSTIC_DURATION_SECONDS;
    deps.diagnosticPromptOpen.value = true;
    deps.diagnosticActionBusy.value = true;
    deps.errorText.value = "";
    try {
      if (!options.skipAdminRequest) {
        const relaunching = await api.requestAdminDiagnosticStart(durationSeconds);
        if (relaunching) {
          deps.statusText.value = deps.t("status.waitingAdmin");
          return;
        }
      }
      await applyDiagnosticStatus(await api.diagnosticStart(durationSeconds));
      deps.statusText.value = options.pending
        ? deps.t("diagnostic.resumed")
        : deps.t("diagnostic.started");
      if (deps.isDiagnosticActive.value) {
        ensureDiagnosticPolling();
      }
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.diagnosticActionBusy.value = false;
    }
  }

  async function cancelDiagnostic() {
    const sessionId = deps.diagnosticStatus.value?.session_id;
    if (!sessionId || !deps.isDiagnosticActive.value || deps.diagnosticActionBusy.value) return;
    deps.diagnosticActionBusy.value = true;
    deps.errorText.value = "";
    try {
      await applyDiagnosticStatus(await api.diagnosticCancel(sessionId));
      deps.statusText.value = deps.t("diagnostic.stopping");
      if (deps.isDiagnosticActive.value) {
        ensureDiagnosticPolling();
      }
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.diagnosticActionBusy.value = false;
    }
  }

  async function pollDiagnosticStatus() {
    const sessionId = deps.diagnosticStatus.value?.session_id;
    if (!sessionId || deps.diagnosticPollInFlight.value) return;
    deps.diagnosticPollInFlight.value = true;
    try {
      await applyDiagnosticStatus(await api.diagnosticStatus(sessionId));
    } catch (error) {
      clearDiagnosticPolling();
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.diagnosticPollInFlight.value = false;
    }
  }

  async function applyDiagnosticStatus(status: DiagnosticStatus) {
    deps.diagnosticStatus.value = status;
    if (status.state === "completed") {
      clearDiagnosticPolling();
      deps.statusText.value = deps.t("diagnostic.completed");
    } else if (status.state === "failed") {
      clearDiagnosticPolling();
      deps.errorText.value = status.error ? `${status.error.code}: ${status.error.message}` : deps.t("diagnostic.failed");
    } else {
      deps.statusText.value = deps.t("diagnostic.running");
    }
  }

  function ensureDiagnosticPolling() {
    if (diagnosticPollTimer) return;
    diagnosticPollTimer = setInterval(() => {
      void pollDiagnosticStatus();
    }, 1000);
  }

  function clearDiagnosticPolling() {
    if (!diagnosticPollTimer) return;
    clearInterval(diagnosticPollTimer);
    diagnosticPollTimer = null;
  }

  return {
    openDiagnosticPrompt,
    cancelDiagnosticPrompt,
    confirmDiagnosticPrompt,
    startPendingAdminDiagnostic,
    cancelDiagnostic,
    pollDiagnosticStatus,
    applyDiagnosticStatus,
    ensureDiagnosticPolling,
    clearDiagnosticPolling,
  };
}
