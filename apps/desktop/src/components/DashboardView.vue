<script setup lang="ts">
import { CircleStop, RadioTower, RefreshCw } from "lucide-vue-next";
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

        <section class="metrics-grid" data-agent-id="dashboard-metrics">
          <div class="metric">
            <span>{{ app.t("dashboard.totalPulls") }}</span>
            <strong>{{ app.summary?.total_records ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>{{ app.t("dashboard.trackedBanners") }}</span>
            <strong>{{ app.trackedBannerCount }}</strong>
          </div>
          <div class="metric">
            <span>{{ app.t("dashboard.totalRollPoints") }}</span>
            <strong>{{ app.totalRollPoints }}</strong>
          </div>
          <div class="metric">
            <span>{{ app.t("dashboard.selected5Pity") }}</span>
            <strong>{{ app.selectedSummary?.current_pity ?? 0 }}</strong>
          </div>
        </section>

        <section class="pool-strip">
          <button
            v-for="pool in app.allPoolSummaries"
            :key="pool.pool_kind"
            :class="{ active: app.isSelectedDashboardPool(pool.pool_kind) }"
            type="button"
            @click="app.selectDashboardPool(pool.pool_kind)"
          >
            <span>
              <strong>{{ pool.label }}</strong>
              <small>{{ app.kindLabels[pool.pool_kind] }} · {{ app.t("dashboard.pulls", { count: pool.total_pulls }) }}</small>
            </span>
            <span class="pity">{{ pool.current_pity }}/{{ pool.hard_pity }}</span>
            <span v-if="pool.pool_kind === 'fork_lottery' && pool.current_guarantee" class="state">{{ app.t("dashboard.guaranteed") }}</span>
            <span class="pool-latest">{{ app.t("dashboard.latest5") }} · {{ pool.latest_5star?.item_name ?? "-" }}</span>
          </button>
        </section>

        <section class="banner-thumb-rail" data-agent-id="dashboard-banner-rail">
          <button
            v-for="banner in app.bannerSummaries"
            :key="banner.banner_id"
            :class="{ active: app.isSelectedDashboardBanner(banner.banner_id) }"
            type="button"
            :title="banner.title"
            @click="app.selectDashboardBanner(banner)"
          >
            <span v-if="app.hasBannerVisual(banner)" class="rail-thumb">
              <img :src="app.bannerVisualUrl(banner)" alt="" />
            </span>
            <span v-else class="rail-thumb empty">{{ banner.title.slice(0, 1) }}</span>
            <span>{{ banner.title }}</span>
          </button>
        </section>

        <section class="banner-grid">
          <article
            v-for="banner in app.selectedPoolBannerSummaries"
            :key="banner.banner_id"
            class="banner-card"
            :class="{ active: app.isSelectedDashboardBanner(banner.banner_id) }"
            role="button"
            tabindex="0"
            @click="app.selectDashboardBanner(banner)"
            @keydown.enter.prevent="app.selectDashboardBanner(banner)"
            @keydown.space.prevent="app.selectDashboardBanner(banner)"
          >
            <span v-if="app.hasBannerVisual(banner)" class="banner-visual">
              <img :src="app.bannerVisualUrl(banner)" alt="" />
            </span>
            <span class="banner-card-head">
              <span>
                <strong>{{ banner.title }}</strong>
              </span>
            </span>
            <span class="banner-window">{{ app.formatBannerWindow(banner.start_at, banner.end_at) }}</span>
            <span class="banner-stats">
              <span>{{ app.t("dashboard.pulls", { count: banner.total_pulls }) }}</span>
              <span>{{ app.t("dashboard.roll", { count: banner.roll_points_total }) }}</span>
              <span>5★ {{ banner.current_5star_pity }}</span>
            </span>
            <span class="banner-hit-line">
              5★ {{ banner.five_star_count }} · 4★ {{ banner.four_star_count }} · UP {{ banner.rate_up_5_count }}/{{ banner.off_rate_5_count }}
            </span>
            <span v-if="banner.pool_kind === 'fork_lottery'" class="banner-hit-line">
              W {{ banner.fork_win_count }} · L {{ banner.fork_loss_count }} · G {{ banner.fork_forced_up_count }} · 25/75 {{ app.forkWinRate(banner) }}
            </span>
            <span class="banner-stats banner-extra-stats">
              <span>{{ app.t("dashboard.avg5Pity") }} {{ app.numberOrDash(banner.average_5star_pity) }}</span>
              <span>{{ app.t("dashboard.latestHit") }} · {{ banner.latest_hit?.item_name ?? "-" }}</span>
              <span>4★ UP {{ banner.rate_up_4_count }}</span>
              <span>{{ app.t("dashboard.missingRoll", { count: banner.missing_roll_point_records }) }}</span>
            </span>
          </article>
        </section>

        <section class="panel latest-five-wall" data-agent-id="dashboard-latest-five-wall">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.selectedSummary?.label ?? app.t("dashboard.pool") }}</span>
              <h2>{{ app.t("dashboard.latest5") }}</h2>
            </div>
          </div>
          <div class="five-wall-grid">
            <button
              v-for="hit in (app.detail?.five_star_history ?? []).slice(-12).reverse()"
              :key="hit.record.record_id"
              type="button"
              class="five-wall-item"
              @click="app.selectDashboardBannerById(hit.record.derived.banner_id)"
            >
              <span v-if="app.hasRecordVisual(hit.record)" class="five-wall-thumb">
                <img :src="app.itemVisualUrl(hit.record)" alt="" />
              </span>
              <span v-else class="five-wall-thumb empty">{{ hit.record.item_name.slice(0, 1) }}</span>
              <span class="five-wall-meta">
                <strong>{{ hit.record.item_name }}</strong>
                <small>{{ hit.pity_distance }} · {{ app.formatTime(hit.record.time) }}</small>
              </span>
              <span v-if="app.forkHitBadge(hit.record)" class="hit-badge" :class="`hit-${app.forkHitBadge(hit.record).toLowerCase()}`">{{ app.forkHitBadge(hit.record) }}</span>
            </button>
          </div>
          <div v-if="!app.detail?.five_star_history.length" class="empty-row">{{ app.t("dashboard.fiveStarRecordsEmpty") }}</div>
        </section>

        <section class="split wide-left">
          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ app.selectedSummary?.label ?? app.t("dashboard.pool") }}</span>
                <h2>{{ app.t("dashboard.poolDetail") }}</h2>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>{{ app.t("dashboard.totalPulls") }}</span><strong>{{ app.selectedSummary?.total_pulls ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.fiveHits") }}</span><strong>{{ app.selectedSummary?.hit_count ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.averagePity") }}</span><strong>{{ app.numberOrDash(app.selectedSummary?.average_5star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.shortest") }}</span><strong>{{ app.numberOrDash(app.selectedSummary?.min_5star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.longest") }}</span><strong>{{ app.numberOrDash(app.selectedSummary?.max_5star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.upRate") }}</span><strong>{{ app.percent(app.selectedSummary?.observed_up_rate) }}</strong></div>
              <div v-if="app.selectedSummary?.pool_kind === 'fork_lottery'"><span>25/75</span><strong>{{ app.forkWinRate(app.selectedSummary) }}</strong></div>
              <div v-if="app.selectedSummary?.pool_kind === 'fork_lottery'"><span>W/L/G</span><strong>{{ app.selectedSummary.fork_win_count }}/{{ app.selectedSummary.fork_loss_count }}/{{ app.selectedSummary.fork_forced_up_count }}</strong></div>
            </div>
            <div class="record-table detail-table">
              <div class="record-header five-star-header">
                <span>{{ app.t("common.time") }}</span>
                <span>{{ app.t("common.item") }}</span>
                <span>{{ app.t("dashboard.pool") }}</span>
                <span>{{ app.t("records.fiveStarProgress") }}</span>
                <span>{{ app.t("common.result") }}</span>
                <span>{{ app.t("dashboard.guarantee") }}</span>
              </div>
              <div v-for="hit in app.detail?.five_star_history ?? []" :key="hit.record.record_id" class="record-line five-star-line">
                <span>{{ app.formatTime(hit.record.time) }}</span>
                <span>{{ hit.record.item_name }}</span>
                <span>{{ hit.record.pool_label }}</span>
                <span>{{ hit.pity_distance }}</span>
                <span>
                  <span>{{ app.formatResult(hit.result) }}</span>
                  <span v-if="app.forkHitBadge(hit.record)" class="hit-badge" :class="`hit-${app.forkHitBadge(hit.record).toLowerCase()}`">{{ app.forkHitBadge(hit.record) }}</span>
                </span>
                <span>{{ hit.guarantee_before ? app.t("format.before") : "-" }} / {{ hit.guarantee_after ? app.t("format.after") : "-" }}</span>
              </div>
              <div v-if="!app.detail?.five_star_history.length" class="empty-row">{{ app.t("dashboard.fiveStarRecordsEmpty") }}</div>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ app.t("dashboard.rarity") }}</span>
                <h2>{{ app.t("dashboard.distribution") }}</h2>
              </div>
            </div>
            <div :ref="app.setChartEl" class="chart"></div>
            <div class="rank-list">
              <div v-for="item in app.detail?.item_ranking ?? []" :key="item.item_id">
                <span>{{ item.item_name }}</span>
                <strong>{{ item.count }}</strong>
              </div>
            </div>
          </div>
        </section>
      </section>
</template>
