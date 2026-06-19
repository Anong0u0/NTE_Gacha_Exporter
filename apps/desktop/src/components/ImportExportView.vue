<script setup lang="ts">
import { Download, FileDown, FileJson, HardDriveDownload, HardDriveUpload, Upload } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack narrow">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Import</span>
              <h2>Update data</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('raw')">
              <Upload :size="17" />
              <span>Raw JSONL replay</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('public')">
              <FileJson :size="17" />
              <span>Public JSON</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>Selected import path</span>
              <input v-model="app.importPath" placeholder="D:\\path\\history.raw.jsonl" />
            </label>
            <select v-model="app.importMode">
              <option value="raw">Raw JSONL</option>
              <option value="public">Public JSON</option>
            </select>
            <button type="button" :disabled="app.isWorkflowBusy || !app.importPath.trim()" @click="app.runImport">Import</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Export</span>
              <h2>Shareable files</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('json')">
              <FileDown :size="17" />
              <span>Public JSON</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickExportFile('csv')">
              <Download :size="17" />
              <span>CSV</span>
            </button>
          </div>
          <div class="manual-path">
            <label class="field">
              <span>Selected export path</span>
              <input v-model="app.exportPath" placeholder="D:\\path\\history.json" />
            </label>
            <select v-model="app.exportMode">
              <option value="json">Public JSON</option>
              <option value="csv">CSV</option>
            </select>
            <button type="button" :disabled="app.isWorkflowBusy || !app.exportPath.trim()" @click="app.runExport">Export</button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Backup</span>
              <h2>Portable data snapshot</h2>
            </div>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.pickBackupFile">
              <HardDriveDownload :size="17" />
              <span>Create backup</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickRestoreFile">
              <HardDriveUpload :size="17" />
              <span>Restore backup</span>
            </button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>Selected backup path</span>
              <input v-model="app.backupPath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.runBackup">Backup</button>
          </div>
          <div class="manual-path compact-path">
            <label class="field">
              <span>Selected restore path</span>
              <input v-model="app.restorePath" placeholder="D:\\path\\nte-data-backup.zip" />
            </label>
            <button type="button" :disabled="app.isWorkflowBusy || !app.restorePath.trim()" @click="app.runRestore">Restore</button>
          </div>
        </section>

        <section v-if="app.lastReport" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastReport.source_kind }}</span>
              <h2>Last import</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Seen</span><strong>{{ app.lastReport.records_seen }}</strong></div>
            <div><span>Inserted</span><strong>{{ app.lastReport.records_inserted }}</strong></div>
            <div><span>Skipped</span><strong>{{ app.lastReport.records_skipped }}</strong></div>
          </div>
        </section>

        <section v-if="app.lastBackup" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastBackup.path }}</span>
              <h2>Last backup</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Profiles</span><strong>{{ app.lastBackup.profile_count }}</strong></div>
            <div><span>Records</span><strong>{{ app.lastBackup.record_count }}</strong></div>
          </div>
        </section>

        <section v-if="app.lastRestore" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.lastRestore.source_path }}</span>
              <h2>Last restore</h2>
            </div>
          </div>
          <div class="stat-table">
            <div><span>Profiles</span><strong>{{ app.lastRestore.profiles_seen }}</strong></div>
            <div><span>Created</span><strong>{{ app.lastRestore.profiles_created }}</strong></div>
            <div><span>Merged</span><strong>{{ app.lastRestore.profiles_merged }}</strong></div>
            <div><span>Inserted</span><strong>{{ app.lastRestore.records_inserted }}</strong></div>
            <div><span>Skipped</span><strong>{{ app.lastRestore.records_skipped }}</strong></div>
          </div>
        </section>
      </section>
</template>
