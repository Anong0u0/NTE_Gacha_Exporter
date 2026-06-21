<script setup lang="ts">
import { ArchiveRestore, Database, DatabaseBackup, Download, FileDown, FileJson, FileUp, HardDriveUpload, RefreshCw, Settings, Stethoscope, Trash2 } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack settings-workbench" data-agent-id="view-settings">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("settings.runtime") }}</span>
              <h2>{{ app.t("common.settings") }}</h2>
            </div>
          </div>
          <div class="form-grid runtime-grid">
            <label class="field">
              <span>{{ app.t("settings.uiLocale") }}</span>
              <select v-model="app.uiLocale" :disabled="app.isWorkflowBusy">
                <option v-for="item in app.uiLocales" :key="item" :value="item">{{ app.uiLocaleName(item) }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("settings.dataLocale") }}</span>
              <select v-model="app.locale" :disabled="app.isWorkflowBusy">
                <option v-for="item in app.locales" :key="item" :value="item">{{ item }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("settings.updateChannel") }}</span>
              <select v-model="app.settingsUpdateChannel" :disabled="app.isWorkflowBusy">
                <option value="stable">{{ app.t("update.stable") }}</option>
                <option value="beta">{{ app.t("update.beta") }}</option>
              </select>
            </label>
          </div>
          <div class="settings-actions">
            <label class="check-field">
              <input v-model="app.settingsCheckUpdates" type="checkbox" :disabled="app.isWorkflowBusy" />
              <span>{{ app.t("settings.checkUpdatesStartup") }}</span>
            </label>
            <button type="button" data-agent-id="runtime-ping" :disabled="app.isWorkflowBusy" @click="app.pingRuntime">
              <Database :size="17" />
              <span>{{ app.t("settings.pingRuntime") }}</span>
            </button>
            <button type="button" data-agent-id="doctor-run" :disabled="app.isWorkflowBusy" @click="app.runDoctor">
              <Stethoscope :size="17" />
              <span>{{ app.t("common.doctor") }}</span>
            </button>
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.saveSettings">
              <Settings :size="17" />
              <span>{{ app.t("common.save") }}</span>
            </button>
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
            <div><span>{{ app.t("common.staged") }}</span><strong>{{ app.stagedUpdate?.package.version ?? app.updateStatus?.staged_version ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.checkForUpdates(true)">
              <RefreshCw :size="17" />
              <span>{{ app.t("settings.checkUpdates") }}</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="app.isWorkflowBusy || !app.updateCheckReport?.package"
              @click="app.downloadUpdate"
            >
              <Download :size="17" />
              <span>{{ app.t("common.download") }}</span>
            </button>
            <button
              type="button"
              :disabled="app.isWorkflowBusy || !(app.stagedUpdate?.package.version || app.updateStatus?.staged_version)"
              @click="app.installUpdate"
            >
              <HardDriveUpload :size="17" />
              <span>{{ app.t("settings.restartUpdate") }}</span>
            </button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("settings.assetsPack") }}</span>
              <h2>{{ app.t("settings.visualAssets") }}</h2>
            </div>
          </div>
          <div class="stat-table compact">
            <div><span>{{ app.t("common.status") }}</span><strong class="stat-text">{{ app.assetsPackSummary }}</strong></div>
            <div><span>{{ app.t("common.version") }}</span><strong>{{ app.assetsPackStatus?.installed_app_version ?? "-" }}</strong></div>
            <div><span>{{ app.t("settings.assetImages") }}</span><strong>{{ app.assetsPackStatus?.file_count ?? 0 }}</strong></div>
            <div><span>{{ app.t("common.available") }}</span><strong class="stat-text">{{ app.assetsPackCheckReport?.package?.app_version ?? "-" }}</strong></div>
            <div><span>{{ app.t("common.source") }}</span><strong class="stat-text">{{ app.assetsPackStatus?.source_commit?.slice(0, 12) ?? "-" }}</strong></div>
            <div><span>{{ app.t("settings.map") }}</span><strong class="stat-text">{{ app.assetsPackStatus?.installed_map_hash?.slice(0, 12) ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" data-agent-id="assets-check" :disabled="app.isWorkflowBusy" @click="app.checkAssetsPack">
              <RefreshCw :size="17" />
              <span>{{ app.t("settings.checkAssets") }}</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="app.isWorkflowBusy || !app.assetsPackCheckReport?.package"
              @click="app.downloadAssetsPack"
            >
              <Download :size="17" />
              <span>{{ app.t("settings.downloadAssets") }}</span>
            </button>
            <button
              class="danger"
              type="button"
              :disabled="app.isWorkflowBusy || !app.assetsPackStatus?.installed"
              @click="app.removeAssetsPack"
            >
              <Trash2 :size="17" />
              <span>{{ app.t("common.remove") }}</span>
            </button>
          </div>
          <div v-if="app.lastAssetsPackInstall" class="asset-pack-note">
            {{ app.t("settings.installedAssets", { count: app.lastAssetsPackInstall.file_count, commit: app.lastAssetsPackInstall.source_commit.slice(0, 12) }) }}
          </div>
        </section>

        <section v-if="app.doctorReport" class="panel settings-doctor-panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">exit {{ app.doctorReport.exit_code }}</span>
              <h2>{{ app.t("common.doctor") }}</h2>
            </div>
            <FileJson :size="18" />
          </div>
          <pre>{{ app.doctorReport.lines.join("\n") }}</pre>
        </section>
      </section>
</template>
