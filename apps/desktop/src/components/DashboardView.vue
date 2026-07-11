<script setup lang="ts">
import { Calculator, ChevronDown, ChevronUp, Crown, Eye, EyeOff, Gem, Hash, ListFilter, ListOrdered, Target, Ticket, TrendingDown, TrendingUp, X, Zap } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useAppContext } from "../app/context";
import DashboardSummaryMetric from "./DashboardSummaryMetric.vue";

const app = useAppContext();
const fiveWallGrid = ref<HTMLElement | null>(null);
const fiveWallOverflowsOneRow = ref(false);
const fiveWallRowHeight = ref(84);
const fiveWallAfterFirstRowRecordIds = ref<ReadonlySet<string>>(new Set());
const fiveWallFrameLayouts = ref<FiveWallFrameLayout[]>([]);
const fiveWallGridSize = ref({ width: 0, height: 0 });
let fiveWallResizeObserver: ResizeObserver | null = null;
let fiveWallLayoutSignature = "";

const fiveWallFrameOffset = 4;
const fiveWallRowTolerance = 1;
const fiveWallGroupPalette = ["#4E79A7", "#F28E2B", "#59A14F", "#B07AA1", "#E15759", "#76B7B2"] as const;

type SummaryMetricTone = "default" | "rarity-5" | "rarity-4" | "currency";

type SummaryMetricCard = {
  key: string;
  title: string;
  value: string | number;
  icon: Component;
  tone?: SummaryMetricTone;
  progressWidth?: string;
  className: string;
};

type FiveWallItemBox = {
  groupKey: string;
  recordId: string;
  left: number;
  top: number;
  right: number;
  bottom: number;
};

type FiveWallFrameSegment = {
  left: number;
  top: number;
  right: number;
  bottom: number;
};

type FiveWallFrameLayout = {
  key: string;
  segments: FiveWallFrameSegment[];
  paletteIndex: number;
  hitCount: number;
  displayDistance: number;
  distanceMode: string;
  recordIds: string;
  label: string;
  crossesRows: boolean;
  crossesFirstRow: boolean;
};

const dashboardScopeKey = computed(() =>
  app.selectedDashboardScope.kind === "banner"
    ? `${app.selectedDashboardScope.pool_kind}:${app.selectedDashboardScope.banner_id}`
    : app.selectedDashboardScope.pool_kind,
);
const fiveWallShellClasses = computed(() => ({
  "is-collapsible": fiveWallOverflowsOneRow.value,
  "is-collapsed": fiveWallOverflowsOneRow.value && !app.fiveWallExpanded,
  "is-expanded": fiveWallOverflowsOneRow.value && app.fiveWallExpanded,
}));
const fiveWallShellStyle = computed<Record<string, string>>(() => ({
  "--five-wall-row-height": `${fiveWallRowHeight.value}px`,
}));
const summaryMetricCards = computed<SummaryMetricCard[]>(() => {
  const summary = app.selectedSummary;
  const cards: SummaryMetricCard[] = [
    {
      key: "five-pity",
      title: app.t("dashboard.selected5Pity"),
      value: `${summary?.current_pity ?? 0}/${summary?.hard_pity ?? 0}`,
      icon: Crown,
      tone: "rarity-5",
      progressWidth: summaryProgressWidth(summary?.current_pity, summary?.hard_pity),
      className: "summary-metric-card--five-pity",
    },
    {
      key: "four-progress",
      title: app.summaryProgressLabel(summary),
      value: app.formatTenPullProgressSummary(summary?.current_ten_pull_progress),
      icon: Zap,
      tone: "rarity-4",
      progressWidth: summaryProgressWidth(summary?.current_ten_pull_progress, 10),
      className: "summary-metric-card--four-progress",
    },
    {
      key: "total-pulls",
      title: app.t("dashboard.totalPulls"),
      value: summary?.total_pulls ?? 0,
      icon: Ticket,
      className: "summary-metric-card--total-pulls",
    },
    {
      key: "currency",
      title: app.t("dashboard.convertedCurrency"),
      value: app.pullCurrency(summary?.total_pulls),
      icon: Gem,
      tone: "currency",
      className: "summary-metric-card--currency",
    },
    {
      key: "avg-five-pity",
      title: app.t("dashboard.avg5Pity"),
      value: app.numberOrDash(summary?.average_5star_pity),
      icon: TrendingUp,
      className: "summary-metric-card--avg-five-pity",
    },
    {
      key: "avg-four-pity",
      title: app.t("dashboard.avg4Pity"),
      value: app.numberOrDash(summary?.average_4star_pity),
      icon: TrendingDown,
      className: "summary-metric-card--avg-four-pity",
    },
  ];

  if (summary?.pool_kind === "fork_lottery") {
    cards.push({
      key: "fork-win-rate",
      title: app.t("dashboard.forkWinRate"),
      value: app.forkWinRate(summary),
      icon: Target,
      className: "summary-metric-card--fork-win-rate",
    });
  }

  return cards;
});
const summaryMetricColumns = computed(() => ({
  pity: summaryMetricCards.value.slice(0, 2),
  detail: summaryMetricCards.value.slice(2),
}));

