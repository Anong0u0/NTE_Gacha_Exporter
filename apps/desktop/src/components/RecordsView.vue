<script setup lang="ts">
import { ChevronDown, ChevronLeft, ChevronRight, Search, SlidersHorizontal, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";
import MultiSelectDropdown from "./MultiSelectDropdown.vue";

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
              <span>{{ app.t("common.banner") }}</span>
              <MultiSelectDropdown
                v-model="app.recordBannerIds"
                :label="app.t('common.banner')"
                :all-label="app.t('records.allBanners')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.recordBannerOptions"
              />
            </label>
            <label class="field">
              <span>{{ app.t("records.itemRarity") }}</span>
              <MultiSelectDropdown
                v-model="app.itemRarities"
                :label="app.t('records.itemRarity')"
                :all-label="app.t('records.allItemRarities')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.itemRarityOptions"
              />
            </label>
            <label class="field">
              <span>{{ app.t("records.hitRarity") }}</span>
              <MultiSelectDropdown
                v-model="app.hitRarities"
                :label="app.t('records.hitRarity')"
                :all-label="app.t('records.allHits')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.hitRarityOptions"
              />
            </label>
          </div>

          <div v-if="app.recordAdvancedFiltersOpen" class="filter-grid advanced">
            <label class="field">
              <span>{{ app.t("records.upResult") }}</span>
              <MultiSelectDropdown
                v-model="app.rateUpResults"
                :label="app.t('records.upResult')"
                :all-label="app.t('records.allResults')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.rateUpResultSelectOptions"
              />
            </label>
            <label class="field">
              <span>{{ app.t("records.rollBucket") }}</span>
              <MultiSelectDropdown
                v-model="app.rollBuckets"
                :label="app.t('records.rollBucket')"
                :all-label="app.t('records.allRollBuckets')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.rollBucketOptions"
              />
            </label>
            <label class="field">
              <span>{{ app.t("records.itemKind") }}</span>
              <MultiSelectDropdown
                v-model="app.itemKinds"
                :label="app.t('records.itemKind')"
                :all-label="app.t('records.allItemKinds')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.itemKindOptions"
              />
            </label>
            <label v-if="app.showForkRecordFilters" class="field">
              <span>{{ app.t("records.forkResultMark") }}</span>
              <MultiSelectDropdown
                v-model="app.forkResultMarks"
                :label="app.t('records.forkResultMark')"
                :all-label="app.t('records.allForkResultMarks')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.forkResultMarkSelectOptions"
              />
            </label>
            <label v-if="app.showForkRecordFilters" class="field">
              <span>{{ app.t("records.forkPityBadge") }}</span>
              <MultiSelectDropdown
                v-model="app.forkPityBadges"
                :label="app.t('records.forkPityBadge')"
                :all-label="app.t('records.allForkPityBadges')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.forkPityBadgeSelectOptions"
              />
            </label>
            <label class="field">
              <span>{{ app.t("records.from") }}</span>
              <input v-model="app.dateFrom" type="date" />
            </label>
            <label class="field">
              <span>{{ app.t("records.to") }}</span>
              <input v-model="app.dateTo" type="date" />
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
              <select v-model="app.sortDirection" class="time-order-select" :title="app.t('records.timeOrder')">
                <option value="desc">{{ app.t("records.newestFirst") }}</option>
                <option value="asc">{{ app.t("records.oldestFirst") }}</option>
              </select>
              <select v-model.number="app.pageSize">
                <option v-for="size in app.recordPageSizes" :key="size" :value="size">{{ size }}</option>
              </select>
              <button type="button" :disabled="!app.canPrevPage || app.isWorkflowBusy" :title="app.t('records.previousPage')" @click="app.pageIndex--">
                <ChevronLeft :size="16" />
              </button>
              <button type="button" :disabled="!app.canNextPage || app.isWorkflowBusy" :title="app.t('records.nextPage')" @click="app.pageIndex++">
                <ChevronRight :size="16" />
              </button>
            </div>
          </div>
          <div class="record-table history-table">
            <div class="record-header history-header">
              <span>#</span>
              <span>{{ app.t("common.time") }}</span>
              <span>{{ app.t("common.banner") }}</span>
              <span>{{ app.t("common.item") }}</span>
              <span>{{ app.t("dashboard.rarity") }}</span>
              <span>{{ app.t("records.pullNo") }}</span>
              <span>{{ app.t("records.fiveStarProgress") }}</span>
              <span>{{ app.t("records.tenPullProgress") }}</span>
              <span>{{ app.t("records.rolls") }}</span>
            </div>
            <div v-for="record in app.records" :key="record.record_id" class="record-line history-line">
              <span>{{ app.formatPoolKindPullNo(record) }}</span>
              <span>{{ app.formatTime(record.time) }}</span>
              <span class="history-banner-cell">
                <span v-if="app.hasBannerVisual(record.banner)" class="banner-row-thumb">
                  <img :src="app.bannerVisualUrl(record.banner)" alt="" />
                </span>
                <span v-else class="banner-row-thumb empty">{{ app.bannerTitle(record.banner).slice(0, 1) }}</span>
                <span>
                  <strong>{{ app.bannerTitle(record.banner) }}</strong>
                  <small v-if="app.bannerMeta(record.banner)">{{ app.bannerMeta(record.banner) }}</small>
                </span>
              </span>
              <span class="history-item-cell">
                <span v-if="app.hasRecordVisual(record)" class="history-item-thumb">
                  <img :src="app.itemVisualUrl(record)" alt="" />
                </span>
                <span class="history-item-text">
                  <span v-if="app.primaryRecordBadge(record) || app.formatPityBadge(record)" class="record-badge-strip">
                    <span
                      v-if="app.primaryRecordBadge(record)"
                      :class="app.isHitBadgeLabel(app.primaryRecordBadge(record)) ? ['hit-badge', `hit-${app.primaryRecordBadge(record).toLowerCase()}`] : 'derived-chip'"
                    >
                      {{ app.primaryRecordBadge(record) }}
                    </span>
                    <small v-if="app.formatPityBadge(record)" class="record-guarantee-badge">{{ app.formatPityBadge(record) }}</small>
                  </span>
                  <strong class="history-item-name" :class="app.recordRarityClass(record)">{{ app.formatQuantityName(record.item_name, record.count) }}</strong>
                  <small v-if="record.secondary_item_name">{{ app.formatQuantityName(record.secondary_item_name, record.secondary_count) }}</small>
                </span>
              </span>
              <span class="record-rarity" :class="app.recordRarityClass(record)">{{ record.rarity ? `${record.rarity}★` : "-" }}</span>
              <span>{{ app.formatPullNo(record) }}</span>
              <span>{{ app.formatPity(record) }}</span>
              <span>{{ app.formatTenPullProgress(record) }}</span>
              <span>{{ app.formatRolls(record) }}</span>
            </div>
            <div v-if="app.records.length === 0" class="empty-row">{{ app.t("records.empty") }}</div>
          </div>
        </section>
      </section>
</template>
