<script setup lang="ts">
import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { init, use, type ECharts } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import {
  Activity,
  BarChart3,
  Database,
  Download,
  FileJson,
  FolderInput,
  History,
  RefreshCw,
  Search,
  Settings,
  Upload,
} from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { api, type DashboardSummary, type ImportReport, type Profile, type StoredRecord } from "./api";

type ViewId = "dashboard" | "records" | "import" | "settings";
use([BarChart, GridComponent, TooltipComponent, CanvasRenderer]);

const navItems: Array<{ id: ViewId; label: string; icon: typeof BarChart3 }> = [
  { id: "dashboard", label: "Dashboard", icon: BarChart3 },
  { id: "records", label: "Records", icon: History },
  { id: "import", label: "Import / Export", icon: FolderInput },
  { id: "settings", label: "Settings", icon: Settings },
];

const activeView = ref<ViewId>("dashboard");
const profiles = ref<Profile[]>([]);
const selectedProfileId = ref<number | null>(null);
const summary = ref<DashboardSummary | null>(null);
const records = ref<StoredRecord[]>([]);
const recordTotal = ref(0);
const search = ref("");
const selectedPool = ref("");
const busy = ref(false);
const statusText = ref("Ready");
const errorText = ref("");
const importPath = ref("");
const exportPath = ref("");
const lastReport = ref<ImportReport | null>(null);
const chartEl = ref<HTMLElement | null>(null);
let chart: ECharts | null = null;

const selectedProfile = computed(() =>
  profiles.value.find((profile) => profile.id === selectedProfileId.value) ?? null,
);
const pools = computed(() => summary.value?.pools ?? []);
const topPool = computed(() => pools.value[0] ?? null);
const latest = computed(() => summary.value?.latest_records ?? []);
const canUseData = computed(() => Boolean(selectedProfileId.value));
const importModeLabel = computed(() => (importPath.value.endsWith(".jsonl") ? "Raw JSONL" : "Public JSON"));

onMounted(async () => {
  await loadProfiles();
  await refreshAll();
});

onBeforeUnmount(() => {
  chart?.dispose();
});

watch(
  () => summary.value?.timeline,
  async () => {
    await nextTick();
    renderChart();
  },
  { deep: true },
);

watch([search, selectedPool], () => {
  void loadRecords();
});

async function loadProfiles() {
  profiles.value = await api.listProfiles();
  if (!selectedProfileId.value && profiles.value.length > 0) {
    selectedProfileId.value = profiles.value[0].id;
  }
}

async function refreshAll() {
  if (!selectedProfileId.value) return;
  busy.value = true;
  errorText.value = "";
  try {
    summary.value = await api.dashboardSummary(selectedProfileId.value);
    await loadRecords();
    statusText.value = "Dashboard updated";
  } catch (error) {
    errorText.value = formatError(error);
  } finally {
    busy.value = false;
  }
}

async function loadRecords() {
  if (!selectedProfileId.value) return;
  const result = await api.listRecords(selectedProfileId.value, {
    pool_id: selectedPool.value || null,
    search: search.value || null,
    limit: 200,
    offset: 0,
  });
  records.value = result.records;
  recordTotal.value = result.total;
}

async function refreshRules() {
  await runTask("Rules refreshed", async () => {
    await api.refreshRules("zh-Hant");
    await refreshAll();
  });
}

async function runImport() {
  if (!selectedProfileId.value || !importPath.value.trim()) return;
  await runTask("Import completed", async () => {
    const path = importPath.value.trim();
    lastReport.value = path.endsWith(".jsonl")
      ? await api.importRawJsonl(selectedProfileId.value!, path, "zh-Hant")
      : await api.importPublicJson(selectedProfileId.value!, path);
    await refreshAll();
  });
}

async function exportJson() {
  if (!selectedProfileId.value || !exportPath.value.trim()) return;
  await runTask("JSON exported", () => api.exportProfileJson(selectedProfileId.value!, exportPath.value.trim()));
}

