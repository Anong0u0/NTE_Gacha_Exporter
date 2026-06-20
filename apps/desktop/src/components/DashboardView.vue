<script setup lang="ts">
import { CircleStop, FileJson, RadioTower, RefreshCw, Upload } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack" data-agent-id="view-dashboard">
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
            <button
              type="button"
              :disabled="app.isWorkflowBusy"
              @click="app.startFullCapture"
            >
              <RefreshCw :size="17" />
              <span>{{ app.t("capture.fullUpdate") }}</span>
            </button>
            <button type="button" :disabled="!app.isCaptureActive || app.captureActionBusy" @click="app.stopLiveCapture">
              <CircleStop :size="17" />
              <span>{{ app.t("capture.stop") }}</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('raw')">
              <Upload :size="17" />
              <span>{{ app.t("import.rawJsonl") }}</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('public')">
              <FileJson :size="17" />
              <span>{{ app.t("import.publicJson") }}</span>
            </button>
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
            <strong>{{ app.summary?.resource.total_roll_points ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>{{ app.t("dashboard.selected5Pity") }}</span>
            <strong>{{ app.selectedBanner?.current_5star_pity ?? app.selectedSummary?.current_pity ?? 0 }}</strong>
          </div>
        </section>

        <section class="pool-strip">
          <button
            v-for="pool in app.allPoolSummaries"
            :key="pool.pool_kind"
            :class="{ active: app.selectedPoolKind === pool.pool_kind }"
            type="button"
            @click="app.selectedPoolKind = pool.pool_kind"
          >
            <span>
              <strong>{{ pool.label }}</strong>
              <small>{{ app.kindLabels[pool.pool_kind] }} · {{ app.t("dashboard.pulls", { count: pool.total_pulls }) }}</small>
            </span>
            <span class="pity">{{ pool.current_pity }}/{{ pool.hard_pity }}</span>
            <span class="state">{{ pool.current_guarantee ? app.t("dashboard.guaranteed") : app.t("dashboard.normal") }}</span>
            <span class="pool-latest">{{ app.t("dashboard.latest5") }} · {{ pool.latest_5star?.item_name ?? "-" }}</span>
          </button>
        </section>

        <section class="banner-grid">
          <button
            v-for="banner in app.bannerSummaries"
            :key="banner.banner_id"
            class="banner-card"
            :class="{ active: app.selectedBanner?.banner_id === banner.banner_id }"
            type="button"
            @click="
              app.selectedBannerId = banner.banner_id;
              app.selectedPoolKind = banner.pool_kind;
            "
          >
            <span v-if="app.hasBannerVisual(banner)" class="banner-visual">
              <img :src="app.bannerVisualUrl(banner)" alt="" />
            </span>
            <span class="banner-card-head">
              <span>
                <strong>{{ banner.title }}</strong>
                <small>{{ app.kindLabels[banner.pool_kind] }} · {{ banner.banner_type ?? app.t("common.banner") }}</small>
              </span>
              <span class="confidence-badge">{{ banner.source_confidence ?? app.t("common.unknown").toLowerCase() }}</span>
            </span>
            <span class="banner-window">{{ app.formatBannerWindow(banner.start_at, banner.end_at) }}</span>
            <span class="banner-stats">
              <span>{{ app.t("dashboard.pulls", { count: banner.total_pulls }) }}</span>
              <span>{{ app.t("dashboard.roll", { count: banner.roll_points_total }) }}</span>
              <span>5★ {{ banner.current_5star_pity }}</span>
              <span>4★ {{ banner.current_4star_pity }}</span>
            </span>
            <span class="banner-hit-line">
              5★ {{ banner.five_star_count }} · 4★ {{ banner.four_star_count }} · UP {{ banner.rate_up_5_count }}/{{ banner.off_rate_5_count }}
            </span>
          </button>
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
            </div>
            <div class="record-table detail-table">
              <div class="record-header five-star-header">
                <span>{{ app.t("common.time") }}</span>
                <span>{{ app.t("common.item") }}</span>
                <span>{{ app.t("dashboard.pool") }}</span>
                <span>{{ app.t("records.pity") }}</span>
                <span>{{ app.t("common.result") }}</span>
                <span>{{ app.t("dashboard.guarantee") }}</span>
              </div>
              <div v-for="hit in app.detail?.five_star_history ?? []" :key="hit.record.record_id" class="record-line five-star-line">
                <span>{{ app.formatTime(hit.record.time) }}</span>
                <span>{{ hit.record.item_name }}</span>
                <span>{{ hit.record.pool_label }}</span>
                <span>{{ hit.pity_distance }}</span>
                <span>{{ app.formatResult(hit.result) }}</span>
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
              <div v-for="item in app.summary?.item_ranking ?? []" :key="item.item_id">
                <span>{{ item.item_name }}</span>
                <strong>{{ item.count }}</strong>
              </div>
            </div>
          </div>
        </section>

        <section class="split">
          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ app.selectedBanner ? app.bannerMeta(app.selectedBanner) : app.t("common.banner") }}</span>
                <h2>{{ app.t("dashboard.selectedBanner") }}</h2>
              </div>
            </div>
            <div v-if="app.hasSelectedBannerVisuals()" class="selected-banner-visual">
              <div v-if="app.bannerVisualUrl(app.selectedBanner)" class="selected-banner-hero">
                <img :src="app.bannerVisualUrl(app.selectedBanner)" alt="" />
              </div>
              <div v-if="app.selectedBannerPortraitUrls().length" class="portrait-strip">
                <span v-for="url in app.selectedBannerPortraitUrls()" :key="url" class="portrait-thumb">
                  <img :src="url" alt="" />
                </span>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>{{ app.t("common.title") }}</span><strong class="stat-text">{{ app.selectedBanner?.title ?? "-" }}</strong></div>
              <div><span>{{ app.t("dashboard.totalPulls") }}</span><strong>{{ app.selectedBanner?.total_pulls ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.rollPoints") }}</span><strong>{{ app.selectedBanner?.roll_points_total ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.avg5Pity") }}</span><strong>{{ app.numberOrDash(app.selectedBanner?.average_5star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.avg4Pity") }}</span><strong>{{ app.numberOrDash(app.selectedBanner?.average_4star_pity) }}</strong></div>
              <div><span>{{ app.t("dashboard.latestHit") }}</span><strong class="stat-text">{{ app.selectedBanner?.latest_hit?.item_name ?? "-" }}</strong></div>
            </div>
            <div class="derived-chip-row">
              <span class="derived-chip">5★ UP {{ app.selectedBanner?.rate_up_5_count ?? 0 }}</span>
              <span class="derived-chip">5★ {{ app.t("format.offRate") }} {{ app.selectedBanner?.off_rate_5_count ?? 0 }}</span>
              <span class="derived-chip">4★ UP {{ app.selectedBanner?.rate_up_4_count ?? 0 }}</span>
              <span class="derived-chip">{{ app.t("dashboard.missingRoll", { count: app.selectedBanner?.missing_roll_point_records ?? 0 }) }}</span>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">{{ app.t("dashboard.resource") }}</span>
                <h2>{{ app.t("dashboard.rollPoints") }}</h2>
              </div>
            </div>
            <div class="resource-grid">
              <div><span>{{ app.t("dashboard.total") }}</span><strong>{{ app.summary?.resource.total_roll_points ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.knownRecords") }}</span><strong>{{ app.summary?.resource.known_roll_point_records ?? 0 }}</strong></div>
              <div><span>{{ app.t("dashboard.missingRecords") }}</span><strong>{{ app.summary?.resource.missing_roll_point_records ?? 0 }}</strong></div>
            </div>
            <div class="timeline-list compact-list">
              <div v-for="resource in app.summary?.resource.by_pool_kind ?? []" :key="resource.pool_kind">
                <span>{{ resource.label }}</span>
                <strong>{{ resource.roll_points_total }}</strong>
              </div>
            </div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("dashboard.phase") }}</span>
              <h2>{{ app.t("dashboard.bannerTimeline") }}</h2>
            </div>
          </div>
          <div class="timeline-list">
            <div v-for="phase in app.phaseSummaries" :key="`${phase.version ?? 'v'}-${phase.phase ?? 'p'}`">
              <span>{{ phase.version ?? app.t("common.unknown").toLowerCase() }} · {{ phase.phase ?? app.t("dashboard.phase").toLowerCase() }}</span>
              <small>{{ phase.banner_count }} {{ app.t("common.banner").toLowerCase() }} · {{ app.t("dashboard.pulls", { count: phase.total_pulls }) }} · 5★ {{ phase.five_star_count }} · 4★ {{ phase.four_star_count }}</small>
              <strong>{{ phase.roll_points_total }}</strong>
            </div>
            <div v-if="app.phaseSummaries.length === 0" class="empty-row">{{ app.t("dashboard.phaseStatsEmpty") }}</div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("dashboard.latest") }}</span>
              <h2>{{ app.t("dashboard.recentRecords") }}</h2>
            </div>
          </div>
          <div class="record-list compact">
            <div v-for="record in app.latest" :key="record.record_id" class="record-row">
              <span v-if="app.hasRecordVisual(record)" class="item-thumb">
                <img :src="app.itemVisualUrl(record)" alt="" />
              </span>
              <div>
                <strong>{{ record.item_name }}</strong>
                <span>{{ app.bannerTitle(record.banner) }} · {{ record.rarity ? `${record.rarity}★` : app.t("common.unknown").toLowerCase() }} · {{ app.t("dashboard.pull", { value: app.formatPullNo(record) }) }}</span>
                <span class="derived-chip">{{ app.formatResult(record.derived.rate_up_result) }} · {{ app.formatPity(record) }}</span>
              </div>
              <small>{{ app.formatTime(record.time) }}</small>
            </div>
          </div>
        </section>
      </section>
</template>
