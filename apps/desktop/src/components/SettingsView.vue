<script setup lang="ts">
import { ArchiveRestore, DatabaseBackup, Download, FileDown, FileJson, FileUp, RefreshCw, Stethoscope } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();

function selectValue(event: Event) {
  return (event.target as HTMLSelectElement).value;
}

function checkedValue(event: Event) {
  return (event.target as HTMLInputElement).checked;
}
</script>

<template>
      <section class="view-stack settings-workbench" data-agent-id="view-settings">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("settings.runtime") }}</span>
              <h2>{{ app.t("common.settings") }}</h2>
            </div>
            <button type="button" data-agent-id="diagnostic-open" :disabled="app.isWorkflowBusy" @click="app.openDiagnosticPrompt">
              <Stethoscope :size="17" />
              <span>{{ app.t("common.doctor") }}</span>
            </button>
          </div>
          <div class="form-grid runtime-grid">
            <label class="field">
              <span>{{ app.t("settings.uiLocale") }}</span>
              <select :value="app.uiLocale" :disabled="app.isWorkflowBusy" @change="app.setUiLocale(selectValue($event))">
                <option v-for="item in app.uiLocales" :key="item" :value="item">{{ app.uiLocaleName(item) }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("settings.dataLocale") }}</span>
              <select :value="app.locale" :disabled="app.isWorkflowBusy" @change="app.setDataLocale(selectValue($event))">
                <option v-for="item in app.locales" :key="item" :value="item">{{ item }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("settings.updateChannel") }}</span>
              <select :value="app.settingsUpdateChannel" :disabled="app.isWorkflowBusy" @change="app.setUpdateChannel(selectValue($event))">
                <option value="stable">{{ app.t("update.stable") }}</option>
                <option value="beta">{{ app.t("update.beta") }}</option>
              </select>
            </label>
          </div>
        </section>

        <section class="panel data-action-panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("settings.dataActions") }}</span>
              <h2>{{ app.t("import.import") }} / {{ app.t("import.export") }}</h2>
            </div>
          </div>
          <div class="data-action-row">
            <button class="primary" type="button" data-agent-id="settings-import-raw" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('raw')">
              <FileUp :size="17" />
              <span>{{ app.t("import.rawReplay") }}</span>
            </button>
            <button type="button" data-agent-id="settings-import-public" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('public')">
              <FileJson :size="17" />
              <span>{{ app.t("import.publicJson") }}</span>
            </button>
            <button type="button" data-agent-id="settings-export-json" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('json')">
              <FileDown :size="17" />
              <span>{{ app.t("import.export") }} {{ app.t("import.publicJson") }}</span>
            </button>
            <button type="button" data-agent-id="settings-export-csv" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('csv')">
              <FileDown :size="17" />
              <span>{{ app.t("import.export") }} CSV</span>
            </button>
            <button type="button" data-agent-id="settings-backup-create" :disabled="app.isWorkflowBusy" @click="app.pickBackupFile">
              <DatabaseBackup :size="17" />
              <span>{{ app.t("import.createBackup") }}</span>
            </button>
            <button type="button" data-agent-id="settings-backup-restore" :disabled="app.isWorkflowBusy" @click="app.pickRestoreFile">
              <ArchiveRestore :size="17" />
              <span>{{ app.t("import.restoreBackup") }}</span>
            </button>
          </div>
          <div v-if="app.dataOperationSummary" class="operation-summary" data-agent-id="settings-data-summary">
            {{ app.dataOperationSummary }}
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("settings.updater") }}</span>
              <h2>{{ app.t("settings.portableUpdate") }}</h2>
            </div>
          </div>
          <div class="stat-table compact">
            <div><span>{{ app.t("common.current") }}</span><strong>{{ app.updateStatus?.current_version ?? "-" }}</strong></div>
            <div><span>{{ app.t("common.layout") }}</span><strong>{{ app.updateStatus?.supported_layout ? app.t("settings.portable") : app.t("settings.unsupported") }}</strong></div>
            <div><span>{{ app.t("common.available") }}</span><strong>{{ app.updateCheckReport?.package?.version ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.checkForUpdates()">
              <RefreshCw :size="17" />
              <span>{{ app.t("settings.checkUpdates") }}</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="app.isWorkflowBusy || !app.canOpenDismissedUpdatePrompt"
              @click="app.openUpdatePrompt"
            >
              <Download :size="17" />
              <span>{{ app.t("settings.updateNow") }}</span>
            </button>
            <label class="check-field">
              <input :checked="app.settingsCheckUpdates" type="checkbox" :disabled="app.isWorkflowBusy" @change="app.setCheckUpdatesOnStartup(checkedValue($event))" />
              <span>{{ app.t("settings.checkUpdatesStartup") }}</span>
            </label>
          </div>
        </section>
      </section>
</template>
