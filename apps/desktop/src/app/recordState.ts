import { computed, ref, type ComputedRef, type Ref } from "vue";

import type {
  ItemKind,
  PityBadge,
  PoolKind,
  RateUpResult,
  RecordFilter,
  RecordFilterOptions,
  RollBucket,
  SortDirection,
  ForkResultMark,
} from "../api";
import type { I18nKey } from "./i18n";
import type { PoolKindFilter } from "./options";
import {
  defaultRecordViewPrefs,
  forkPityBadgeOptions,
  forkResultMarkOptions,
  isRecordRarity,
  rateUpResultOptions,
  readRecordViewPrefs,
  recordColumnGridTracks,
  recordColumnIds,
  recordPageSizes,
  recordPrefsKey,
  type FiveStarWallMode,
  type RecordColumnId,
  type RecordPageSize,
  type RecordViewPrefs,
} from "./recordPrefs";
import {
  formatForkResultMark,
  bannerTitle,
  formatItemKind,
  formatPityBadgeValue,
  formatResult,
  formatRollBucket,
} from "./viewHelpers";

type Translator = (
  key: I18nKey,
  params?: Record<string, string | number | boolean | null | undefined>,
) => string;

type RecordStateDeps = {
  activeProfileName: Ref<string>;
  filterOptions: Ref<RecordFilterOptions>;
  isWorkflowBusy(): boolean;
  t: Translator;
};

