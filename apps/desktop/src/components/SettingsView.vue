<script setup lang="ts">
import { Database, Download, FileJson, HardDriveUpload, RefreshCw, Settings, Stethoscope, Trash2 } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack narrow" data-agent-id="view-settings">
        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Runtime</span>
              <h2>Settings</h2>
            </div>
          </div>
          <div class="form-grid">
            <label class="field">
              <span>Profile</span>
              <select v-model="app.activeProfileName" :disabled="app.isWorkflowBusy">
                <option v-for="profile in app.profiles" :key="profile.name" :value="profile.name">
                  {{ profile.name }}
                </option>
              </select>
            </label>
            <label class="field">
              <span>Locale</span>
              <select v-model="app.locale" :disabled="app.isWorkflowBusy">
                <option v-for="item in app.locales" :key="item" :value="item">{{ item }}</option>
              </select>
            </label>
            <label class="field">
              <span>Update channel</span>
              <select v-model="app.settingsUpdateChannel" :disabled="app.isWorkflowBusy">
                <option value="stable">stable</option>
                <option value="beta">beta</option>
              </select>
            </label>
            <label class="check-field">
              <input v-model="app.settingsCheckUpdates" type="checkbox" :disabled="app.isWorkflowBusy" />
              <span>Check updates on startup</span>
            </label>
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.saveSettings">
              <Settings :size="17" />
              <span>Save settings</span>
            </button>
            <button type="button" data-agent-id="runtime-ping" :disabled="app.isWorkflowBusy" @click="app.pingRuntime">
              <Database :size="17" />
              <span>Ping runtime</span>
            </button>
            <button type="button" data-agent-id="doctor-run" :disabled="app.isWorkflowBusy" @click="app.runDoctor">
              <Stethoscope :size="17" />
              <span>Doctor</span>
            </button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Updater</span>
              <h2>Portable update</h2>
            </div>
          </div>
          <div class="stat-table compact">
            <div><span>Current</span><strong>{{ app.updateStatus?.current_version ?? "-" }}</strong></div>
            <div><span>Layout</span><strong>{{ app.updateStatus?.supported_layout ? "Portable" : "Unsupported" }}</strong></div>
            <div><span>Available</span><strong>{{ app.updateCheckReport?.package?.version ?? "-" }}</strong></div>
            <div><span>Staged</span><strong>{{ app.stagedUpdate?.package.version ?? app.updateStatus?.staged_version ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.checkForUpdates(true)">
              <RefreshCw :size="17" />
              <span>Check updates</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="app.isWorkflowBusy || !app.updateCheckReport?.package"
              @click="app.downloadUpdate"
            >
              <Download :size="17" />
              <span>Download</span>
            </button>
            <button
              type="button"
              :disabled="app.isWorkflowBusy || !(app.stagedUpdate?.package.version || app.updateStatus?.staged_version)"
              @click="app.installUpdate"
            >
              <HardDriveUpload :size="17" />
              <span>Restart to update</span>
            </button>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Assets Pack</span>
              <h2>Visual assets</h2>
            </div>
          </div>
          <div class="stat-table compact">
            <div><span>Status</span><strong class="stat-text">{{ app.assetsPackSummary }}</strong></div>
            <div><span>Version</span><strong>{{ app.assetsPackStatus?.installed_app_version ?? "-" }}</strong></div>
            <div><span>Images</span><strong>{{ app.assetsPackStatus?.file_count ?? 0 }}</strong></div>
            <div><span>Available</span><strong class="stat-text">{{ app.assetsPackCheckReport?.package?.app_version ?? "-" }}</strong></div>
            <div><span>Source</span><strong class="stat-text">{{ app.assetsPackStatus?.source_commit?.slice(0, 12) ?? "-" }}</strong></div>
            <div><span>Map</span><strong class="stat-text">{{ app.assetsPackStatus?.installed_map_hash?.slice(0, 12) ?? "-" }}</strong></div>
          </div>
          <div class="action-row">
            <button type="button" data-agent-id="assets-check" :disabled="app.isWorkflowBusy" @click="app.checkAssetsPack">
              <RefreshCw :size="17" />
              <span>Check assets</span>
            </button>
            <button
              class="primary"
              type="button"
              :disabled="app.isWorkflowBusy || !app.assetsPackCheckReport?.package"
              @click="app.downloadAssetsPack"
            >
              <Download :size="17" />
              <span>Download assets</span>
            </button>
            <button
              type="button"
              :disabled="app.isWorkflowBusy || !app.assetsPackStatus?.installed"
              @click="app.removeAssetsPack"
            >
              <Trash2 :size="17" />
              <span>Remove</span>
            </button>
          </div>
          <div v-if="app.lastAssetsPackInstall" class="asset-pack-note">
            Installed {{ app.lastAssetsPackInstall.file_count }} images from {{ app.lastAssetsPackInstall.source_commit.slice(0, 12) }}.
          </div>
        </section>

        <section v-if="app.doctorReport" class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">exit {{ app.doctorReport.exit_code }}</span>
              <h2>Doctor</h2>
            </div>
            <FileJson :size="18" />
          </div>
          <pre>{{ app.doctorReport.lines.join("\n") }}</pre>
        </section>
      </section>
</template>
