<script setup lang="ts">
import { CircleStop, FileJson, Image, RadioTower, RefreshCw, Upload } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack">
        <section class="update-band">
          <div>
            <span class="eyebrow">Update Data</span>
            <h2>{{ app.captureTitle }}</h2>
            <p>{{ app.captureSubtitle }}</p>
            <div v-if="app.captureStatus" class="capture-app.summary">
              <div class="capture-stats">
                <span>{{ app.captureModeLabel }}</span>
                <span>{{ app.formatCaptureState(app.captureStatus.state) }}</span>
                <span>{{ app.captureStatus.counters.packets_seen }} packets</span>
                <span>{{ app.captureStatus.counters.decoded_packets }} decoded</span>
                <span>{{ app.captureStatus.counters.dropped_packets }} dropped</span>
                <span v-if="app.captureStatus.counters.duplicate_packets">{{ app.captureStatus.counters.duplicate_packets }} duplicates</span>
              </div>
              <div v-if="app.autoPageStatusLine" class="capture-target">{{ app.autoPageStatusLine }}</div>
              <div v-if="app.captureStatus.auto_page" class="capture-stats">
                <span>{{ app.captureStatus.auto_page.completed_pools?.length ?? 0 }} pools done</span>
                <span>{{ app.captureStatus.auto_page.skipped_pools?.length ?? 0 }} pools skipped</span>
              </div>
              <div v-if="app.captureStatus.raw_path" class="capture-target">{{ app.captureStatus.raw_path }}</div>
              <div v-if="app.captureStatus.target" class="capture-target">
                {{ app.captureStatus.target.pid ?? "-" }} · {{ app.captureStatus.target.interface ?? "-" }}
              </div>
              <div v-if="app.captureStatus.latest_records.length" class="capture-app.latest">
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
                Auto-page
              </button>
              <button
                type="button"
                :class="{ active: app.captureMode === 'live_only' }"
                :disabled="app.isWorkflowBusy"
                @click="app.captureMode = 'live_only'"
              >
                Live only
              </button>
            </div>
            <button class="primary" type="button" :disabled="app.isWorkflowBusy" @click="app.startLiveCapture()">
              <RadioTower :size="17" />
              <span>Update Data</span>
            </button>
            <button
              type="button"
              :disabled="app.isWorkflowBusy"
              @click="app.startFullCapture"
            >
              <RefreshCw :size="17" />
              <span>Full update</span>
            </button>
            <button type="button" :disabled="!app.isCaptureActive || app.captureActionBusy" @click="app.stopLiveCapture">
              <CircleStop :size="17" />
              <span>Stop</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('raw')">
              <Upload :size="17" />
              <span>Raw JSONL</span>
            </button>
            <button type="button" :disabled="app.isWorkflowBusy" @click="app.pickImportFile('public')">
              <FileJson :size="17" />
              <span>Public JSON</span>
            </button>
          </div>
        </section>

        <section class="metrics-grid">
          <div class="metric">
            <span>Total pulls</span>
            <strong>{{ app.summary?.total_records ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>Tracked banners</span>
            <strong>{{ app.trackedBannerCount }}</strong>
          </div>
          <div class="metric">
            <span>Total roll points</span>
            <strong>{{ app.summary?.resource.total_roll_points ?? 0 }}</strong>
          </div>
          <div class="metric">
            <span>Selected 5★ pity</span>
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
              <small>{{ app.kindLabels[pool.pool_kind] }} · {{ pool.total_pulls }} pulls</small>
            </span>
            <span class="pity">{{ pool.current_pity }}/{{ pool.hard_pity }}</span>
            <span class="state">{{ pool.current_guarantee ? "Guaranteed" : "Normal" }}</span>
            <span class="pool-app.latest">Latest 5★ · {{ pool.latest_5star?.item_name ?? "-" }}</span>
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
            <span class="banner-visual">
              <img v-if="app.bannerVisualUrl(banner)" :src="app.bannerVisualUrl(banner)" alt="" />
              <span v-else class="asset-placeholder"><Image :size="20" /></span>
            </span>
            <span class="banner-card-head">
              <span>
                <strong>{{ banner.title }}</strong>
                <small>{{ app.kindLabels[banner.pool_kind] }} · {{ banner.banner_type ?? "banner" }}</small>
              </span>
              <span class="confidence-badge">{{ banner.source_confidence ?? "unknown" }}</span>
            </span>
            <span class="banner-window">{{ app.formatBannerWindow(banner.start_at, banner.end_at) }}</span>
            <span class="banner-stats">
              <span>{{ banner.total_pulls }} pulls</span>
              <span>{{ banner.roll_points_total }} roll</span>
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
                <span class="eyebrow">{{ app.selectedSummary?.label ?? "Pool" }}</span>
                <h2>Pool app.detail</h2>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>Total pulls</span><strong>{{ app.selectedSummary?.total_pulls ?? 0 }}</strong></div>
              <div><span>5★ hits</span><strong>{{ app.selectedSummary?.hit_count ?? 0 }}</strong></div>
              <div><span>Average pity</span><strong>{{ app.numberOrDash(app.selectedSummary?.average_5star_pity) }}</strong></div>
              <div><span>Shortest</span><strong>{{ app.numberOrDash(app.selectedSummary?.min_5star_pity) }}</strong></div>
              <div><span>Longest</span><strong>{{ app.numberOrDash(app.selectedSummary?.max_5star_pity) }}</strong></div>
              <div><span>UP rate</span><strong>{{ app.percent(app.selectedSummary?.observed_up_rate) }}</strong></div>
            </div>
            <div class="record-table app.detail-table">
              <div class="record-header five-star-header">
                <span>Time</span>
                <span>Item</span>
                <span>Pool</span>
                <span>Pity</span>
                <span>Result</span>
                <span>Guarantee</span>
              </div>
              <div v-for="hit in app.detail?.five_star_history ?? []" :key="hit.record.record_id" class="record-line five-star-line">
                <span>{{ app.formatTime(hit.record.time) }}</span>
                <span>{{ hit.record.item_name }}</span>
                <span>{{ hit.record.pool_label }}</span>
                <span>{{ hit.pity_distance }}</span>
                <span>{{ app.formatResult(hit.result) }}</span>
                <span>{{ hit.guarantee_before ? "Before" : "-" }} / {{ hit.guarantee_after ? "After" : "-" }}</span>
              </div>
              <div v-if="!app.detail?.five_star_history.length" class="empty-row">No 5★ app.records in this pool.</div>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">Rarity</span>
                <h2>Known distribution</h2>
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
                <span class="eyebrow">{{ app.selectedBanner ? app.bannerMeta(app.selectedBanner) : "Banner" }}</span>
                <h2>Selected banner</h2>
              </div>
            </div>
            <div class="selected-banner-visual">
              <div class="selected-banner-hero">
                <img v-if="app.bannerVisualUrl(app.selectedBanner)" :src="app.bannerVisualUrl(app.selectedBanner)" alt="" />
                <span v-else class="asset-placeholder"><Image :size="24" /></span>
              </div>
              <div class="portrait-strip">
                <span v-for="url in app.selectedBannerPortraitUrls()" :key="url" class="portrait-thumb">
                  <img :src="url" alt="" />
                </span>
                <span v-if="app.selectedBannerPortraitUrls().length === 0" class="portrait-thumb placeholder">
                  <Image :size="18" />
                </span>
              </div>
            </div>
            <div class="stat-table compact">
              <div><span>Title</span><strong class="stat-text">{{ app.selectedBanner?.title ?? "-" }}</strong></div>
              <div><span>Pulls</span><strong>{{ app.selectedBanner?.total_pulls ?? 0 }}</strong></div>
              <div><span>Roll points</span><strong>{{ app.selectedBanner?.roll_points_total ?? 0 }}</strong></div>
              <div><span>Avg 5★ pity</span><strong>{{ app.numberOrDash(app.selectedBanner?.average_5star_pity) }}</strong></div>
              <div><span>Avg 4★ pity</span><strong>{{ app.numberOrDash(app.selectedBanner?.average_4star_pity) }}</strong></div>
              <div><span>Latest hit</span><strong class="stat-text">{{ app.selectedBanner?.latest_hit?.item_name ?? "-" }}</strong></div>
            </div>
            <div class="derived-chip-row">
              <span class="derived-chip">5★ UP {{ app.selectedBanner?.rate_up_5_count ?? 0 }}</span>
              <span class="derived-chip">5★ off {{ app.selectedBanner?.off_rate_5_count ?? 0 }}</span>
              <span class="derived-chip">4★ UP {{ app.selectedBanner?.rate_up_4_count ?? 0 }}</span>
              <span class="derived-chip">missing roll {{ app.selectedBanner?.missing_roll_point_records ?? 0 }}</span>
            </div>
          </div>

          <div class="panel">
            <div class="panel-head">
              <div>
                <span class="eyebrow">Resource</span>
                <h2>Roll points</h2>
              </div>
            </div>
            <div class="resource-grid">
              <div><span>Total</span><strong>{{ app.summary?.resource.total_roll_points ?? 0 }}</strong></div>
              <div><span>Known app.records</span><strong>{{ app.summary?.resource.known_roll_point_records ?? 0 }}</strong></div>
              <div><span>Missing app.records</span><strong>{{ app.summary?.resource.missing_roll_point_records ?? 0 }}</strong></div>
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
              <span class="eyebrow">Phase</span>
              <h2>Banner timeline</h2>
            </div>
          </div>
          <div class="timeline-list">
            <div v-for="phase in app.phaseSummaries" :key="`${phase.version ?? 'v'}-${phase.phase ?? 'p'}`">
              <span>{{ phase.version ?? "unknown" }} · {{ phase.phase ?? "phase" }}</span>
              <small>{{ phase.banner_count }} banners · {{ phase.total_pulls }} pulls · 5★ {{ phase.five_star_count }} · 4★ {{ phase.four_star_count }}</small>
              <strong>{{ phase.roll_points_total }}</strong>
            </div>
            <div v-if="app.phaseSummaries.length === 0" class="empty-row">No phase stats.</div>
          </div>
        </section>

        <section class="panel">
          <div class="panel-head">
            <div>
              <span class="eyebrow">Latest</span>
              <h2>Recent app.records</h2>
            </div>
          </div>
          <div class="record-list compact">
            <div v-for="record in app.latest" :key="record.record_id" class="record-row">
              <span class="item-thumb">
                <img v-if="app.itemVisualUrl(record)" :src="app.itemVisualUrl(record)" alt="" />
                <span v-else class="asset-placeholder"><Image :size="17" /></span>
              </span>
              <div>
                <strong>{{ record.item_name }}</strong>
                <span>{{ app.bannerTitle(record.banner) }} · {{ record.rarity ? `${record.rarity}★` : "unknown" }} · pull {{ app.formatPullNo(record) }}</span>
                <span class="derived-chip">{{ app.formatResult(record.derived.rate_up_result) }} · {{ app.formatPity(record) }}</span>
              </div>
              <small>{{ app.formatTime(record.time) }}</small>
            </div>
          </div>
        </section>
      </section>
</template>