function summaryProgressWidth(current?: number | null, max?: number | null) {
  const total = max ?? 0;
  if (total <= 0) return "0%";
  return `${Math.min(100, Math.max(0, ((current ?? 0) / total) * 100))}%`;
}

function updateFiveWallLayout() {
  const grid = fiveWallGrid.value;
  if (!grid) {
    clearFiveWallLayout();
    return;
  }
  const items = [...grid.querySelectorAll<HTMLElement>(".five-wall-item")];
  if (!items.length) {
    clearFiveWallLayout();
    return;
  }

  const gridRect = grid.getBoundingClientRect();
  const gridWidth = roundFiveWallCoordinate(gridRect.width);
  const gridHeight = roundFiveWallCoordinate(gridRect.height);
  const boxes = items.map((item) => fiveWallItemBox(item, gridRect));
  const firstTop = boxes[0].top;
  const afterFirstRowRecordIds = new Set(
    boxes.filter((box) => box.top > firstTop + fiveWallRowTolerance).map((box) => box.recordId),
  );
  const frames = buildFiveWallFrameLayouts(boxes, gridWidth, gridHeight, firstTop);
  const rowHeight = roundFiveWallCoordinate(boxes[0].bottom - boxes[0].top);
  const overflowsOneRow = afterFirstRowRecordIds.size > 0;
  const signature = JSON.stringify({
    width: gridWidth,
    height: gridHeight,
    rowHeight,
    afterFirstRowRecordIds: [...afterFirstRowRecordIds],
    frames,
  });

  if (signature === fiveWallLayoutSignature) return;
  fiveWallLayoutSignature = signature;
  fiveWallGridSize.value = { width: gridWidth, height: gridHeight };
  fiveWallRowHeight.value = rowHeight;
  fiveWallOverflowsOneRow.value = overflowsOneRow;
  fiveWallAfterFirstRowRecordIds.value = afterFirstRowRecordIds;
  fiveWallFrameLayouts.value = frames;
}

function clearFiveWallLayout() {
  fiveWallLayoutSignature = "";
  fiveWallGridSize.value = { width: 0, height: 0 };
  fiveWallOverflowsOneRow.value = false;
  fiveWallAfterFirstRowRecordIds.value = new Set();
  fiveWallFrameLayouts.value = [];
}

function fiveWallItemBox(item: HTMLElement, gridRect: DOMRect): FiveWallItemBox {
  const rect = item.getBoundingClientRect();
  return {
    groupKey: item.dataset.fiveWallGroupKey ?? "",
    recordId: item.dataset.recordId ?? "",
    left: roundFiveWallCoordinate(rect.left - gridRect.left),
    top: roundFiveWallCoordinate(rect.top - gridRect.top),
    right: roundFiveWallCoordinate(rect.right - gridRect.left),
    bottom: roundFiveWallCoordinate(rect.bottom - gridRect.top),
  };
}

