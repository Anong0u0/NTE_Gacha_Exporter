<script setup lang="ts">
import { CircleSlash, Download, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
  <Teleport to="body">
    <div v-if="app.updatePromptOpen && app.updateCheckReport?.package" class="update-dialog-backdrop" @click.self="app.cancelUpdatePrompt">
      <section class="update-dialog" role="dialog" aria-modal="true" :aria-label="app.t('update.availableTitle')" @keydown.esc="app.cancelUpdatePrompt">
        <div class="update-dialog-head">
          <div>
            <span class="eyebrow">{{ app.t("settings.updater") }}</span>
            <h2>{{ app.t("update.availableTitle") }}</h2>
          </div>
          <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.cancelUpdatePrompt">
            <X :size="17" />
          </button>
        </div>
        <div class="update-dialog-body">
          <div class="update-version-line">
            {{ app.t("update.availableSubtitle", { current: app.updateCheckReport.current_version, next: app.updateCheckReport.package.version }) }}
          </div>
          <section class="update-notes">
            <h3>{{ app.t("update.releaseNotes") }}</h3>
            <pre>{{ app.updateCheckReport.release_notes || app.t("update.noReleaseNotes") }}</pre>
          </section>
        </div>
        <div class="update-dialog-actions">
          <button type="button" :disabled="app.isWorkflowBusy" @click="app.skipUpdateVersion">
            <CircleSlash :size="17" />
            <span>{{ app.t("update.skipVersion") }}</span>
          </button>
          <div>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.cancelUpdatePrompt">
              <X :size="17" />
              <span>{{ app.t("common.cancel") }}</span>
            </button>
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.confirmUpdatePrompt">
              <Download :size="17" />
              <span>{{ app.t("update.confirmInstall") }}</span>
            </button>
          </div>
        </div>
      </section>
    </div>
  </Teleport>
</template>
