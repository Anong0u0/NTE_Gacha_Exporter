<script setup lang="ts">
import { Download, RefreshCw, ShieldCheck, X } from "lucide-vue-next";
import { computed } from "vue";
import { useAppContext } from "../app/context";

const app = useAppContext();

const open = computed(() => Boolean(app.captureRecoveryState));

function iconFor(actionId: string) {
  switch (actionId) {
    case "install_windivert_retry":
    case "reinstall_windivert":
      return Download;
    case "trusted_retry":
      return ShieldCheck;
    case "retry_pktmon":
    case "retry_auto_page_slower":
      return RefreshCw;
    default:
      return X;
  }
}

function installStageLabel(stage?: string | null) {
  switch (stage) {
    case "checking":
      return app.t("capture.installStageChecking");
    case "downloading":
      return app.t("capture.installStageDownloading");
    case "verifying":
      return app.t("capture.installStageVerifying");
    case "installing":
      return app.t("capture.installStageInstalling");
    case "ready":
      return app.t("capture.installStageReady");
    case "failed":
      return app.t("capture.installStageFailed");
    default:
      return "";
  }
}
</script>

<template>
  <Teleport to="body">
    <div v-if="open && app.captureRecoveryState" class="update-dialog-backdrop" @click.self="app.closeCaptureRecoveryDialog">
      <section class="update-dialog capture-stalled-dialog" role="dialog" aria-modal="true" :aria-label="app.t('capture.failed')" @keydown.esc="app.closeCaptureRecoveryDialog">
        <div class="update-dialog-head">
          <div>
            <span class="eyebrow">{{ app.t("capture.statusDetails") }}</span>
            <h2>{{ app.t("capture.failed") }}</h2>
          </div>
          <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.closeCaptureRecoveryDialog">
            <X :size="17" />
          </button>
        </div>

        <div class="update-dialog-body">
          <p v-if="app.captureRecoveryState.kind === 'stalled'" class="status-dialog-message error">{{ app.t("capture.slowRetryPrompt") }}</p>
          <p v-else-if="app.captureRecoveryState.kind === 'vpn_proxy_suspected'" class="status-dialog-message error">{{ app.t("capture.vpnProxyPrompt") }}</p>
          <p v-else-if="app.captureRecoveryState.kind === 'windivert_unavailable'" class="status-dialog-message error">{{ app.t("capture.windivertUnavailablePrompt") }}</p>
          <p v-else class="status-dialog-message error">{{ app.t("capture.windivertNoDecodePrompt") }}</p>
          <details v-if="app.captureRecoveryState.technicalDetail" class="status-dialog-subtitle">
            <summary>{{ app.t("capture.technicalDetail") }}</summary>
            <p>{{ app.captureRecoveryState.technicalDetail }}</p>
          </details>
          <p v-if="app.captureRecoveryState.busyStage" class="status-dialog-subtitle">
            {{ installStageLabel(app.captureRecoveryState.busyStage) }}
          </p>

          <div v-if="app.captureStatus" class="capture-summary">
            <div class="capture-stats">
              <span>{{ app.captureModeLabel }}</span>
              <span>{{ app.formatCaptureState(app.captureStatus.state) }}</span>
              <span>{{ app.t("capture.packets", { count: app.captureStatus.counters.packets_seen }) }}</span>
              <span>{{ app.t("capture.decoded", { count: app.captureStatus.counters.decoded_packets }) }}</span>
              <span>{{ app.t("capture.dropped", { count: app.captureStatus.counters.dropped_packets }) }}</span>
              <span v-if="app.captureStatus.counters.duplicate_packets">{{ app.t("capture.duplicates", { count: app.captureStatus.counters.duplicate_packets }) }}</span>
            </div>
            <div v-if="app.autoPageStatusLine" class="capture-target">{{ app.autoPageStatusLine }}</div>
            <div v-if="app.captureStatus.raw_path" class="capture-target">{{ app.t("capture.rawPath") }} · {{ app.captureStatus.raw_path }}</div>
            <div v-if="app.captureStatus.error?.support_path" class="capture-target">support · {{ app.captureStatus.error.support_path }}</div>
            <div v-if="app.captureStatus.target" class="capture-target">
              {{ app.t("capture.target") }} · {{ app.captureStatus.target.pid ?? "-" }} · {{ app.captureStatus.target.interface ?? "-" }}
            </div>
          </div>
        </div>

        <div class="update-dialog-actions">
          <button
            v-for="action in app.captureRecoveryState.actions"
            :key="action.id"
            type="button"
            :class="{ primary: action.primary }"
            :disabled="app.captureActionBusy && action.id !== 'close'"
            @click="app.runRecoveryDialogAction(action.id)"
          >
            <component :is="iconFor(action.id)" :size="17" />
            <span>{{ action.label }}</span>
          </button>
        </div>
      </section>
    </div>
  </Teleport>
</template>
