import { computed, type Ref } from "vue";
import type {
  AssetsPackStatus,
  CaptureMode,
  CaptureStatus,
  DashboardOverview,
  PoolKind,
  RecordFilterOptions,
  Profile,
} from "../api";
import { formatCaptureMode } from "./viewHelpers";

type PoolKindFilter = PoolKind | "all";

type ComputedDeps = {
  profiles: Ref<Profile[]>;
  activeProfileName: Ref<string>;
  summary: Ref<DashboardOverview | null>;
  selectedPoolKind: Ref<PoolKind>;
  selectedBannerId: Ref<string>;
  recordTotal: Ref<number>;
  filterOptions: Ref<RecordFilterOptions>;
  captureStatus: Ref<CaptureStatus | null>;
  captureMode: Ref<CaptureMode>;
  busy: Ref<boolean>;
  captureActionBusy: Ref<boolean>;
  assetsPackStatus: Ref<AssetsPackStatus | null>;
  recordPoolKind: Ref<PoolKindFilter>;
  pageSize: Ref<number>;
  pageIndex: Ref<number>;
};

export function createAppComputed(deps: ComputedDeps) {
  const activeProfile = computed(() =>
    deps.profiles.value.find((profile) => profile.name === deps.activeProfileName.value),
  );
  const allPoolSummaries = computed(() => deps.summary.value?.pool_kinds ?? []);
  const trackedPoolCount = computed(
    () => allPoolSummaries.value.filter((pool) => pool.total_pulls > 0).length,
  );
  const bannerSummaries = computed(() => deps.summary.value?.banners ?? []);
  const trackedBannerCount = computed(
    () => bannerSummaries.value.filter((banner) => banner.total_pulls > 0).length,
  );
  const selectedSummary = computed(
    () => allPoolSummaries.value.find((item) => item.pool_kind === deps.selectedPoolKind.value) ?? null,
  );
  const selectedBanner = computed(() => {
    const byId = bannerSummaries.value.find((banner) => banner.banner_id === deps.selectedBannerId.value);
    if (byId) return byId;
    return (
      bannerSummaries.value.find(
        (banner) => banner.pool_kind === deps.selectedPoolKind.value && banner.total_pulls > 0,
      ) ??
      bannerSummaries.value[0] ??
      null
    );
  });
  const latest = computed(() => deps.summary.value?.latest_records ?? []);
  const phaseSummaries = computed(() => deps.summary.value?.time_stats.phases ?? []);
  const recordPageStart = computed(() =>
    deps.recordTotal.value === 0 ? 0 : deps.pageIndex.value * deps.pageSize.value + 1,
  );
  const recordPageEnd = computed(() =>
    Math.min(deps.recordTotal.value, (deps.pageIndex.value + 1) * deps.pageSize.value),
  );
  const canPrevPage = computed(() => deps.pageIndex.value > 0);
  const canNextPage = computed(() => recordPageEnd.value < deps.recordTotal.value);
  const poolsForRecordKind = computed(() =>
    deps.filterOptions.value.pools.filter(
      (pool) => deps.recordPoolKind.value === "all" || pool.pool_kind === deps.recordPoolKind.value,
    ),
  );
  const bannersForRecordKind = computed(() =>
    deps.filterOptions.value.banners.filter(
      (banner) => deps.recordPoolKind.value === "all" || banner.pool_kind === deps.recordPoolKind.value,
    ),
  );
  const isCaptureActive = computed(() => {
    const state = deps.captureStatus.value?.state;
    return state === "starting" || state === "running" || state === "stopping";
  });
  const isWorkflowBusy = computed(() => deps.busy.value || isCaptureActive.value || deps.captureActionBusy.value);
  const captureTitle = computed(() => {
    if (!deps.captureStatus.value) {
      return deps.summary.value?.total_records ? "Merge new records" : "Import records to start tracking";
    }
    if (deps.captureStatus.value.state === "completed") return "Capture completed";
    if (deps.captureStatus.value.state === "failed") return "Capture failed";
    if (deps.captureStatus.value.state === "stopping") return "Stopping capture";
    return "Capture running";
  });
  const captureSubtitle = computed(() => {
    if (!deps.captureStatus.value) {
      return deps.summary.value?.last_run
        ? `${deps.summary.value.last_run.records_inserted} inserted, ${deps.summary.value.last_run.records_skipped} skipped`
        : "Live capture, raw replay, and public JSON merge into this profile.";
    }
    if (deps.captureStatus.value.import_report) {
      return `${deps.captureStatus.value.import_report.records_inserted} inserted, ${deps.captureStatus.value.import_report.records_skipped} skipped`;
    }
    if (deps.captureStatus.value.error) return deps.captureStatus.value.error.message;
    return `${deps.captureStatus.value.records_count} records seen`;
  });
  const autoPageStatusLine = computed(() => {
    const auto = deps.captureStatus.value?.auto_page;
    if (!auto) return "";
    const page =
      auto.current_page && auto.total_pages ? ` page=${auto.current_page}/${auto.total_pages}` : "";
    const pool = auto.pool ? ` ${auto.pool}` : "";
    return `${auto.message}${pool}${page}`;
  });
  const captureModeLabel = computed(() =>
    formatCaptureMode(deps.captureStatus.value?.mode ?? deps.captureMode.value),
  );
  const assetsPackSummary = computed(() => {
    if (!deps.assetsPackStatus.value?.installed) return "Not installed";
    if (!deps.assetsPackStatus.value.compatible) return "Installed pack does not match this build";
    return `${deps.assetsPackStatus.value.file_count} images installed`;
  });

  return {
    activeProfile,
    allPoolSummaries,
    trackedPoolCount,
    bannerSummaries,
    trackedBannerCount,
    selectedSummary,
    selectedBanner,
    latest,
    phaseSummaries,
    recordPageStart,
    recordPageEnd,
    canPrevPage,
    canNextPage,
    poolsForRecordKind,
    bannersForRecordKind,
    isCaptureActive,
    isWorkflowBusy,
    captureTitle,
    captureSubtitle,
    autoPageStatusLine,
    captureModeLabel,
    assetsPackSummary,
  };
}