function buildFiveWallFrameLayouts(
  boxes: FiveWallItemBox[],
  gridWidth: number,
  gridHeight: number,
  firstTop: number,
): FiveWallFrameLayout[] {
  const boxesByGroup = new Map<string, FiveWallItemBox[]>();
  for (const box of boxes) {
    const groupBoxes = boxesByGroup.get(box.groupKey) ?? [];
    groupBoxes.push(box);
    boxesByGroup.set(box.groupKey, groupBoxes);
  }

  return app.displayedLatestFiveStarGroups.filter((group) => group.hits.length > 1).flatMap((group, paletteOrder) => {
    const groupBoxes = boxesByGroup.get(group.key) ?? [];
    if (groupBoxes.length !== group.hits.length) return [];
    const segments = buildFiveWallFrameSegments(groupBoxes, gridWidth, gridHeight);

    return [{
      key: group.key,
      segments,
      paletteIndex: paletteOrder % fiveWallGroupPalette.length,
      hitCount: group.hits.length,
      displayDistance: group.displayDistance,
      distanceMode: group.distanceMode,
      recordIds: group.recordIds,
      label: app.fiveWallGroupItemLabel(group),
      crossesRows: segments.length > 1,
      crossesFirstRow:
        groupBoxes.some((box) => Math.abs(box.top - firstTop) <= fiveWallRowTolerance) &&
        groupBoxes.some((box) => box.top > firstTop + fiveWallRowTolerance),
    }];
  });
}

function buildFiveWallFrameSegments(
  boxes: FiveWallItemBox[],
  gridWidth: number,
  gridHeight: number,
): FiveWallFrameSegment[] {
  const rows: FiveWallFrameSegment[] = [];

  for (const box of boxes) {
    const row = rows.at(-1);
    if (row && Math.abs(box.top - (row.top + fiveWallFrameOffset)) <= fiveWallRowTolerance) {
      row.right = box.right + fiveWallFrameOffset;
      row.bottom = Math.max(row.bottom, box.bottom + fiveWallFrameOffset);
      continue;
    }
    rows.push({
      left: box.left - fiveWallFrameOffset,
      top: box.top - fiveWallFrameOffset,
      right: box.right + fiveWallFrameOffset,
      bottom: box.bottom + fiveWallFrameOffset,
    });
  }

  return rows.map((row) => ({
    left: clampFiveWallFrame(row.left, gridWidth),
    top: clampFiveWallFrame(row.top, gridHeight),
    right: clampFiveWallFrame(row.right, gridWidth),
    bottom: clampFiveWallFrame(row.bottom, gridHeight),
  }));
}

function clampFiveWallFrame(value: number, limit: number) {
  return roundFiveWallCoordinate(Math.min(Math.max(value, 0.5), Math.max(0.5, limit - 0.5)));
}

function roundFiveWallCoordinate(value: number) {
  return Math.round(value * 100) / 100;
}

function isFiveWallAfterFirstRow(recordId: string) {
  return fiveWallAfterFirstRowRecordIds.value.has(recordId);
}

function fiveWallGroupFrameStyle(frame: FiveWallFrameLayout) {
  return { "--five-wall-group-color": fiveWallGroupPalette[frame.paletteIndex] };
}

function fiveWallGroupCountStyle(frame: FiveWallFrameLayout) {
  const segment = frame.segments[0];
  return { left: `${segment.right}px`, top: `${segment.top}px` };
}

function fiveWallGroupDistanceStyle(frame: FiveWallFrameLayout, useFirstSegment = false) {
  const segment = useFirstSegment ? frame.segments[0] : frame.segments.at(-1)!;
  return { left: `${segment.right}px`, top: `${segment.bottom}px` };
}

async function refreshFiveWallLayout() {
  await nextTick();
  updateFiveWallLayout();
}

onMounted(() => {
  void refreshFiveWallLayout();
  if (fiveWallGrid.value) {
    fiveWallResizeObserver = new ResizeObserver(updateFiveWallLayout);
    fiveWallResizeObserver.observe(fiveWallGrid.value);
  }
});