async function exportCsv() {
  if (!selectedProfileId.value || !exportPath.value.trim()) return;
  await runTask("CSV exported", () => api.exportProfileCsv(selectedProfileId.value!, exportPath.value.trim()));
}

async function pingSidecar() {
  await runTask("Sidecar responded", () => api.sidecarPing());
}

async function runTask(done: string, task: () => Promise<unknown>) {
  busy.value = true;
  errorText.value = "";
  try {
    await task();
    statusText.value = done;
  } catch (error) {
    errorText.value = formatError(error);
  } finally {
    busy.value = false;
  }
}

function renderChart() {
  if (!chartEl.value || !summary.value) return;
  chart ??= init(chartEl.value);
  chart.setOption({
    animationDuration: 280,
    grid: { top: 16, right: 18, bottom: 30, left: 36 },
    tooltip: { trigger: "axis" },
    xAxis: {
      type: "category",
      data: summary.value.timeline.map((bucket) => bucket.day.slice(5)),
      axisTick: { show: false },
    },
    yAxis: {
      type: "value",
      splitLine: { lineStyle: { color: "#e8e1d4" } },
    },
    series: [
      {
        type: "bar",
        data: summary.value.timeline.map((bucket) => bucket.record_count),
        itemStyle: { color: "#2f6f73", borderRadius: [4, 4, 0, 0] },
      },
    ],
  });
}

