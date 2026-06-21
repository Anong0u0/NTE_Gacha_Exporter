<script setup lang="ts">
import { ChevronDown, ChevronLeft, ChevronRight, Search, SlidersHorizontal, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack records-workbench" data-agent-id="view-records">
        <section class="record-filter-shell">
          <div class="record-filter-head">
            <div class="record-filter-title">
              <strong>{{ app.t("records.filters") }}</strong>
              <span class="active-filter-count">{{ app.t("records.activeFilters", { count: app.activeRecordFilterCount }) }}</span>
            </div>
            <div class="filter-actions">
              <button type="button" class="ghost" :disabled="app.activeRecordFilterCount === 0 || app.isWorkflowBusy" @click="app.resetRecordFilters">
                <X :size="16" />
                <span>{{ app.t("records.clearFilters") }}</span>
              </button>
              <button type="button" @click="app.recordAdvancedFiltersOpen = !app.recordAdvancedFiltersOpen">
                <SlidersHorizontal :size="16" />
                <span>{{ app.t("records.advancedFilters") }}</span>
                <ChevronDown :size="16" />
              </button>
            </div>
          </div>

          <div class="toolbar dense">
            <div class="segmented">
              <button :class="{ active: app.recordPoolKind === 'all' }" type="button" @click="app.recordPoolKind = 'all'">{{ app.t("common.all") }}</button>
              <button
                v-for="kind in app.kindOrder"
                :key="kind"
                :class="{ active: app.recordPoolKind === kind }"
                type="button"
                @click="app.recordPoolKind = kind"
              >
                {{ app.kindLabels[kind] }}
              </button>
            </div>
            <label class="search-box">
              <Search :size="17" />
              <input v-model="app.search" :placeholder="app.t('common.search')" />
            </label>
          </div>

          <div class="filter-grid basic">
            <label class="field">
              <span>{{ app.t("records.pool") }}</span>
              <select v-model="app.recordPoolId">
                <option value="">{{ app.t("records.allPools") }}</option>
                <option v-for="pool in app.poolsForRecordKind" :key="pool.pool_id" :value="pool.pool_id">
                  {{ pool.label }} ({{ pool.count }})
                </option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("common.banner") }}</span>
              <select v-model="app.recordBannerId">
                <option value="">{{ app.t("records.allBanners") }}</option>
                <option v-for="banner in app.bannersForRecordKind" :key="banner.banner_id" :value="banner.banner_id">
                  {{ banner.title }} ({{ banner.count }})
                </option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("common.type") }}</span>
              <select v-model="app.recordType">
                <option value="">{{ app.t("records.allTypes") }}</option>
                <option v-for="type in app.filterOptions.record_types" :key="type.record_type" :value="type.record_type">
                  {{ type.record_type }} ({{ type.count }})
                </option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("records.hitRarity") }}</span>
              <select v-model="app.hitRarity">
                <option value="">{{ app.t("records.allHits") }}</option>
                <option value="5">5★</option>
                <option value="4">4★</option>
              </select>
            </label>
          </div>

          <div v-if="app.recordAdvancedFiltersOpen" class="filter-grid advanced">
            <label class="field">
              <span>{{ app.t("sort.rateUp") }}</span>
              <select v-model="app.rateUpResult">
                <option value="">{{ app.t("records.allResults") }}</option>
                <option value="up">UP</option>
                <option value="off_rate">{{ app.t("format.offRate") }}</option>
                <option value="not_applicable">{{ app.t("format.notApplicable") }}</option>
                <option value="unknown">{{ app.t("common.unknown") }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("records.from") }}</span>
              <input v-model="app.dateFrom" type="date" />
            </label>
            <label class="field">
              <span>{{ app.t("records.to") }}</span>
              <input v-model="app.dateTo" type="date" />
            </label>
            <label class="field">
              <span>{{ app.t("records.sort") }}</span>
              <select v-model="app.sortKey">
                <option value="time">{{ app.t("sort.time") }}</option>
                <option value="banner">{{ app.t("sort.banner") }}</option>
                <option value="pool">{{ app.t("sort.pool") }}</option>
                <option value="item">{{ app.t("sort.item") }}</option>
                <option value="rarity">{{ app.t("sort.rarity") }}</option>
                <option value="record_type">{{ app.t("sort.type") }}</option>
                <option value="pull_no">{{ app.t("sort.pullNo") }}</option>
                <option value="pity_5">{{ app.t("sort.pity5") }}</option>
                <option value="pity_4">{{ app.t("sort.pity4") }}</option>
                <option value="rate_up">{{ app.t("sort.rateUp") }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("records.direction") }}</span>
              <select v-model="app.sortDirection">
                <option value="desc">{{ app.t("common.desc") }}</option>
                <option value="asc">{{ app.t("common.asc") }}</option>
              </select>
            </label>
            <label class="field">
              <span>{{ app.t("records.pity5Min") }}</span>
              <input v-model="app.pity5Min" inputmode="numeric" placeholder="0" />
            </label>
            <label class="field">
              <span>{{ app.t("records.pity5Max") }}</span>
              <input v-model="app.pity5Max" inputmode="numeric" placeholder="90" />
            </label>
            <label class="field">
              <span>{{ app.t("records.pity4Min") }}</span>
              <input v-model="app.pity4Min" inputmode="numeric" placeholder="0" />
            </label>
            <label class="field">
              <span>{{ app.t("records.pity4Max") }}</span>
              <input v-model="app.pity4Max" inputmode="numeric" placeholder="10" />
            </label>
          </div>
        </section>

        <section class="panel" data-agent-id="records-history">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.t("records.pageRange", { start: app.recordPageStart, end: app.recordPageEnd, total: app.recordTotal }) }}</span>
              <h2>{{ app.t("records.history") }}</h2>
            </div>
            <div class="pager">
              <select v-model.number="app.pageSize">
                <option :value="50">50</option>
                <option :value="100">100</option>
                <option :value="200">200</option>
              </select>
              <button type="button" :disabled="!app.canPrevPage || app.isWorkflowBusy" :title="app.t('records.previousPage')" @click="app.pageIndex--">
                <ChevronLeft :size="16" />
              </button>
              <button type="button" :disabled="!app.canNextPage || app.isWorkflowBusy" :title="app.t('records.nextPage')" @click="app.pageIndex++">
                <ChevronRight :size="16" />
              </button>
            </div>
          </div>
          <div class="record-table history-table" :class="{ 'without-visual': !app.recordsHaveAnyVisual() }">
            <div class="record-header history-header" :class="{ 'without-visual': !app.recordsHaveAnyVisual() }">
              <span>{{ app.t("common.time") }}</span>
              <span>{{ app.t("common.banner") }}</span>
              <span>{{ app.t("common.item") }}</span>
              <span>{{ app.t("dashboard.rarity") }}</span>
              <span>{{ app.t("records.pullNo") }}</span>
              <span>{{ app.t("records.pity") }}</span>
              <span>{{ app.t("common.result") }}</span>
              <span>{{ app.t("records.rolls") }}</span>
              <span v-if="app.recordsHaveAnyVisual()">{{ app.t("common.visual") }}</span>
            </div>
            <div v-for="record in app.records" :key="record.record_id" class="record-line history-line" :class="{ 'without-visual': !app.recordsHaveAnyVisual() }">
              <span>{{ app.formatTime(record.time) }}</span>
              <span>
                <strong>{{ app.bannerTitle(record.banner) }}</strong>
                <small>{{ app.bannerMeta(record.banner) }}</small>
              </span>
              <span>
                <strong>{{ record.item_name }}</strong>
                <small v-if="record.secondary_item_name">{{ record.secondary_item_name }} x{{ record.secondary_count ?? 1 }}</small>
              </span>
              <span>{{ record.rarity ? `${record.rarity}★` : "-" }}</span>
              <span>{{ app.formatPullNo(record) }}</span>
              <span>{{ app.formatPity(record) }}</span>
              <span>
                <span class="derived-chip">{{ app.formatResult(record.derived.rate_up_result) }}</span>
                <small v-if="app.formatGuarantee(record)">{{ app.formatGuarantee(record) }}</small>
              </span>
              <span>{{ record.roll_points ?? "-" }}</span>
              <span v-if="app.recordsHaveAnyVisual()" class="history-visual">
                <span v-if="app.hasRecordVisual(record)" class="item-thumb small">
                  <img :src="app.itemVisualUrl(record)" alt="" />
                </span>
                <span v-if="app.hasRecordVisual(record)" class="history-visual-meta">{{ app.t("records.visualRefs", { count: app.assetRefsCount(record.item_asset_refs) + app.assetRefsCount(record.banner.asset_refs) }) }}</span>
              </span>
            </div>
            <div v-if="app.records.length === 0" class="empty-row">{{ app.t("records.empty") }}</div>
          </div>
        </section>
      </section>
</template>