onBeforeUnmount(() => {
  fiveWallResizeObserver?.disconnect();
});

watch(
  () => [
    app.displayedLatestFiveStarGroups
      .map((group) => `${group.key}:${group.hits.length}:${group.displayDistance}:${group.distanceMode}`)
      .join("|"),
    app.selectedPoolKind,
    dashboardScopeKey.value,
    app.latestFiveStarWallMode,
    app.latestFiveStarDistanceMode,
  ],
  () => {
    void refreshFiveWallLayout();
  },
);
</script>

<template>
      <section class="view-stack dashboard-workbench" data-agent-id="view-dashboard">
        <section class="pool-strip">
          <button
            v-for="pool in app.allPoolSummaries"
            :key="pool.pool_kind"
            :class="{ active: app.isSelectedDashboardPool(pool.pool_kind) }"
            type="button"
            :data-pool-kind="pool.pool_kind"
            :data-current-pity="pool.current_pity"
            :data-hard-pity="pool.hard_pity"
            :data-current-ten-pull-progress="pool.current_ten_pull_progress ?? ''"
            :aria-pressed="app.isSelectedDashboardPool(pool.pool_kind)"
            @click="app.selectDashboardPool(pool.pool_kind)"
          >
            <span class="pool-main">
              <strong>{{ pool.label }}</strong>
              <small>{{ app.kindLabels[pool.pool_kind] }} · {{ app.t("dashboard.pulls", { count: pool.total_pulls }) }}</small>
            </span>
            <span class="pool-pity-stack">
              <span class="pool-pity-line rarity-5">
                <span>5★</span>
                <strong>{{ app.formatPityRatio(pool.current_pity, pool.hard_pity) }}</strong>
              </span>
              <span class="pool-pity-line rarity-4">
                <span>4★</span>
                <strong>{{ app.formatPityRatio(pool.current_ten_pull_progress ?? 0, 10) }}</strong>
              </span>
            </span>
            <span class="pool-latest">{{ app.t("dashboard.latest5") }} · {{ app.latestFiveStarNameForPool(pool) }}</span>
          </button>
        </section>

        <section v-if="app.showDashboardBannerRail" class="banner-thumb-rail" data-agent-id="dashboard-banner-rail">
          <button
            class="banner-rail-all"
            type="button"
            :class="{ active: app.isDashboardPoolScope }"
            :aria-pressed="app.isDashboardPoolScope"
            :title="app.t('common.all')"
            :aria-label="app.t('common.all')"
            @click="app.selectDashboardPool(app.selectedPoolKind)"
          >
            <span>{{ app.t("common.all") }}</span>
          </button>
          <button
            v-for="banner in app.selectedPoolBannerSummaries"
            :key="banner.banner_id"
            :class="{ active: app.isSelectedDashboardBanner(banner.banner_id), 'is-fork-banner': banner.banner_type === 'fork' || banner.pool_kind === 'fork_lottery' }"
            type="button"
            :title="app.bannerTitle(banner)"
            :aria-label="app.bannerTitle(banner)"
            :aria-pressed="app.isSelectedDashboardBanner(banner.banner_id)"
            @click="app.selectDashboardBanner(banner)"
          >
            <span v-if="app.hasBannerVisual(banner)" class="rail-thumb">
              <img :src="app.bannerVisualUrl(banner)" alt="" />
            </span>
            <span v-else class="rail-thumb empty">
              <span>{{ app.bannerTitle(banner) }}</span>
            </span>
          </button>
        </section>

        <section class="panel latest-five-wall" data-agent-id="dashboard-latest-five-wall">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.selectedScopeLabel ?? app.t("dashboard.pool") }}</span>
              <h2>{{ app.t("dashboard.latest5") }}</h2>
            </div>
            <div class="latest-five-actions">
              <button
                v-if="app.showLatestFiveStarDistanceModeToggle"
                type="button"
                class="ghost latest-distance-toggle"
                :aria-pressed="app.latestFiveStarDistanceMode === 'cost'"
                :title="app.latestFiveStarDistanceModeLabel()"
                :aria-label="app.latestFiveStarDistanceModeLabel()"
                @click="app.toggleLatestFiveStarDistanceMode"
              >
                <component :is="app.latestFiveStarDistanceMode === 'cost' ? Calculator : Hash" :size="16" />
                <span>{{ app.latestFiveStarDistanceModeLabel() }}</span>
              </button>
              <button
                v-if="app.showLatestFiveStarWallModeToggle"
                type="button"
                class="ghost latest-item-toggle"
                :aria-pressed="app.latestFiveStarWallMode === 'all'"
                :title="app.latestFiveStarWallToggleLabel()"
                :aria-label="app.latestFiveStarWallToggleLabel()"
                @click="app.toggleLatestFiveStarWallMode"
              >
                <component :is="app.latestFiveStarWallMode === 'all' ? Eye : EyeOff" :size="16" />
                <span>{{ app.latestFiveStarWallToggleLabel() }}</span>
              </button>
              <button
                type="button"
                class="ghost"
                :disabled="!app.visibleLatestFiveStarHits.length || app.isWorkflowBusy"
                @click="app.showDashboardFiveStarRecords"
              >
                <ListFilter :size="16" />
                <span>{{ app.t("dashboard.viewDetailedRecords") }}</span>
              </button>
            </div>
          </div>
          <div class="five-wall-shell" :class="fiveWallShellClasses" :style="fiveWallShellStyle">
            <div ref="fiveWallGrid" class="five-wall-grid">
              <svg
                v-if="fiveWallFrameLayouts.length"
                class="five-wall-frame-layer"
                :width="fiveWallGridSize.width"
                :height="fiveWallGridSize.height"
                :viewBox="`0 0 ${fiveWallGridSize.width} ${fiveWallGridSize.height}`"
                aria-hidden="true"
              >
                <g
                  v-for="frame in fiveWallFrameLayouts"
                  :key="frame.key"
                  class="five-wall-group-frame"
                  :data-five-wall-group-key="frame.key"
                  :data-five-wall-group-distance="frame.displayDistance"
                  :data-five-wall-group-distance-mode="frame.distanceMode"
                  :data-five-wall-group-hit-count="frame.hitCount"
                  :data-five-wall-group-record-ids="frame.recordIds"
                  :data-five-wall-segment-count="frame.segments.length"
                  :data-five-wall-crosses-row="frame.crossesRows"
                  :data-five-wall-crosses-first-row="frame.crossesFirstRow"
                  :data-five-wall-palette-index="frame.paletteIndex"
                  :style="fiveWallGroupFrameStyle(frame)"
                >
                  <rect
                    v-for="(segment, segmentIndex) in frame.segments"
                    :key="segmentIndex"
                    class="five-wall-frame-segment"
                    :data-five-wall-segment-index="segmentIndex"
                    :x="segment.left"
                    :y="segment.top"
                    :width="segment.right - segment.left"
                    :height="segment.bottom - segment.top"
                    rx="10"
                  />
                </g>
              </svg>

              <template v-for="group in app.displayedLatestFiveStarGroups" :key="group.key">
                <div
                  v-for="(hit, hitIndex) in group.hits"
                  :key="hit.record.record_id"
                  class="five-wall-item"
                  :class="[app.recordRarityClass(hit.record), { 'is-after-first-row': isFiveWallAfterFirstRow(hit.record.record_id) }]"
                  :data-record-id="hit.record.record_id"
                  :data-source-order="hit.record.source_order"
                  :data-time="hit.record.time ?? ''"
                  :data-pool-kind="hit.record.pool_kind"
                  :data-pool-id="hit.record.pool_id"
                  :data-item-id="hit.record.item_id"
                  :data-rarity="hit.record.rarity ?? ''"
                  :data-five-wall-group-key="group.key"
                  :data-five-wall-group-position="hitIndex + 1"
                  :data-five-wall-group-anchor="hitIndex === 0 ? 'true' : undefined"
                  :data-five-wall-group-record-ids="hitIndex === 0 ? group.recordIds : undefined"
                  :data-five-wall-group-distance="hitIndex === 0 ? group.displayDistance : undefined"
                  :data-five-wall-group-distance-mode="hitIndex === 0 ? group.distanceMode : undefined"
                  :data-five-wall-group-hit-count="hitIndex === 0 ? group.hits.length : undefined"
                  :title="`${app.formatQuantityName(hit.record.item_name, hit.record.count)} · ${app.formatTime(hit.record.time)} · ${group.displayDistance}`"
                  :aria-label="`${app.formatQuantityName(hit.record.item_name, hit.record.count)} ${group.displayDistance}`"
                >
                  <span class="five-wall-thumb" :class="{ empty: !app.hasRecordVisual(hit.record) }">
                    <img v-if="app.hasRecordVisual(hit.record)" :src="app.itemVisualUrl(hit.record)" :alt="app.formatQuantityName(hit.record.item_name, hit.record.count)" />
                    <span v-else>{{ hit.record.item_name.slice(0, 1) }}</span>
                  </span>
                  <span v-if="(hit.record.count ?? 1) > 1" class="five-wall-quantity" aria-hidden="true">x{{ hit.record.count }}</span>
                  <span
                    v-if="group.hits.length === 1"
                    class="five-wall-pity"
                    :class="app.fiveWallPityTone(group.displayDistance, hit.record.pool_kind)"
                    :data-five-wall-group-key="group.key"
                    data-five-wall-distance-placement="tile"
                  >{{ group.displayDistance }}</span>
                </div>
              </template>

              <template v-for="frame in fiveWallFrameLayouts" :key="`${frame.key}:badges`">
                <span
                  class="five-wall-group-count"
                  :style="fiveWallGroupCountStyle(frame)"
                  :data-five-wall-group-key="frame.key"
                  aria-hidden="true"
                >x{{ frame.hitCount }}</span>
                <span
                  v-if="!frame.crossesFirstRow || app.fiveWallExpanded"
                  class="five-wall-pity five-wall-group-distance"
                  :class="app.fiveWallPityTone(frame.displayDistance, app.selectedPoolKind)"
                  :style="fiveWallGroupDistanceStyle(frame)"
                  :data-five-wall-group-key="frame.key"
                  data-five-wall-distance-placement="terminal"
                  :title="`${frame.label} · ${frame.displayDistance}`"
                  :aria-label="`${frame.label} ${frame.displayDistance}`"
                >{{ frame.displayDistance }}</span>
                <span
                  v-else
                  class="five-wall-pity five-wall-group-distance"
                  :class="app.fiveWallPityTone(frame.displayDistance, app.selectedPoolKind)"
                  :style="fiveWallGroupDistanceStyle(frame, true)"
                  :data-five-wall-group-key="frame.key"
                  data-five-wall-distance-placement="proxy"
                  :title="`${frame.label} · ${frame.displayDistance}`"
                  :aria-label="`${frame.label} ${frame.displayDistance}`"
                >{{ frame.displayDistance }}</span>
              </template>
            </div>
            <div v-if="fiveWallOverflowsOneRow" class="five-wall-toolbar">
              <button
                type="button"
                class="ghost"
                data-agent-id="dashboard-five-wall-toggle"
                :title="app.fiveWallExpanded ? app.t('dashboard.collapseFiveWall') : app.t('dashboard.expandFiveWall')"
                :aria-expanded="app.fiveWallExpanded"
                @click="app.toggleFiveWallExpanded"
              >
                <component :is="app.fiveWallExpanded ? ChevronUp : ChevronDown" :size="16" />
                <span>{{ app.fiveWallExpanded ? app.t("dashboard.collapseFiveWall") : app.t("dashboard.expandFiveWall") }}</span>
              </button>
            </div>
          </div>
          <div v-if="app.detailLoading" class="empty-row">{{ app.t("common.loading") }}</div>
          <div v-else-if="!app.visibleLatestFiveStarHits.length" class="empty-row">{{ app.latestFiveStarEmptyText }}</div>
        </section>

        <section class="panel selected-detail-panel" data-agent-id="dashboard-selected-detail">
          <div class="panel-head">
            <div>
              <h2>{{ app.selectedDetailTitle }}</h2>
            </div>
            <button
              v-if="app.hasItemRankingRows"
              type="button"
              class="ranking-details ghost"
              @click="app.openRankingDialog"
            >
              <ListOrdered :size="16" />
              <span>{{ app.t("dashboard.itemRanking") }}</span>
            </button>
          </div>
          <div class="selected-detail-body">
            <div class="detail-analysis">
              <div class="detail-analysis-head">
                <span class="eyebrow">{{ app.t("dashboard.rarity") }}</span>
                <strong>{{ app.t("dashboard.distribution") }}</strong>
              </div>
              <div class="rarity-distribution">
                <div :ref="app.setChartEl" class="chart rarity-pie"></div>
                <div class="rarity-share-list">
                  <div
                    v-for="bucket in app.selectedRarityShares"
                    :key="bucket.key"
                    class="rarity-share-row"
                    :class="bucket.className"
                  >
                    <span class="rarity-dot" aria-hidden="true"></span>
                    <span>{{ bucket.label }}</span>
                    <strong>{{ bucket.count }} <span>[{{ bucket.percentText }}]</span></strong>
                  </div>
                </div>
              </div>
            </div>
            <div class="selected-summary-grid">
              <div class="summary-metric-grid">
                <div class="summary-metric-column summary-metric-column--pity">
                  <DashboardSummaryMetric
                    v-for="card in summaryMetricColumns.pity"
                    :key="card.key"
                    :class="card.className"
                    :icon="card.icon"
                    :title="card.title"
                    :value="card.value"
                    :tone="card.tone"
                    :progress-width="card.progressWidth"
                  />
                </div>
                <div class="summary-metric-column summary-metric-column--detail">
                  <DashboardSummaryMetric
                    v-for="card in summaryMetricColumns.detail"
                    :key="card.key"
                    :class="card.className"
                    :icon="card.icon"
                    :title="card.title"
                    :value="card.value"
                    :tone="card.tone"
                    :progress-width="card.progressWidth"
                  />
                </div>
              </div>
            </div>
          </div>
        </section>
        <div v-if="app.rankingDialogOpen" class="ranking-dialog-backdrop" @click.self="app.closeRankingDialog">
          <section class="ranking-dialog" role="dialog" aria-modal="true" :aria-label="app.rankingDialogTitle">
            <div class="ranking-dialog-head">
              <div>
                <h2>{{ app.rankingDialogTitle }}</h2>
              </div>
              <div class="ranking-rarity-toggle" role="group" :aria-label="app.t('dashboard.rankingRarityFilter')">
                <button
                  v-for="option in app.rankingRarityOptions"
                  :key="option.rarity"
                  type="button"
                  :class="[option.className, { active: option.active }]"
                  :aria-pressed="option.active"
                  @click="app.toggleRankingRarity(option.rarity)"
                >
                  {{ option.label }}
                </button>
              </div>
              <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.closeRankingDialog">
                <X :size="17" />
              </button>
            </div>
            <div class="ranking-share-list">
              <div v-if="!app.itemRankingShares.length" class="ranking-empty">{{ app.t("dashboard.noRankingRarityItems") }}</div>
              <div v-for="item in app.itemRankingShares" :key="`${item.item_id}:${item.reward_count}`" class="ranking-share-row" :class="app.recordRarityClass(item)">
                <span v-if="app.hasItemVisual(item)" class="ranking-item-thumb">
                  <img :src="app.itemVisualUrl(item)" alt="" />
                </span>
                <span v-else class="ranking-item-thumb empty">{{ item.item_name.slice(0, 1) }}</span>
                <span class="ranking-name">{{ app.formatQuantityName(item.item_name, item.reward_count) }}</span>
                <strong>{{ item.count }}</strong>
                <span>{{ app.percent(item.share) }}</span>
                <span class="ranking-share-bar" aria-hidden="true">
                  <span :style="{ width: item.shareWidth }"></span>
                </span>
              </div>
            </div>
          </section>
        </div>
      </section>
</template>