function formatError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">NTE</div>
        <div>
          <strong>Gacha Exporter</strong>
          <span>local tracker</span>
        </div>
      </div>

      <label class="profile-picker">
        <span>Profile</span>
        <select v-model.number="selectedProfileId" @change="refreshAll">
          <option v-for="profile in profiles" :key="profile.id" :value="profile.id">
            {{ profile.name }}
          </option>
        </select>
      </label>

      <nav class="nav-list">
        <button
          v-for="item in navItems"
          :key="item.id"
          :class="{ active: activeView === item.id }"
          type="button"
          @click="activeView = item.id"
        >
          <component :is="item.icon" :size="18" />
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </aside>

    <main class="workspace">
      <header class="topbar">
        <div>
          <span class="eyebrow">{{ selectedProfile?.name ?? "No profile" }}</span>
          <h1>{{ activeView === "dashboard" ? "Dashboard" : navItems.find((item) => item.id === activeView)?.label }}</h1>
        </div>
        <div class="status" :class="{ error: errorText }">
          {{ errorText || statusText }}
        </div>
      </header>

      <section v-if="activeView === 'dashboard'" class="view-stack">
        <section class="update-strip">
          <div>
            <span>Data update</span>
            <strong>{{ summary?.total_records ?? 0 }} records stored</strong>
          </div>
          <div class="action-row">
            <button class="primary" type="button" :disabled="busy || !canUseData" @click="refreshRules">
              <RefreshCw :size="17" />
              <span>Refresh Rules</span>
            </button>
            <button type="button" :disabled="busy || !canUseData" @click="refreshAll">
              <Activity :size="17" />
              <span>Reload</span>
            </button>
          </div>
        </section>

        <section class="metric-grid">
          <div class="metric">
            <span>Total Pulls</span>
            <strong>{{ summary?.total_records ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>Tracked Pools</span>
            <strong>{{ pools.length }}</strong>
          </div>
          <div class="metric">
            <span>Current Focus</span>
            <strong>{{ topPool?.current_pity ?? "n/a" }}</strong>
          </div>
          <div class="metric">
            <span>Latest Item</span>
            <strong>{{ topPool?.last_item_name ?? "n/a" }}</strong>
          </div>
        </section>

        <section class="split-layout">
          <div class="panel chart-panel">
            <div class="section-heading">
              <h2>Pull Timeline</h2>
              <span>{{ summary?.timeline.length ?? 0 }} days</span>
            </div>
            <div ref="chartEl" class="chart"></div>
          </div>

          <div class="panel">
            <div class="section-heading">
              <h2>Pity Tracker</h2>
              <span>{{ pools.length }} pools</span>
            </div>
            <div class="pity-list">
              <article v-for="pool in pools" :key="pool.pool_id" class="pity-row">
                <div>
                  <strong>{{ pool.group_label || pool.pool_name }}</strong>
                  <span>{{ pool.pool_name }}</span>
                </div>
                <div class="pity-value">
                  <strong>{{ pool.current_pity ?? "n/a" }}</strong>
                  <span v-if="pool.pity_limit">/ {{ pool.pity_limit }}</span>
                </div>
              </article>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="section-heading">
            <h2>Latest Records</h2>
            <button type="button" @click="activeView = 'records'">Open Records</button>
          </div>
          <div class="record-list compact">
            <div v-for="record in latest" :key="record.record_id" class="record-row">
              <span>{{ record.time ?? "" }}</span>
              <strong>{{ record.item_name ?? record.item_id }}</strong>
              <span>{{ record.pool_name ?? record.pool_id }}</span>
              <span>{{ record.roll_label ?? "" }}</span>
            </div>
          </div>
        </section>
      </section>

      <section v-else-if="activeView === 'records'" class="view-stack">
        <section class="toolbar">
          <div class="search-box">
            <Search :size="17" />
            <input v-model="search" placeholder="Search item" />
          </div>
          <select v-model="selectedPool">
            <option value="">All pools</option>
            <option v-for="pool in pools" :key="pool.pool_id" :value="pool.pool_id">
              {{ pool.group_label || pool.pool_name }}
            </option>
          </select>
          <span>{{ recordTotal }} records</span>
        </section>

        <section class="table-panel">
          <div class="record-row table-head">
            <span>Time</span>
            <span>Item</span>
            <span>Pool</span>
            <span>Roll</span>
          </div>
          <div class="record-list">
            <div v-for="record in records" :key="record.record_id" class="record-row">
              <span>{{ record.time ?? "" }}</span>
              <strong>{{ record.item_name ?? record.item_id }}</strong>
              <span>{{ record.pool_name ?? record.pool_id }}</span>
              <span>{{ record.roll_label ?? "" }}</span>
            </div>
          </div>
        </section>
      </section>

      <section v-else-if="activeView === 'import'" class="view-stack narrow">
        <section class="panel form-panel">
          <div class="section-heading">
            <h2>Import Data</h2>
            <span>{{ importModeLabel }}</span>
          </div>
          <label>
            <span>Source path</span>
            <input v-model="importPath" placeholder="D:\\path\\history.json or sample.raw.jsonl" />
          </label>
          <button class="primary" type="button" :disabled="busy || !importPath" @click="runImport">
            <Upload :size="17" />
            <span>Import</span>
          </button>
          <p v-if="lastReport" class="report">
            Seen {{ lastReport.records_seen }}, inserted {{ lastReport.records_inserted }}, skipped
            {{ lastReport.records_skipped }}.
          </p>
        </section>

        <section class="panel form-panel">
          <div class="section-heading">
            <h2>Export Data</h2>
            <span>JSON / CSV</span>
          </div>
          <label>
            <span>Output path</span>
            <input v-model="exportPath" placeholder="D:\\path\\nte-history.json" />
          </label>
          <div class="action-row">
            <button type="button" :disabled="busy || !exportPath" @click="exportJson">
              <FileJson :size="17" />
              <span>JSON</span>
            </button>
            <button type="button" :disabled="busy || !exportPath" @click="exportCsv">
              <Download :size="17" />
              <span>CSV</span>
            </button>
          </div>
        </section>
      </section>

      <section v-else class="view-stack narrow">
        <section class="panel form-panel">
          <div class="section-heading">
            <h2>Diagnostics</h2>
            <span>local sidecar</span>
          </div>
          <button type="button" :disabled="busy" @click="pingSidecar">
            <Database :size="17" />
            <span>Ping Sidecar</span>
          </button>
        </section>
      </section>
    </main>
  </div>
</template>
