import type { Ref } from "vue";

import {
  api,
  type BannerSummary,
  type DashboardOverview,
  type DashboardSelection,
  type DashboardSelectionDetail,
  type PoolKind,
} from "../api";

type DashboardActionDeps = {
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  summary: Ref<DashboardOverview | null>;
  selectedPoolKind: Ref<PoolKind>;
  selectedDashboardScope: Ref<DashboardSelection>;
  detail: Ref<DashboardSelectionDetail | null>;
  detailLoading: Ref<boolean>;
  errorText: Ref<string>;
  formatError(error: unknown): string;
  resolveVisibleAssets(): Promise<void>;
};

export function createDashboardActions(deps: DashboardActionDeps) {
  let detailLoadId = 0;

  function normalizeDashboardScope(fallbackPoolKind?: PoolKind) {
    const scope = deps.selectedDashboardScope.value;
    if (scope.kind === "banner") {
      const banner = deps.summary.value?.banners.find((item) => item.banner_id === scope.banner_id);
      if (banner) {
        deps.selectedPoolKind.value = banner.pool_kind;
        deps.selectedDashboardScope.value = { kind: "banner", pool_kind: banner.pool_kind, banner_id: banner.banner_id };
        return;
      }
    }
    const poolKind = deps.summary.value?.pool_kinds.some((pool) => pool.pool_kind === scope.pool_kind)
      ? scope.pool_kind
      : (fallbackPoolKind ?? deps.selectedPoolKind.value);
    deps.selectedPoolKind.value = poolKind;
    deps.selectedDashboardScope.value = { kind: "pool_kind", pool_kind: poolKind };
  }

  function selectDashboardPool(poolKind: PoolKind) {
    deps.selectedPoolKind.value = poolKind;
    deps.selectedDashboardScope.value = { kind: "pool_kind", pool_kind: poolKind };
    void loadDetail().catch(handleDetailError);
  }

  function selectDashboardBanner(banner: BannerSummary) {
    deps.selectedPoolKind.value = banner.pool_kind;
    deps.selectedDashboardScope.value = { kind: "banner", pool_kind: banner.pool_kind, banner_id: banner.banner_id };
    void loadDetail().catch(handleDetailError);
  }

  function isSelectedDashboardPool(poolKind: PoolKind) {
    const scope = deps.selectedDashboardScope.value;
    return scope.pool_kind === poolKind;
  }

  function isSelectedDashboardBanner(bannerId: string) {
    const scope = deps.selectedDashboardScope.value;
    return scope.kind === "banner" && scope.banner_id === bannerId;
  }

  function isSameDashboardScope(left: DashboardSelection, right: DashboardSelection) {
    if (left.kind !== right.kind || left.pool_kind !== right.pool_kind) return false;
    if (left.kind === "pool_kind" || right.kind === "pool_kind") return true;
    return left.banner_id === right.banner_id;
  }

  function handleDetailError(error: unknown) {
    deps.errorText.value = deps.formatError(error);
  }

  async function loadDetail() {
    if (!deps.activeProfileName.value) {
      deps.detail.value = null;
      deps.detailLoading.value = false;
      return;
    }
    const requestId = ++detailLoadId;
    const profileName = deps.activeProfileName.value;
    const requestLocale = deps.locale.value;
    const requestScope = deps.selectedDashboardScope.value;
    deps.errorText.value = "";
    deps.detail.value = null;
    deps.detailLoading.value = true;
    try {
      const nextDetail = await api.dashboardSelectionDetail(profileName, requestScope, requestLocale);
      if (
        requestId !== detailLoadId
        || profileName !== deps.activeProfileName.value
        || requestLocale !== deps.locale.value
        || !isSameDashboardScope(requestScope, deps.selectedDashboardScope.value)
      ) {
        return;
      }
      deps.detail.value = nextDetail;
      await deps.resolveVisibleAssets();
    } finally {
      if (
        requestId === detailLoadId
        && profileName === deps.activeProfileName.value
        && requestLocale === deps.locale.value
        && isSameDashboardScope(requestScope, deps.selectedDashboardScope.value)
      ) {
        deps.detailLoading.value = false;
      }
    }
  }

  return {
    normalizeDashboardScope,
    selectDashboardPool,
    selectDashboardBanner,
    isSelectedDashboardPool,
    isSelectedDashboardBanner,
    loadDetail,
  };
}
