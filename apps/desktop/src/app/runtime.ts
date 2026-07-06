import { nextTick, onBeforeUnmount, onMounted, watch, type Ref } from "vue";

import {
  api,
  type CaptureStatus,
  type DashboardOverview,
  type DashboardSelection,
  type DashboardSelectionDetail,
  type DiagnosticStatus,
  type DisplayRecord,
  type ForkResultMark,
  type ItemKind,
  type PityBadge,
  type PoolKind,
  type RateUpResult,
  type RecordFilter,
  type RecordFilterOptions,
  type RollBucket,
  type Settings,
  type SortDirection,
} from "../api";
import type { I18nKey } from "./i18n";
import type { PoolKindFilter } from "./options";
import type { RecordColumnId } from "./recordPrefs";

type AppRuntimeDeps = {
  busy: Ref<boolean>;
  statusText: Ref<string>;
  errorText: Ref<string>;
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  uiLocale: Ref<string>;
  locales: Ref<string[]>;
  uiLocales: Ref<string[]>;
  summary: Ref<DashboardOverview | null>;
  selectedDashboardScope: Ref<DashboardSelection>;
  selectedPoolKind: Ref<PoolKind>;
  detail: Ref<DashboardSelectionDetail | null>;
  detailLoading: Ref<boolean>;
  filterOptions: Ref<RecordFilterOptions>;
  records: Ref<DisplayRecord[]>;
  recordTotal: Ref<number>;
  chartEl: Ref<HTMLElement | null>;
  rankingDialogOpen: Ref<boolean>;
  recordPoolKind: Ref<PoolKindFilter>;
  recordBannerIds: Ref<string[]>;
  itemRarities: Ref<number[]>;
  focusedRarities: Ref<number[]>;
  rateUpResults: Ref<RateUpResult[]>;
  rollBuckets: Ref<RollBucket[]>;
  itemKinds: Ref<ItemKind[]>;
  forkResultMarks: Ref<ForkResultMark[]>;
  forkPityBadges: Ref<PityBadge[]>;
  dateFrom: Ref<string>;
  dateTo: Ref<string>;
  search: Ref<string>;
  sortDirection: Ref<SortDirection>;
  pageSize: Ref<number>;
  pageIndex: Ref<number>;
  recordAdvancedFiltersOpen: Ref<boolean>;
  visibleRecordColumns: Ref<RecordColumnId[]>;
  captureStatus: Ref<CaptureStatus | null>;
  diagnosticStatus: Ref<DiagnosticStatus | null>;
  applySettings(settings: Settings): void;
  loadProfiles(): Promise<void>;
  loadUpdaterStatus(): Promise<void>;
  startPendingAdminCapture(): Promise<boolean>;
  startPendingAdminDiagnostic(): Promise<boolean>;
  checkForUpdates(options: { silent?: boolean }): Promise<void>;
  clearCapturePolling(): void;
  clearDiagnosticPolling(): void;
  disposeChart(): void;
  renderChart(): void;
  currentRecordFilter(): RecordFilter;
  recordFilterKey(filter: RecordFilter): string;
  saveRecordViewPrefs(profileName?: string): void;
  normalizeDashboardScope(fallbackPoolKind?: PoolKind): void;
  loadDetail(): Promise<void>;
  resolveVisibleAssets(): Promise<void>;
  isRecordPrefsReady(): boolean;
  applyRecordViewPrefs(): void;
  normalizeRecordFilterSelection(): void;
  normalizeAfterFilterWatch(): void;
  resetPageAfterFilterWatch(): void;
  shouldSkipFilterWatch(): boolean;
  shouldSkipPageWatch(): boolean;
  shouldSkipPrefsWatch(): boolean;
  shouldSkipColumnWatch(): boolean;
  formatError(error: unknown): string;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
};

