<script setup lang="ts">
import { ChevronDown, ChevronLeft, ChevronRight, Search, SkipBack, SkipForward, SlidersHorizontal, X } from "lucide-vue-next";
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
              <button data-record-pool-kind="all" :class="{ active: app.recordPoolKind === 'all' }" type="button" @click="app.recordPoolKind = 'all'">{{ app.t("common.all") }}</button>
              <button
                v-for="kind in app.kindOrder"
                :key="kind"
                :data-record-pool-kind="kind"
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
              <span>{{ app.t("records.focusedRarity") }}</span>
              <MultiSelectDropdown
                v-model="app.focusedRarities"
                :label="app.t('records.focusedRarity')"
                :all-label="app.t('records.allFocusedRarities')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.focusedRarityOptions"
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
              <h2>{{ app.t("records.history") }}</h2>
            </div>
            <div class="pager">
              <MultiSelectDropdown
                v-model="app.visibleRecordColumns"
                class="record-column-select"
                :label="app.t('records.columns')"
                :all-label="app.t('records.noColumns')"
                :all-selected-label="app.t('records.allColumns')"
                :selected-label="app.t('records.selectedCount')"
                :options="app.recordColumnOptions"
                :disabled="app.isWorkflowBusy"
              />
              <select v-model="app.sortDirection" class="time-order-select" :title="app.t('records.timeOrder')">
                <option value="desc">{{ app.t("records.newestFirst") }}</option>
                <option value="asc">{{ app.t("records.oldestFirst") }}</option>
              </select>
              <select v-model.number="app.pageSize">
                <option v-for="size in app.recordPageSizes" :key="size" :value="size">{{ size }}</option>
              </select>
              <button type="button" class="page-button" :disabled="!app.canFirstPage || app.isWorkflowBusy" :title="app.t('records.firstPage')" :aria-label="app.t('records.firstPage')" @click="app.goToFirstRecordPage">
                <SkipBack :size="16" />
              </button>
              <button type="button" class="page-button" :disabled="!app.canPrevPage || app.isWorkflowBusy" :title="app.t('records.previousPage')" :aria-label="app.t('records.previousPage')" @click="app.pageIndex--">
                <ChevronLeft :size="16" />
              </button>
              <button
                type="button"
                class="pager-range"
                :disabled="app.recordTotal === 0 || app.isWorkflowBusy"
                :title="app.t('records.jumpToPage')"
                @click="app.openRecordPageJump"
              >
                {{ app.t("records.pageRange", { start: app.recordPageStart, end: app.recordPageEnd, total: app.recordTotal }) }}
              </button>
              <button type="button" class="page-button" :disabled="!app.canNextPage || app.isWorkflowBusy" :title="app.t('records.nextPage')" :aria-label="app.t('records.nextPage')" @click="app.pageIndex++">
                <ChevronRight :size="16" />
              </button>
              <button type="button" class="page-button" :disabled="!app.canLastPage || app.isWorkflowBusy" :title="app.t('records.lastPage')" :aria-label="app.t('records.lastPage')" @click="app.goToLastRecordPage">
                <SkipForward :size="16" />
              </button>
            </div>
          </div>
          <div class="record-table history-table">
            <div class="record-header history-header" :style="{ '--history-grid-template': app.visibleRecordGridTemplate }">
              <span v-if="app.isRecordColumnVisible('index')">#</span>
              <span v-if="app.isRecordColumnVisible('time')">{{ app.t("common.time") }}</span>
              <span v-if="app.isRecordColumnVisible('banner')">{{ app.t("common.banner") }}</span>
              <span v-if="app.isRecordColumnVisible('item')">{{ app.t("common.item") }}</span>
              <span v-if="app.isRecordColumnVisible('rarity')">{{ app.t("dashboard.rarity") }}</span>
              <span v-if="app.isRecordColumnVisible('pullNo')">{{ app.t("records.pullNo") }}</span>
              <span v-if="app.isRecordColumnVisible('fiveStarProgress')">{{ app.t("records.fiveStarProgress") }}</span>
              <span v-if="app.isRecordColumnVisible('tenPullProgress')">{{ app.t("records.tenPullProgress") }}</span>
              <span v-if="app.isRecordColumnVisible('rolls')">{{ app.t("records.rolls") }}</span>
            </div>
            <div
              v-for="record in app.records"
              :key="record.record_id"
              class="record-line history-line"
              :data-record-id="record.record_id"
              :data-item-id="record.item_id"
              :data-pool-kind="record.pool_kind"
              :data-item-kind="record.item_kind"
              :data-rarity="record.rarity ?? ''"
              :data-rate-up-result="record.derived.rate_up_result"
              :style="{ '--history-grid-template': app.visibleRecordGridTemplate }"
            >
              <span v-if="app.isRecordColumnVisible('index')">{{ app.formatPoolKindPullNo(record) }}</span>
              <span v-if="app.isRecordColumnVisible('time')">{{ app.formatTime(record.time) }}</span>
              <span v-if="app.isRecordColumnVisible('banner')" class="history-banner-cell">
                <span v-if="app.hasBannerVisual(record.banner)" class="banner-row-thumb">
                  <img :src="app.bannerVisualUrl(record.banner)" alt="" />
                </span>
                <span v-else class="banner-row-thumb empty">{{ app.bannerTitle(record.banner).slice(0, 1) }}</span>
                <span>
                  <strong>{{ app.bannerTitle(record.banner) }}</strong>
                  <small v-if="app.bannerMeta(record.banner)">{{ app.bannerMeta(record.banner) }}</small>
                </span>
              </span>
              <span v-if="app.isRecordColumnVisible('item')" class="history-item-cell">
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
              <span v-if="app.isRecordColumnVisible('rarity')" class="record-rarity" :class="app.recordRarityClass(record)">{{ record.rarity ? `${record.rarity}★` : "-" }}</span>
              <span v-if="app.isRecordColumnVisible('pullNo')">{{ app.formatPullNo(record) }}</span>
              <span v-if="app.isRecordColumnVisible('fiveStarProgress')">{{ app.formatPity(record) }}</span>
              <span v-if="app.isRecordColumnVisible('tenPullProgress')">{{ app.formatTenPullProgress(record) }}</span>
              <span v-if="app.isRecordColumnVisible('rolls')">{{ app.formatRolls(record) }}</span>
            </div>
            <div v-if="app.records.length === 0" class="empty-row">{{ app.t("records.empty") }}</div>
          </div>
        </section>
        <div v-if="app.recordPageJumpOpen" class="page-jump-dialog-backdrop" @click.self="app.closeRecordPageJump">
          <section class="page-jump-dialog" role="dialog" aria-modal="true" :aria-label="app.t('records.jumpToPage')" @keydown.esc="app.closeRecordPageJump">
            <div class="page-jump-dialog-head">
              <h2>{{ app.t("records.jumpToPage") }}</h2>
              <button type="button" class="icon-button" :title="app.t('common.close')" @click="app.closeRecordPageJump">
                <X :size="16" />
              </button>
            </div>
            <form class="page-jump-dialog-body" @submit.prevent="app.confirmRecordPageJump">
              <label class="field">
                <span>{{ app.t("records.pageNumber") }}</span>
                <input v-model="app.recordPageJumpInput" type="number" min="1" :max="app.recordPageCount || 1" step="1" autofocus />
              </label>
              <p>{{ app.t("records.pageCount", { count: app.recordPageCount }) }}</p>
              <div class="page-jump-actions">
                <button type="button" class="ghost" @click="app.closeRecordPageJump">{{ app.t("common.cancel") }}</button>
                <button type="submit">{{ app.t("records.goToPage") }}</button>
              </div>
            </form>
          </section>
        </div>
      </section>
</template>
