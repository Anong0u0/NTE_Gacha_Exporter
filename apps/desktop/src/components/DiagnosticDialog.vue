<script setup lang="ts">
import { Archive, CheckCircle2, Play, Square, Stethoscope, X } from "lucide-vue-next";
import { computed } from "vue";
import { useAppContext } from "../app/context";

const app = useAppContext();

const isActive = computed(() => app.isDiagnosticActive);
const progressPercent = computed(() => Math.round((app.diagnosticStatus?.progress ?? 0) * 100));
const elapsed = computed(() => Math.round(app.diagnosticStatus?.elapsed_seconds ?? 0));
const duration = computed(() => app.diagnosticStatus?.duration_seconds ?? 20);
</script>

<template>
  <Teleport to="body">
    <div v-if="app.diagnosticPromptOpen" class="update-dialog-backdrop" @click.self="!isActive && app.cancelDiagnosticPrompt()">
      <section class="update-dialog diagnostic-dialog" role="dialog" aria-modal="true" :aria-label="app.t('diagnostic.title')">
        <div class="update-dialog-head">
          <div>
            <span class="eyebrow">{{ app.t("common.doctor") }}</span>
            <h2>{{ app.t("diagnostic.title") }}</h2>
          </div>
          <button type="button" :disabled="isActive" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.cancelDiagnosticPrompt">
            <X :size="17" />
          </button>
        </div>

        <div class="update-dialog-body">
          <section v-if="!app.diagnosticStatus" class="diagnostic-copy">
            <Stethoscope :size="22" />
            <p>{{ app.t("diagnostic.prompt") }}</p>
          </section>

          <section v-else class="diagnostic-run">
            <div class="diagnostic-progress-head">
              <span>{{ app.t("diagnostic.stage") }}</span>
              <strong>{{ app.diagnosticStatus.stage }}</strong>
            </div>
            <div class="diagnostic-progress-track" :aria-label="app.t('diagnostic.progress')">
              <div :style="{ width: `${progressPercent}%` }"></div>
            </div>
            <div class="diagnostic-elapsed-row">
              <span>{{ app.t("diagnostic.elapsed") }}</span>
              <strong>{{ elapsed }}s / {{ duration }}s</strong>
            </div>

            <div v-if="app.diagnosticStatus.summary" class="stat-table compact">
              <div><span>{{ app.t("diagnostic.verdict") }}</span><strong>{{ app.diagnosticStatus.summary.verdict }}</strong></div>
              <div><span>{{ app.t("diagnostic.packets") }}</span><strong>{{ app.diagnosticStatus.summary.packets_seen }}</strong></div>
              <div><span>{{ app.t("diagnostic.rows") }}</span><strong>{{ app.diagnosticStatus.summary.rows_count }}</strong></div>
              <div><span>{{ app.t("diagnostic.external") }}</span><strong>{{ app.diagnosticStatus.summary.external_ok ? app.t("diagnostic.externalOk") : app.t("diagnostic.externalFailed") }}</strong></div>
            </div>

            <div v-if="app.diagnosticStatus.summary?.findings.length" class="diagnostic-findings">
              <div v-for="finding in app.diagnosticStatus.summary.findings" :key="finding">{{ finding }}</div>
            </div>

            <div v-if="app.diagnosticStatus.support_zip_path" class="diagnostic-path">
              <Archive :size="17" />
              <span>{{ app.diagnosticStatus.support_zip_path }}</span>
            </div>

            <div v-if="app.diagnosticStatus.error" class="diagnostic-error">
              {{ app.diagnosticStatus.error.code }}: {{ app.diagnosticStatus.error.message }}
            </div>
          </section>
        </div>

        <div class="update-dialog-actions">
          <button v-if="!app.diagnosticStatus" type="button" :disabled="app.diagnosticActionBusy" @click="app.cancelDiagnosticPrompt">
            <X :size="17" />
            <span>{{ app.t("common.cancel") }}</span>
          </button>
          <button v-else-if="isActive" type="button" :disabled="app.diagnosticActionBusy" @click="app.cancelDiagnostic">
            <Square :size="17" />
            <span>{{ app.t("capture.stop") }}</span>
          </button>
          <button v-else type="button" @click="app.cancelDiagnosticPrompt">
            <CheckCircle2 :size="17" />
            <span>{{ app.t("common.close") }}</span>
          </button>

          <div>
            <button v-if="!app.diagnosticStatus" type="button" :disabled="app.diagnosticActionBusy" @click="app.confirmDiagnosticPrompt('pktmon')">
              <Play :size="17" />
              <span>{{ app.t("diagnostic.startPktmon") }}</span>
            </button>
            <button v-if="!app.diagnosticStatus" class="primary" type="button" :disabled="app.diagnosticActionBusy" @click="app.confirmDiagnosticPrompt('windivert')">
              <Play :size="17" />
              <span>{{ app.t("diagnostic.startWinDivert") }}</span>
            </button>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
