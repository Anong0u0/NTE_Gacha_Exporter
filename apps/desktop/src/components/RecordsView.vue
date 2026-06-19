<script setup lang="ts">
import { ChevronLeft, ChevronRight, Image, Search } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <section class="view-stack" data-agent-id="view-records">
        <section class="toolbar dense">
          <div class="segmented">
            <button :class="{ active: app.recordPoolKind === 'all' }" type="button" @click="app.recordPoolKind = 'all'">All</button>
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
          <label class="app.search-box">
            <Search :size="17" />
            <input v-model="app.search" placeholder="Search app.records" />
          </label>
        </section>

        <section class="filter-grid">
          <label class="field">
            <span>Pool</span>
            <select v-model="app.recordPoolId">
              <option value="">All pools</option>
              <option v-for="pool in app.poolsForRecordKind" :key="pool.pool_id" :value="pool.pool_id">
                {{ pool.label }} ({{ pool.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Banner</span>
            <select v-model="app.recordBannerId">
              <option value="">All banners</option>
              <option v-for="banner in app.bannersForRecordKind" :key="banner.banner_id" :value="banner.banner_id">
                {{ banner.title }} ({{ banner.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Type</span>
            <select v-model="app.recordType">
              <option value="">All types</option>
              <option v-for="type in app.filterOptions.record_types" :key="type.record_type" :value="type.record_type">
                {{ type.record_type }} ({{ type.count }})
              </option>
            </select>
          </label>
          <label class="field">
            <span>Hit rarity</span>
            <select v-model="app.hitRarity">
              <option value="">All hits</option>
              <option value="5">5★</option>
              <option value="4">4★</option>
            </select>
          </label>
          <label class="field">
            <span>Rate-up</span>
            <select v-model="app.rateUpResult">
              <option value="">All results</option>
              <option value="up">UP</option>
              <option value="off_rate">Off-rate</option>
              <option value="not_applicable">N/A</option>
              <option value="unknown">Unknown</option>
            </select>
          </label>
          <label class="field">
            <span>From</span>
            <input v-model="app.dateFrom" type="date" />
          </label>
          <label class="field">
            <span>To</span>
            <input v-model="app.dateTo" type="date" />
          </label>
          <label class="field">
            <span>Sort</span>
            <select v-model="app.sortKey">
              <option value="time">Time</option>
              <option value="banner">Banner</option>
              <option value="pool">Pool</option>
              <option value="item">Item</option>
              <option value="rarity">Rarity</option>
              <option value="record_type">Type</option>
              <option value="pull_no">Pull no</option>
              <option value="pity_5">5★ pity</option>
              <option value="pity_4">4★ pity</option>
              <option value="rate_up">Rate-up</option>
            </select>
          </label>
          <label class="field">
            <span>Direction</span>
            <select v-model="app.sortDirection">
              <option value="desc">Desc</option>
              <option value="asc">Asc</option>
            </select>
          </label>
          <label class="field">
            <span>5★ pity min</span>
            <input v-model="app.pity5Min" inputmode="numeric" placeholder="0" />
          </label>
          <label class="field">
            <span>5★ pity max</span>
            <input v-model="app.pity5Max" inputmode="numeric" placeholder="90" />
          </label>
          <label class="field">
            <span>4★ pity min</span>
            <input v-model="app.pity4Min" inputmode="numeric" placeholder="0" />
          </label>
          <label class="field">
            <span>4★ pity max</span>
            <input v-model="app.pity4Max" inputmode="numeric" placeholder="10" />
          </label>
        </section>

        <section class="panel" data-agent-id="records-history">
          <div class="panel-head">
            <div>
              <span class="eyebrow">{{ app.recordPageStart }}-{{ app.recordPageEnd }} of {{ app.recordTotal }}</span>
              <h2>History</h2>
            </div>
            <div class="pager">
              <select v-model.number="app.pageSize">
                <option :value="50">50</option>
                <option :value="100">100</option>
                <option :value="200">200</option>
              </select>
              <button type="button" :disabled="!app.canPrevPage || app.isWorkflowBusy" title="Previous page" @click="app.pageIndex--">
                <ChevronLeft :size="16" />
              </button>
              <button type="button" :disabled="!app.canNextPage || app.isWorkflowBusy" title="Next page" @click="app.pageIndex++">
                <ChevronRight :size="16" />
              </button>
            </div>
          </div>
          <div class="record-table history-table">
            <div class="record-header history-header">
              <span>Time</span>
              <span>Banner</span>
              <span>Item</span>
              <span>Rarity</span>
              <span>Pull</span>
              <span>Pity</span>
              <span>Result</span>
              <span>Rolls</span>
              <span>Visual</span>
            </div>
            <div v-for="record in app.records" :key="record.record_id" class="record-line history-line">
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
                <small>{{ app.formatGuarantee(record) }}</small>
              </span>
              <span>{{ record.roll_points ?? "-" }}</span>
              <span class="history-visual">
                <span class="item-thumb small">
                  <img v-if="app.itemVisualUrl(record)" :src="app.itemVisualUrl(record)" alt="" />
                  <span v-else class="asset-placeholder"><Image :size="15" /></span>
                </span>
                <span class="history-visual-meta">{{ app.assetRefsCount(record.item_asset_refs) + app.assetRefsCount(record.banner.asset_refs) }} refs</span>
              </span>
            </div>
            <div v-if="app.records.length === 0" class="empty-row">No app.records match current filters.</div>
          </div>
        </section>
      </section>
</template>
