<script setup lang="ts">
import { ChevronDown, ChevronUp, Crown, Eye, EyeOff, Gem, ListFilter, ListOrdered, Target, Ticket, TrendingDown, TrendingUp, X, Zap } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch, type Component } from "vue";
import { useAppContext } from "../app/context";
import DashboardSummaryMetric from "./DashboardSummaryMetric.vue";

const app = useAppContext();
const fiveWallGrid = ref<HTMLElement | null>(null);
const fiveWallOverflowsOneRow = ref(false);
const fiveWallRowHeight = ref(84);
const fiveWallFirstRowItemCount = ref(0);
let fiveWallResizeObserver: ResizeObserver | null = null;

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
    fiveWallOverflowsOneRow.value = false;
    return;
  }
  const items = [...grid.querySelectorAll<HTMLElement>(".five-wall-item")];
  if (!items.length) {
    fiveWallOverflowsOneRow.value = false;
    return;
  }
  const firstTop = items[0].offsetTop;
  fiveWallRowHeight.value = items[0].offsetHeight;
  fiveWallFirstRowItemCount.value = items.filter((item) => Math.abs(item.offsetTop - firstTop) <= 1).length;
  fiveWallOverflowsOneRow.value = items.some((item) => item.offsetTop > firstTop + 1);
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
  () => [app.displayedLatestFiveStarHits.length, app.selectedPoolKind, dashboardScopeKey.value, app.latestFiveStarWallMode],
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
            :class="{ active: app.isSelectedDashboardBanner(banner.banner_id) }"
            type="button"
            :title="banner.title"
            :aria-label="banner.title"
            :aria-pressed="app.isSelectedDashboardBanner(banner.banner_id)"
            @click="app.selectDashboardBanner(banner)"
          >
            <span v-if="app.hasBannerVisual(banner)" class="rail-thumb">
              <img :src="app.bannerVisualUrl(banner)" alt="" />
            </span>
            <span v-else class="rail-thumb empty">
              <span>{{ banner.title }}</span>
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
              <div
                v-for="(hit, index) in app.displayedLatestFiveStarHits"
                :key="hit.record.record_id"
                class="five-wall-item"
                :class="[app.recordRarityClass(hit.record), { 'is-after-first-row': fiveWallOverflowsOneRow && index >= fiveWallFirstRowItemCount }]"
                :data-record-id="hit.record.record_id"
                :data-source-order="hit.record.source_order"
                :data-time="hit.record.time ?? ''"
                :data-pool-kind="hit.record.pool_kind"
                :data-pool-id="hit.record.pool_id"
                :data-item-id="hit.record.item_id"
                :data-rarity="hit.record.rarity ?? ''"
                :data-five-wall-distance="app.fiveWallDistance(hit)"
                :title="`${app.formatQuantityName(hit.record.item_name, hit.record.count)} · ${app.formatTime(hit.record.time)}`"
                :aria-label="`${app.formatQuantityName(hit.record.item_name, hit.record.count)} ${app.fiveWallDistance(hit)}`"
              >
                <span v-if="app.hasRecordVisual(hit.record)" class="five-wall-thumb">
                  <img :src="app.itemVisualUrl(hit.record)" :alt="app.formatQuantityName(hit.record.item_name, hit.record.count)" />
                </span>
                <span v-else class="five-wall-thumb empty">{{ hit.record.item_name.slice(0, 1) }}</span>
                <span v-if="hit.record.count && hit.record.count > 1" class="five-wall-quantity" aria-hidden="true">x{{ hit.record.count }}</span>
                <span class="five-wall-pity" :class="app.fiveWallPityTone(app.fiveWallDistance(hit), hit.record.pool_kind)">{{ app.fiveWallDistance(hit) }}</span>
              </div>
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
