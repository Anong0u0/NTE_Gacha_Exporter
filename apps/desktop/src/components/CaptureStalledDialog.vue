<script setup lang="ts">
import { RefreshCw, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
  <Teleport to="body">
    <div v-if="app.captureStalledDialogOpen && app.canRetryAutoPageSlower" class="update-dialog-backdrop" @click.self="app.closeCaptureStalledDialog">
      <section class="update-dialog capture-stalled-dialog" role="dialog" aria-modal="true" :aria-label="app.t('capture.failed')" @keydown.esc="app.closeCaptureStalledDialog">
        <div class="update-dialog-head">
          <div>
            <span class="eyebrow">{{ app.t("capture.statusDetails") }}</span>
            <h2>{{ app.t("capture.failed") }}</h2>
          </div>
          <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.closeCaptureStalledDialog">
            <X :size="17" />
          </button>
        </div>

        <div class="update-dialog-body">
          <p class="status-dialog-message error">{{ app.errorText || app.captureStatus?.error?.message || app.t("status.captureFailed") }}</p>
          <p class="status-dialog-subtitle">{{ app.t("capture.slowRetryPrompt") }}</p>

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
            <div v-if="app.captureStatus.auto_page" class="capture-stats">
              <span>{{ app.t("capture.poolsDone", { count: app.captureStatus.auto_page.completed_pools?.length ?? 0 }) }}</span>
              <span>{{ app.t("capture.poolsSkipped", { count: app.captureStatus.auto_page.skipped_pools?.length ?? 0 }) }}</span>
            </div>
            <div v-if="app.captureStatus.raw_path" class="capture-target">{{ app.t("capture.rawPath") }} · {{ app.captureStatus.raw_path }}</div>
            <div v-if="app.captureStatus.error?.support_path" class="capture-target">support · {{ app.captureStatus.error.support_path }}</div>
            <div v-if="app.captureStatus.error?.support_image_path" class="capture-target">support image · {{ app.captureStatus.error.support_image_path }}</div>
            <div v-if="app.captureStatus.target" class="capture-target">
              {{ app.t("capture.target") }} · {{ app.captureStatus.target.pid ?? "-" }} · {{ app.captureStatus.target.interface ?? "-" }}
            </div>
            <div v-if="app.captureStatus.latest_records.length" class="capture-latest">
              <strong>{{ app.t("capture.latestRecords") }}</strong>
              <div v-for="record in app.captureStatus.latest_records.slice(-8)" :key="String(record.record_id ?? record.item_id ?? app.captureRecordName(record))">
                <span>{{ app.captureRecordName(record) }}</span>
                <small>{{ app.captureRecordMeta(record) }}</small>
              </div>
            </div>
          </div>
        </div>

        <div class="update-dialog-actions">
          <button type="button" @click="app.closeCaptureStalledDialog">
            <X :size="17" />
            <span>{{ app.t("common.close") }}</span>
          </button>
          <div>
            <button type="button" class="primary" :disabled="app.captureActionBusy" @click="app.retryAutoPageSlower">
              <RefreshCw :size="17" />
              <span>{{ app.t("capture.slowRetry", { ms: app.nextPageRecordMinWaitMs }) }}</span>
            </button>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
