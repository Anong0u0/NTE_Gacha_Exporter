<script setup lang="ts">
import { CircleStop, MoreHorizontal, RadioTower, RefreshCw, X } from "lucide-vue-next";
import { computed, ref } from "vue";
import { useAppContext } from "../app/context";

const app = useAppContext();
const captureMenuOpen = ref(false);
const statusDialogOpen = ref(false);
let menuCloseTimer: ReturnType<typeof setTimeout> | null = null;

const statusSummary = computed(() => app.errorText || app.statusText || app.captureSubtitle);

function openCaptureMenu() {
  if (menuCloseTimer) {
    clearTimeout(menuCloseTimer);
    menuCloseTimer = null;
  }
  captureMenuOpen.value = true;
}

function scheduleCaptureMenuClose() {
  if (menuCloseTimer) clearTimeout(menuCloseTimer);
  menuCloseTimer = setTimeout(() => {
    captureMenuOpen.value = false;
    menuCloseTimer = null;
  }, 120);
}

function closeCaptureMenu() {
  if (menuCloseTimer) {
    clearTimeout(menuCloseTimer);
    menuCloseTimer = null;
  }
  captureMenuOpen.value = false;
}

function onCaptureMenuFocusOut(event: FocusEvent) {
  const current = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  if (current?.contains(event.relatedTarget as Node | null)) return;
  closeCaptureMenu();
}

function onAutoPageChange(event: Event) {
  app.setCaptureAutoPageEnabled((event.target as HTMLInputElement).checked);
}

function onFullUpdateChange(event: Event) {
  app.setCaptureFullUpdateEnabled((event.target as HTMLInputElement).checked);
}
</script>

<template>
      <header class="topbar">
        <div>
          <span class="eyebrow">{{ app.activeProfile?.name ?? app.activeProfileName }}</span>
          <h1>{{ app.t(app.navItems.find((item) => item.id === app.activeView)?.labelKey ?? "nav.dashboard") }}</h1>
        </div>
        <div class="topbar-actions">
          <button type="button" :disabled="app.isWorkflowBusy" :title="app.t('status.dashboardUpdated')" @click="app.runTask(app.t('status.dashboardUpdated'), app.refreshAll)">
            <RefreshCw :size="17" />
          </button>
          <button class="primary topbar-update-button" type="button" :disabled="app.isWorkflowBusy" @click="app.startPreferredCapture">
            <RadioTower :size="17" />
            <span>{{ app.t("import.updateData") }}</span>
          </button>
          <button
            v-if="app.isCaptureActive"
            class="danger topbar-stop-button"
            type="button"
            :disabled="app.captureActionBusy || app.captureStatus?.state === 'stopping'"
            @click="app.stopLiveCapture"
          >
            <CircleStop :size="17" />
            <span>{{ app.t("capture.stop") }}</span>
          </button>
          <div
            class="topbar-menu"
            :class="{ 'is-open': captureMenuOpen }"
            @pointerenter="openCaptureMenu"
            @pointerleave="scheduleCaptureMenuClose"
            @focusin="openCaptureMenu"
            @focusout="onCaptureMenuFocusOut"
          >
            <button
              type="button"
              class="topbar-menu-trigger"
              :title="app.t('capture.updateOptions')"
              :aria-label="app.t('capture.updateOptions')"
              :aria-expanded="captureMenuOpen"
              @click="openCaptureMenu"
            >
              <MoreHorizontal :size="17" />
            </button>
            <div class="topbar-menu-panel" role="menu">
              <label class="menu-check">
                <input type="checkbox" :checked="app.captureAutoPageEnabled" :disabled="app.isWorkflowBusy" @change="onAutoPageChange" />
                <span>{{ app.t("capture.autoPageEnabled") }}</span>
              </label>
              <label class="menu-check">
                <input type="checkbox" :checked="app.captureFullUpdateEnabled" :disabled="app.isWorkflowBusy || !app.captureAutoPageEnabled" @change="onFullUpdateChange" />
                <span>{{ app.t("capture.fullUpdateEnabled") }}</span>
              </label>
            </div>
          </div>
          <button type="button" class="status" data-agent-id="topbar-status" :class="{ error: app.errorText }" @click="statusDialogOpen = true">
            {{ statusSummary }}
          </button>
        </div>
      </header>
      <Teleport to="body">
        <div v-if="statusDialogOpen" class="status-dialog-backdrop" @click.self="statusDialogOpen = false">
          <section class="status-dialog" role="dialog" aria-modal="true" :aria-label="app.t('capture.statusDetails')">
            <div class="status-dialog-head">
              <div>
                <span class="eyebrow">{{ app.t("capture.statusDetails") }}</span>
                <h2>{{ app.errorText ? app.t("capture.failed") : app.captureTitle }}</h2>
              </div>
              <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="statusDialogOpen = false">
                <X :size="17" />
              </button>
            </div>
            <div class="status-dialog-body">
              <p class="status-dialog-message" :class="{ error: app.errorText }">{{ app.errorText || app.statusText || app.captureSubtitle }}</p>
              <div v-if="app.canRetryAutoPageSlower" class="status-dialog-actions">
                <p>{{ app.t("capture.slowRetryPrompt") }}</p>
                <button type="button" class="primary" :disabled="app.captureActionBusy" @click="app.retryAutoPageSlower">
                  <RefreshCw :size="17" />
                  <span>{{ app.t("capture.slowRetry", { ms: app.nextPageRecordMinWaitMs }) }}</span>
                </button>
              </div>
              <p v-if="app.captureSubtitle" class="status-dialog-subtitle">{{ app.captureSubtitle }}</p>
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
          </section>
        </div>
      </Teleport>
</template>