export function createRecordState(deps: RecordStateDeps) {
  const recordPoolKind = ref<PoolKindFilter>("all");
  const recordBannerIds = ref<string[]>([]);
  const itemRarities = ref<number[]>([]);
  const focusedRarities = ref<number[]>([]);
  const rateUpResults = ref<RateUpResult[]>([]);
  const rollBuckets = ref<RollBucket[]>([]);
  const itemKinds = ref<ItemKind[]>([]);
  const forkResultMarks = ref<ForkResultMark[]>([]);
  const forkPityBadges = ref<PityBadge[]>([]);
  const dateFrom = ref("");
  const dateTo = ref("");
  const search = ref("");
  const sortDirection = ref<SortDirection>("desc");
  const pageSize = ref<number>(defaultRecordViewPrefs.pageSize);
  const pageIndex = ref(0);
  const recordPageJumpOpen = ref(false);
  const recordPageJumpInput = ref("1");
  const visibleRecordColumns = ref<RecordColumnId[]>([...defaultRecordViewPrefs.visibleRecordColumns]);
  const recordAdvancedFiltersOpen = ref(false);
  const latestFiveStarWallModes = ref<Record<PoolKind, FiveStarWallMode>>({
    ...defaultRecordViewPrefs.latestFiveStarWallModes,
  });

  let recordPrefsReady = false;
  let applyingRecordPrefs = false;
  let normalizingRecordFilters = false;
  let resettingRecordPage = false;
  let bannersForRecordKind: ComputedRef<RecordFilterOptions["banners"]> | null = null;
  let recordPageCount: ComputedRef<number> | null = null;

  const activeRecordFilterCount = computed(() =>
    [
      recordPoolKind.value !== "all",
      recordBannerIds.value.length > 0,
      itemRarities.value.length > 0,
      focusedRarities.value.length > 0,
      rateUpResults.value.length > 0,
      rollBuckets.value.length > 0,
      itemKinds.value.length > 0,
      forkResultMarks.value.length > 0,
      forkPityBadges.value.length > 0,
      Boolean(dateFrom.value),
      Boolean(dateTo.value),
      Boolean(search.value.trim()),
    ].filter(Boolean).length,
  );
  const recordColumnOptions = computed(() =>
    recordColumnIds.map((column) => ({
      value: column,
      label: recordColumnLabel(column),
    })),
  );
  const visibleRecordColumnSet = computed(() => new Set(visibleRecordColumns.value));
  const visibleRecordGridTemplate = computed(() =>
    visibleRecordColumns.value.map((column) => recordColumnGridTracks[column]).join(" ") || "none",
  );
  const recordBannerOptions = computed(() =>
    currentBannersForRecordKind().map((banner) => ({
      value: banner.banner_id,
      label: bannerTitle(banner, deps.t),
      meta: String(banner.count),
    })),
  );
  const focusedRarityOptions = computed(() =>
    [5, 4, 3].map((rarity) => ({
      value: rarity,
      label: `${rarity}★`,
    })),
  );
  const itemRarityOptions = computed(() =>
    [5, 4, 3].map((rarity) => ({
      value: rarity,
      label: `${rarity}★`,
    })),
  );
  const rateUpResultSelectOptions = computed(() =>
    rateUpResultOptions.map((result) => ({
      value: result,
      label: formatResult(result, deps.t),
    })),
  );
  const rollBucketOptions = computed(() =>
    deps.filterOptions.value.roll_buckets.map((bucket) => ({
      value: bucket.bucket,
      label: formatRollBucket(bucket.bucket, deps.t),
      meta: String(bucket.count),
    })),
  );
  const itemKindOptions = computed(() =>
    deps.filterOptions.value.item_kinds.map((itemKind) => ({
      value: itemKind.item_kind,
      label: formatItemKind(itemKind.item_kind, deps.t),
      meta: String(itemKind.count),
    })),
  );
  const showForkRecordFilters = computed(() => recordPoolKind.value === "all" || recordPoolKind.value === "fork_lottery");
  const forkResultMarkSelectOptions = computed(() =>
    forkResultMarkOptions.map((mark) => ({
      value: mark,
      label: formatForkResultMark(mark, deps.t),
    })),
  );
  const forkPityBadgeSelectOptions = computed(() =>
    forkPityBadgeOptions.map((badge) => ({
      value: badge,
      label: formatPityBadgeValue(badge, deps.t),
    })),
  );

  function bindBannersForRecordKind(value: ComputedRef<RecordFilterOptions["banners"]>) {
    bannersForRecordKind = value;
  }

  function bindRecordPageCount(value: ComputedRef<number>) {
    recordPageCount = value;
  }

  function currentBannersForRecordKind() {
    return bannersForRecordKind?.value ?? [];
  }

  function currentRecordViewPrefs(): RecordViewPrefs {
    const normalizedPageSize = recordPageSizes.includes(pageSize.value as RecordPageSize)
      ? (pageSize.value as RecordPageSize)
      : defaultRecordViewPrefs.pageSize;
    return {
      recordPoolKind: recordPoolKind.value,
      recordBannerIds: [...recordBannerIds.value],
      itemRarities: [...itemRarities.value],
      focusedRarities: [...focusedRarities.value],
      rateUpResults: [...rateUpResults.value],
      rollBuckets: [...rollBuckets.value],
      itemKinds: [...itemKinds.value],
      forkResultMarks: [...forkResultMarks.value],
      forkPityBadges: [...forkPityBadges.value],
      dateFrom: dateFrom.value,
      dateTo: dateTo.value,
      search: search.value,
      sortDirection: sortDirection.value,
      pageSize: normalizedPageSize,
      visibleRecordColumns: [...visibleRecordColumns.value],
      recordAdvancedFiltersOpen: recordAdvancedFiltersOpen.value,
      latestFiveStarWallModes: { ...latestFiveStarWallModes.value },
    };
  }

  function saveRecordViewPrefs(profileName = deps.activeProfileName.value) {
    if (!recordPrefsReady || !profileName) return;
    try {
      window.localStorage.setItem(recordPrefsKey(profileName), JSON.stringify(currentRecordViewPrefs()));
    } catch {
      // localStorage may be unavailable in restricted runtimes; record view still works.
    }
  }

  function setActiveProfileName(profileName: string) {
    if (deps.activeProfileName.value !== profileName) {
      recordPrefsReady = false;
    }
    deps.activeProfileName.value = profileName;
  }

  function copyRecordViewPrefs(sourceProfileName: string, targetProfileName: string) {
    if (!sourceProfileName || !targetProfileName || sourceProfileName === targetProfileName) return;
    try {
      const raw = window.localStorage.getItem(recordPrefsKey(sourceProfileName));
      if (raw) window.localStorage.setItem(recordPrefsKey(targetProfileName), raw);
    } catch {
      // Best-effort UI preference migration.
    }
  }

  function removeRecordViewPrefs(profileName: string) {
    if (!profileName) return;
    try {
      window.localStorage.removeItem(recordPrefsKey(profileName));
    } catch {
      // Best-effort cleanup only.
    }
  }

  function applyRecordViewPrefs(profileName = deps.activeProfileName.value) {
    const prefs = readRecordViewPrefs(profileName);
    withApplyingRecordPrefs(() => {
      recordPoolKind.value = prefs.recordPoolKind;
      recordBannerIds.value = [...prefs.recordBannerIds];
      itemRarities.value = [...prefs.itemRarities];
      focusedRarities.value = [...prefs.focusedRarities];
      rateUpResults.value = [...prefs.rateUpResults];
      rollBuckets.value = [...prefs.rollBuckets];
      itemKinds.value = [...prefs.itemKinds];
      forkResultMarks.value = [...prefs.forkResultMarks];
      forkPityBadges.value = [...prefs.forkPityBadges];
      dateFrom.value = prefs.dateFrom;
      dateTo.value = prefs.dateTo;
      search.value = prefs.search;
      sortDirection.value = prefs.sortDirection;
      pageSize.value = prefs.pageSize;
      visibleRecordColumns.value = [...prefs.visibleRecordColumns];
      recordAdvancedFiltersOpen.value = prefs.recordAdvancedFiltersOpen;
      latestFiveStarWallModes.value = { ...prefs.latestFiveStarWallModes };
      pageIndex.value = 0;
      normalizeRecordFilterSelection();
    });
    recordPrefsReady = true;
    saveRecordViewPrefs(profileName);
  }

  function normalizeRecordFilterSelection() {
    const availableBannerIds = new Set(currentBannersForRecordKind().map((banner) => banner.banner_id));
    const availableRollBuckets = new Set(deps.filterOptions.value.roll_buckets.map((bucket) => bucket.bucket));
    const availableItemKinds = new Set(deps.filterOptions.value.item_kinds.map((itemKind) => itemKind.item_kind));
    recordBannerIds.value = recordBannerIds.value.filter((bannerId) => availableBannerIds.has(bannerId));
    itemRarities.value = itemRarities.value.filter(isRecordRarity);
    focusedRarities.value = focusedRarities.value.filter(isRecordRarity);
    rateUpResults.value = rateUpResults.value.filter((result) => rateUpResultOptions.includes(result));
    rollBuckets.value = rollBuckets.value.filter((bucket) => availableRollBuckets.has(bucket));
    itemKinds.value = itemKinds.value.filter((itemKind) => availableItemKinds.has(itemKind));
    if (recordPoolKind.value === "monopoly_limited" || recordPoolKind.value === "monopoly_standard") {
      forkResultMarks.value = [];
      forkPityBadges.value = [];
    } else {
      forkResultMarks.value = forkResultMarks.value.filter((mark) => forkResultMarkOptions.includes(mark));
      forkPityBadges.value = forkPityBadges.value.filter((badge) => forkPityBadgeOptions.includes(badge));
    }
    if (!recordPageSizes.includes(pageSize.value as RecordPageSize)) pageSize.value = defaultRecordViewPrefs.pageSize;
    visibleRecordColumns.value = recordColumnIds.filter((column) => visibleRecordColumns.value.includes(column));
  }

  function recordColumnLabel(column: RecordColumnId) {
    switch (column) {
      case "index":
        return "#";
      case "time":
        return deps.t("common.time");
      case "banner":
        return deps.t("common.banner");
      case "item":
        return deps.t("common.item");
      case "rarity":
        return deps.t("dashboard.rarity");
      case "pullNo":
        return deps.t("records.pullNo");
      case "fiveStarProgress":
        return deps.t("records.fiveStarProgress");
      case "tenPullProgress":
        return deps.t("records.tenPullProgress");
      case "rolls":
        return deps.t("records.rolls");
    }
  }

  function isRecordColumnVisible(column: RecordColumnId) {
    return visibleRecordColumnSet.value.has(column);
  }

  function goToRecordPage(pageNumber: number) {
    const pageCount = recordPageCount?.value ?? 0;
    if (pageCount === 0) return;
    if (!Number.isFinite(pageNumber)) return;
    const target = Math.trunc(pageNumber);
    const clamped = Math.min(Math.max(target, 1), pageCount);
    pageIndex.value = clamped - 1;
  }

  function goToFirstRecordPage() {
    goToRecordPage(1);
  }

  function goToLastRecordPage() {
    goToRecordPage(recordPageCount?.value ?? 0);
  }

  function openRecordPageJump() {
    if ((recordPageCount?.value ?? 0) === 0 || deps.isWorkflowBusy()) return;
    recordPageJumpInput.value = String(pageIndex.value + 1);
    recordPageJumpOpen.value = true;
  }

  function closeRecordPageJump() {
    recordPageJumpOpen.value = false;
  }

  function confirmRecordPageJump() {
    const value = Number.parseInt(recordPageJumpInput.value, 10);
    if (Number.isFinite(value)) goToRecordPage(value);
    closeRecordPageJump();
  }

  function currentRecordFilter(): RecordFilter {
    return {
      pool_kind: recordPoolKind.value === "all" ? null : recordPoolKind.value,
      banner_ids: recordBannerIds.value,
      rarities: itemRarities.value,
      focused_rarities: focusedRarities.value,
      rate_up_results: rateUpResults.value,
      roll_buckets: rollBuckets.value,
      item_kinds: itemKinds.value,
      fork_result_marks: forkResultMarks.value,
      fork_pity_badges: forkPityBadges.value,
      date_from: dateFrom.value || null,
      date_to: dateTo.value || null,
      search: search.value || null,
      sort_direction: sortDirection.value,
      limit: pageSize.value,
      offset: pageIndex.value * pageSize.value,
    };
  }

  function recordFilterKey(filter: RecordFilter) {
    return JSON.stringify(filter);
  }

  function resetRecordFilters(loadRecords: () => void) {
    withApplyingRecordPrefs(() => {
      recordPoolKind.value = "all";
      recordBannerIds.value = [];
      itemRarities.value = [];
      focusedRarities.value = [];
      rateUpResults.value = [];
      rollBuckets.value = [];
      itemKinds.value = [];
      forkResultMarks.value = [];
      forkPityBadges.value = [];
      dateFrom.value = "";
      dateTo.value = "";
      search.value = "";
      pageIndex.value = 0;
    });
    saveRecordViewPrefs();
    loadRecords();
  }

  function withApplyingRecordPrefs(callback: () => void) {
    applyingRecordPrefs = true;
    try {
      callback();
    } finally {
      applyingRecordPrefs = false;
    }
  }

  function normalizeAfterFilterWatch() {
    normalizingRecordFilters = true;
    try {
      normalizeRecordFilterSelection();
    } finally {
      normalizingRecordFilters = false;
    }
  }

  function resetPageAfterFilterWatch() {
    resettingRecordPage = true;
    try {
      pageIndex.value = 0;
    } finally {
      resettingRecordPage = false;
    }
  }

  function shouldSkipFilterWatch() {
    return applyingRecordPrefs || normalizingRecordFilters;
  }

  function shouldSkipPageWatch() {
    return applyingRecordPrefs || resettingRecordPage;
  }

  function shouldSkipPrefsWatch() {
    return applyingRecordPrefs;
  }

  function shouldSkipColumnWatch() {
    return applyingRecordPrefs || normalizingRecordFilters;
  }

  function isRecordPrefsReady() {
    return recordPrefsReady;
  }

  return {
    refs: {
      recordPoolKind,
      recordBannerIds,
      itemRarities,
      focusedRarities,
      rateUpResults,
      rollBuckets,
      itemKinds,
      forkResultMarks,
      forkPityBadges,
      dateFrom,
      dateTo,
      search,
      sortDirection,
      pageSize,
      pageIndex,
      recordPageJumpOpen,
      recordPageJumpInput,
      visibleRecordColumns,
      recordAdvancedFiltersOpen,
      latestFiveStarWallModes,
    },
    computed: {
      activeRecordFilterCount,
      recordColumnOptions,
      visibleRecordGridTemplate,
      recordBannerOptions,
      itemRarityOptions,
      focusedRarityOptions,
      rateUpResultSelectOptions,
      rollBucketOptions,
      itemKindOptions,
      showForkRecordFilters,
      forkResultMarkSelectOptions,
      forkPityBadgeSelectOptions,
      recordPageSizes,
    },
    actions: {
      isRecordColumnVisible,
      goToRecordPage,
      goToFirstRecordPage,
      goToLastRecordPage,
      openRecordPageJump,
      closeRecordPageJump,
      confirmRecordPageJump,
      resetRecordFilters,
    },
    internal: {
      bindBannersForRecordKind,
      bindRecordPageCount,
      currentRecordFilter,
      recordFilterKey,
      saveRecordViewPrefs,
      setActiveProfileName,
      copyRecordViewPrefs,
      removeRecordViewPrefs,
      applyRecordViewPrefs,
      normalizeRecordFilterSelection,
      withApplyingRecordPrefs,
      normalizeAfterFilterWatch,
      resetPageAfterFilterWatch,
      shouldSkipFilterWatch,
      shouldSkipPageWatch,
      shouldSkipPrefsWatch,
      shouldSkipColumnWatch,
      isRecordPrefsReady,
    },
  };
}