export function createAppRuntime(deps: AppRuntimeDeps) {
  onMounted(async () => {
    await bootstrap();
  });

  onBeforeUnmount(() => {
    deps.clearCapturePolling();
    deps.clearDiagnosticPolling();
    deps.disposeChart();
  });

  watch(deps.chartEl, async (element) => {
    if (!element) {
      deps.disposeChart();
      return;
    }
    await nextTick();
    deps.renderChart();
  });
  watch(deps.detail, async () => {
    await nextTick();
    deps.renderChart();
  }, { deep: true });
  watch(deps.uiLocale, async () => {
    await nextTick();
    deps.renderChart();
  });
  watch(() => deps.selectedDashboardScope.value, () => {
    deps.rankingDialogOpen.value = false;
  }, { deep: true });
  watch([
    deps.recordPoolKind,
    deps.recordBannerIds,
    deps.itemRarities,
    deps.focusedRarities,
    deps.rateUpResults,
    deps.rollBuckets,
    deps.itemKinds,
    deps.forkResultMarks,
    deps.forkPityBadges,
    deps.dateFrom,
    deps.dateTo,
    deps.search,
    deps.sortDirection,
    deps.pageSize,
  ], () => {
    if (deps.shouldSkipFilterWatch()) return;
    deps.normalizeAfterFilterWatch();
    deps.resetPageAfterFilterWatch();
    deps.saveRecordViewPrefs();
    void loadRecords();
  }, { flush: "sync" });
  watch(deps.pageIndex, () => {
    if (deps.shouldSkipPageWatch()) return;
    void loadRecords();
  }, { flush: "sync" });
  watch(deps.recordAdvancedFiltersOpen, () => {
    if (deps.shouldSkipPrefsWatch()) return;
    deps.saveRecordViewPrefs();
  }, { flush: "sync" });
  watch(deps.visibleRecordColumns, () => {
    if (deps.shouldSkipColumnWatch()) return;
    deps.normalizeAfterFilterWatch();
    deps.saveRecordViewPrefs();
  }, { flush: "sync" });

  async function bootstrap() {
    deps.busy.value = true;
    try {
      const [settings, maps, uiLocaleList] = await Promise.all([
        api.getSettings(),
        api.mapsList(),
        api.uiLocaleList(),
      ]);
      deps.applySettings(settings);
      deps.locales.value = maps.locales;
      deps.uiLocales.value = uiLocaleList.locales;
      await deps.loadProfiles();
      await refreshAll();
      await deps.loadUpdaterStatus();
      const startedPendingCapture = await deps.startPendingAdminCapture();
      const startedPendingDiagnostic = startedPendingCapture
        ? false
        : await deps.startPendingAdminDiagnostic();
      if (!startedPendingCapture && !startedPendingDiagnostic && settings.check_updates_on_startup) {
        void deps.checkForUpdates({ silent: true });
      }
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.busy.value = false;
    }
  }

  async function refreshAll() {
    if (!deps.activeProfileName.value) return;
    const requestedScope = deps.selectedDashboardScope.value;
    const view = await api.profileAnalysisView(
      deps.activeProfileName.value,
      requestedScope,
      deps.currentRecordFilter(),
      deps.locale.value,
    );
    deps.summary.value = view.overview;
    deps.detail.value = view.selected_detail;
    deps.detailLoading.value = false;
    deps.filterOptions.value = view.record_filter_options;
    deps.records.value = view.record_page.records;
    deps.recordTotal.value = view.record_page.total;
    const firstActive = deps.summary.value.pool_kinds.find((pool) => pool.total_pulls > 0)?.pool_kind;
    deps.normalizeDashboardScope(firstActive);
    const scopeChanged = !isSameDashboardScope(requestedScope, deps.selectedDashboardScope.value);
    const beforeFilter = deps.recordFilterKey(deps.currentRecordFilter());
    if (!deps.isRecordPrefsReady()) {
      deps.applyRecordViewPrefs();
    } else {
      deps.normalizeRecordFilterSelection();
    }
    const filterChanged = deps.recordFilterKey(deps.currentRecordFilter()) !== beforeFilter;
    await Promise.all([
      scopeChanged ? deps.loadDetail() : Promise.resolve(),
      filterChanged ? loadRecords() : Promise.resolve(),
    ]);
    deps.statusText.value = deps.t("status.dashboardUpdated");
    await deps.resolveVisibleAssets();
    await nextTick();
    deps.renderChart();
  }

  async function loadFilterOptions() {
    if (!deps.activeProfileName.value) return;
    deps.filterOptions.value = await api.recordFilterOptions(
      deps.activeProfileName.value,
      deps.locale.value,
    );
  }

  async function loadRecords() {
    if (!deps.activeProfileName.value) return;
    const result = await api.recordPage(
      deps.activeProfileName.value,
      deps.currentRecordFilter(),
      deps.locale.value,
    );
    deps.records.value = result.records;
    deps.recordTotal.value = result.total;
    await deps.resolveVisibleAssets();
  }

  return {
    bootstrap,
    refreshAll,
    loadFilterOptions,
    loadRecords,
  };
}

function isSameDashboardScope(left: DashboardSelection, right: DashboardSelection) {
  if (left.kind !== right.kind || left.pool_kind !== right.pool_kind) return false;
  if (left.kind === "pool_kind" || right.kind === "pool_kind") return true;
  return left.banner_id === right.banner_id;
}
