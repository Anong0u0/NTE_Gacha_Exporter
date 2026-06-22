<script setup lang="ts">
import { CircleStop, ListFilter, ListOrdered, RadioTower, RefreshCw, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack dashboard-workbench" data-agent-id="view-dashboard">
        <section class="update-band">
          <div>
            <span class="eyebrow">{{ app.t("import.updateData") }}</span>
            <h2>{{ app.captureTitle }}</h2>
            <p>{{ app.captureSubtitle }}</p>
            <div v-if="app.captureStatus" class="capture-summary">
              <div class="capture-stats">
                <span>{{ app.captureModeLabel }}</span>
                <span>{{ app.formatCaptureState(app.captureStatus.state) }}</span>
                <span>{{ app.t("capture.packets", { count: app.captureStatus.counters.packets_seen }) }}</span>
                <span>{{ app.t("capture.decoded", { count: app.captureStatus.counters.decoded_packets }) }}</span>
                <span>{{ app.t("capture.dropped", { count: app.captureStatus.counters.dropped_packets }) }}</span>
                <span v-if="app.captureStatus.counters.duplicate_packets">{{ app.t("capture.duplicates", { count: app.captureStatus.counters.duplicate_packets }) }}</span>
              </div>
              <div v-if="app.autoPageStatusLine" class="capture-target">{{ app.autoPageStatusLine }}</div>
              <div v-if="app.captureStatus.auto_page" class="capture-stats">
                <span>{{ app.t("capture.poolsDone", { count: app.captureStatus.auto_page.completed_pools?.length ?? 0 }) }}</span>
                <span>{{ app.t("capture.poolsSkipped", { count: app.captureStatus.auto_page.skipped_pools?.length ?? 0 }) }}</span>
              </div>
              <div v-if="app.captureStatus.raw_path" class="capture-target">{{ app.captureStatus.raw_path }}</div>
              <div v-if="app.captureStatus.target" class="capture-target">
                {{ app.captureStatus.target.pid ?? "-" }} · {{ app.captureStatus.target.interface ?? "-" }}
              </div>
              <div v-if="app.captureStatus.latest_records.length" class="capture-latest">
                <div v-for="record in app.captureStatus.latest_records.slice(-3)" :key="String(record.record_id ?? record.item_id ?? app.captureRecordName(record))">
                  <span>{{ app.captureRecordName(record) }}</span>
                  <small>{{ app.captureRecordMeta(record) }}</small>
                </div>
              </div>
            </div>
          </div>
          <div class="capture-command">
            <div class="action-row">
              <div class="segmented mode-toggle">
                <button
                  type="button"
                  :class="{ active: app.captureMode === 'auto_page_incremental' }"
                  :disabled="app.isWorkflowBusy"
                  @click="app.captureMode = 'auto_page_incremental'"
                >
                  {{ app.t("capture.autoPage") }}
                </button>
                <button
                  type="button"
                  :class="{ active: app.captureMode === 'live_only' }"
                  :disabled="app.isWorkflowBusy"
                  @click="app.captureMode = 'live_only'"
                >
                  {{ app.t("capture.liveOnly") }}
                </button>
              </div>
              <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.startLiveCapture()">
                <RadioTower :size="17" />
                <span>{{ app.t("import.updateData") }}</span>
              </button>
              <button type="button" :disabled="app.isWorkflowBusy" @click="app.startFullCapture">
                <RefreshCw :size="17" />
                <span>{{ app.t("capture.fullUpdate") }}</span>
              </button>
              <button type="button" :disabled="!app.isCaptureActive || app.captureActionBusy" @click="app.stopLiveCapture">
                <CircleStop :size="17" />
                <span>{{ app.t("capture.stop") }}</span>
              </button>
            </div>
          </div>
        </section>

        <section class="pool-strip">
          <button
            v-for="pool in app.allPoolSummaries"
            :key="pool.pool_kind"
            :class="{ active: app.isSelectedDashboardPool(pool.pool_kind) }"
            type="button"
            :aria-pressed="app.isSelectedDashboardPool(pool.pool_kind)"
            @click="app.selectDashboardPool(pool.pool_kind)"
          >
            <span>
              <strong>{{ pool.label }}</strong>
              <small>{{ app.kindLabels[pool.pool_kind] }} · {{ app.t("dashboard.pulls", { count: pool.total_pulls }) }}</small>
            </span>
            <span class="pity">{{ pool.current_pity }}/{{ pool.hard_pity }}</span>
            <span class="pool-latest">{{ app.t("dashboard.latest5") }} · {{ pool.latest_5star?.item_name ?? "-" }}</span>
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
            <button
              type="button"
              class="ghost"
              :disabled="!app.detail?.five_star_history.length || app.isWorkflowBusy"
              @click="app.showDashboardFiveStarRecords"
            >
              <ListFilter :size="16" />
              <span>{{ app.t("dashboard.viewDetailedRecords") }}</span>
            </button>
          </div>
          <div class="five-wall-grid">
            <div
              v-for="hit in (app.detail?.five_star_history ?? []).slice(-12).reverse()"
              :key="hit.record.record_id"
              class="five-wall-item"
              :title="`${hit.record.item_name} · ${hit.pity_distance} · ${app.formatTime(hit.record.time)}`"
              :aria-label="`${hit.record.item_name} ${hit.pity_distance}`"
            >
              <span v-if="app.hasRecordVisual(hit.record)" class="five-wall-thumb">
                <img :src="app.itemVisualUrl(hit.record)" :alt="hit.record.item_name" />
              </span>
              <span v-else class="five-wall-thumb empty">{{ hit.record.item_name.slice(0, 1) }}</span>
              <span class="five-wall-pity" :class="app.fiveWallPityTone(hit.pity_distance, hit.record.pool_kind)">{{ hit.pity_distance }}</span>
            </div>
          </div>
          <div v-if="app.detailLoading" class="empty-row">{{ app.t("common.loading") }}</div>
          <div v-else-if="!app.detail?.five_star_history.length" class="empty-row">{{ app.t("dashboard.fiveStarRecordsEmpty") }}</div>
        </section>

        <section class="panel selected-detail-panel" data-agent-id="dashboard-selected-detail">
          <div class="panel-head">
            <div>
              <h2>{{ app.selectedDetailTitle }}</h2>
            </div>
            <button
              v-if="app.itemRankingShares.length"
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
            <div class="stat-table selected-summary-grid">
              <div><span>{{ app.t("dashboard.selected5Pity") }}</span><strong>{{ app.selectedSummary?.current_pity ?? 0 }}/{{ app.selectedSummary?.hard_pity ?? 0 }}</strong></div>
              <div><span>{{ app.summaryProgressLabel(app.selectedSummary) }}</span><strong>{{ app.formatTenPullProgressSummary(app.selectedSummary?.current_ten_pull_progress) }}</strong></div>
              <div><span>{{ app.t("dashboard.totalPulls") }}</span><strong>{{ app.selectedSummary?.total_pulls ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.fiveUpHits") }}</span><strong>{{ app.fiveUpHitRatio(app.selectedSummary) }}</strong></div>
              <div><span>{{ app.t("dashboard.fourHits") }}</span><strong>{{ app.selectedSummary?.four_star_count ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.avg5Pity") }}</span><strong>{{ app.numberOrDash(app.selectedSummary?.average_5star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.avg4Pity") }}</span><strong>{{ app.numberOrDash(app.selectedSummary?.average_4star_pity) }}</strong></div>
              <div v-if="app.selectedSummary?.pool_kind === 'fork_lottery'"><span>{{ app.t("dashboard.forkWinRate") }}</span><strong>{{ app.forkWinRate(app.selectedSummary) }}</strong></div>
            </div>
          </div>
        </section>
        <div v-if="app.rankingDialogOpen" class="ranking-dialog-backdrop" @click.self="app.closeRankingDialog">
          <section class="ranking-dialog" role="dialog" aria-modal="true" :aria-label="app.rankingDialogTitle">
            <div class="ranking-dialog-head">
              <div>
                <span class="eyebrow">{{ app.selectedScopeLabel ?? app.t("dashboard.pool") }}</span>
                <h2>{{ app.rankingDialogTitle }}</h2>
              </div>
              <button type="button" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="app.closeRankingDialog">
                <X :size="17" />
              </button>
            </div>
            <div class="ranking-share-list">
              <div v-for="item in app.itemRankingShares" :key="item.item_id" class="ranking-share-row">
                <span v-if="app.hasItemVisual(item)" class="ranking-item-thumb">
                  <img :src="app.itemVisualUrl(item)" alt="" />
                </span>
                <span v-else class="ranking-item-thumb empty">{{ item.item_name.slice(0, 1) }}</span>
                <span class="ranking-name">{{ item.item_name }}</span>
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
