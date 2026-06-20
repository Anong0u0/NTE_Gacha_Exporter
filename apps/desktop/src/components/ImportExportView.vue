<script setup lang="ts">
import { Download, FileDown, FileJson, HardDriveDownload, HardDriveUpload, Upload } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack narrow" data-agent-id="view-import-export">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("import.import") }}</span>
              <h2>{{ app.t("import.updateData") }}</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('raw')">
              <Upload :size="17" />
              <span>{{ app.t("import.rawReplay") }}</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('public')">
              <FileJson :size="17" />
              <span>{{ app.t("import.publicJson") }}</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>{{ app.t("import.selectImportPath") }}</span>
              <input v-model="app.importPath" data-agent-id="import-path" placeholder="D:\\path\\history.raw.jsonl" />
            </label>
            <select v-model="app.importMode" data-agent-id="import-mode">
              <option value="raw">{{ app.t("import.rawJsonl") }}</option>
              <option value="public">{{ app.t("import.publicJson") }}</option>
            </select>
            <button type="button" data-agent-id="import-run" :disabled="app.isWorkflowBusy || !app.importPath.trim()" @click="app.runImport">{{ app.t("import.import") }}</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("import.export") }}</span>
              <h2>{{ app.t("import.shareableFiles") }}</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('json')">
              <FileDown :size="17" />
              <span>{{ app.t("import.publicJson") }}</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('csv')">
              <Download :size="17" />
              <span>CSV</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>{{ app.t("import.selectExportPath") }}</span>
              <input v-model="app.exportPath" placeholder="D:\\path\\history.json" />
            </label>
            <select v-model="app.exportMode">
              <option value="json">{{ app.t("import.publicJson") }}</option>
              <option value="csv">CSV</option>
            </select>
            <button type="button" :disabled="app.isWorkflowBusy || !app.exportPath.trim()" @click="app.runExport">{{ app.t("import.export") }}</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("import.backup") }}</span>
              <h2>{{ app.t("import.backupSnapshot") }}</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickBackupFile">
              <HardDriveDownload :size="17" />
              <span>{{ app.t("import.createBackup") }}</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickRestoreFile">
              <HardDriveUpload :size="17" />
              <span>{{ app.t("import.restoreBackup") }}</span>
            </button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>{{ app.t("import.selectBackupPath") }}</span>
              <input v-model="app.backupPath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.runBackup">{{ app.t("import.backup") }}</button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>{{ app.t("import.selectRestorePath") }}</span>
              <input v-model="app.restorePath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="app.isWorkflowBusy || !app.restorePath.trim()" @click="app.runRestore">{{ app.t("import.restore") }}</button>
          </div>
        </section>

        <section v-if="app.lastReport" class="panel" data-agent-id="last-import-panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastReport.source_kind }}</span>
              <h2>{{ app.t("import.lastImport") }}</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>{{ app.t("common.seen") }}</span><strong>{{ app.lastReport.records_seen }}</strong></div>
            <div><span>{{ app.t("common.inserted") }}</span><strong>{{ app.lastReport.records_inserted }}</strong></div>
            <div><span>{{ app.t("common.skipped") }}</span><strong>{{ app.lastReport.records_skipped }}</strong></div>
          </div>
        </section>

        <section v-if="app.lastBackup" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastBackup.path }}</span>
              <h2>{{ app.t("import.lastBackup") }}</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>{{ app.t("common.profiles") }}</span><strong>{{ app.lastBackup.profile_count }}</strong></div>
            <div><span>{{ app.t("common.records") }}</span><strong>{{ app.lastBackup.record_count }}</strong></div>
          </div>
        </section>

        <section v-if="app.lastRestore" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastRestore.source_path }}</span>
              <h2>{{ app.t("import.lastRestore") }}</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>{{ app.t("common.profiles") }}</span><strong>{{ app.lastRestore.profiles_seen }}</strong></div>
            <div><span>{{ app.t("common.created") }}</span><strong>{{ app.lastRestore.profiles_created }}</strong></div>
            <div><span>{{ app.t("common.merged") }}</span><strong>{{ app.lastRestore.profiles_merged }}</strong></div>
            <div><span>{{ app.t("common.inserted") }}</span><strong>{{ app.lastRestore.records_inserted }}</strong></div>
            <div><span>{{ app.t("common.skipped") }}</span><strong>{{ app.lastRestore.records_skipped }}</strong></div>
          </div>
        </section>
      </section>
</template>
