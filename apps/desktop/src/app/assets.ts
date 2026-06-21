import type { ComputedRef, Ref } from "vue";
import { api, type AssetResolveRequest, type BannerSummary, type DashboardSelectionDetail, type DisplayRecord } from "../api";

type AssetRefEntry = { key: string; kind: string; value: unknown };

type AssetToolsOptions = {
  assetUrlCache: Ref<Record<string, string>>;
  bannerSummaries: ComputedRef<BannerSummary[]>;
  records: Ref<DisplayRecord[]>;
  detail: Ref<DashboardSelectionDetail | null>;
  errorText: Ref<string>;
  formatError(error: unknown): string;
};

export function createAssetTools(options: AssetToolsOptions) {
  const { assetUrlCache, bannerSummaries, records, detail, errorText, formatError } = options;

  function assetRefEntries(assetRefs?: Record<string, unknown> | null, preferredKeys: string[] = []): AssetRefEntry[] {
    if (!assetRefs) return [];
    const all = Object.entries(assetRefs);
    const preferred = preferredKeys
      .map((key) => all.find(([candidate]) => candidate === key))
      .filter((entry): entry is [string, unknown] => Boolean(entry));
    const rest = all.filter(([key]) => !preferredKeys.includes(key));
    return [...preferred, ...rest].flatMap(([key, value]) => {
      if (Array.isArray(value)) return value.map((item, index) => ({ key: `${key}[${index}]`, kind: key, value: item }));
      return [{ key, kind: key, value }];
    });
  }

  function assetCacheKey(assetRef: string, kind?: string | null) {
    return `${kind ?? ""}\0${assetRef}`;
  }

  function cachedAssetUrl(assetRef: string, kind?: string | null) {
    return assetUrlCache.value[assetCacheKey(assetRef, kind)] ?? assetUrlCache.value[assetCacheKey(assetRef, null)];
  }

  function assetRefsCount(assetRefs?: Record<string, unknown> | null) {
    return assetRefEntries(assetRefs).length;
  }

  function firstAssetUrl(assetRefs?: Record<string, unknown> | null, preferredKeys: string[] = []) {
    for (const entry of assetRefEntries(assetRefs, preferredKeys)) {
      if (typeof entry.value !== "string") continue;
      const url = cachedAssetUrl(entry.value, entry.kind);
      if (url) return url;
    }
    return "";
  }

  function itemVisualUrl(record?: DisplayRecord | null) {
    return record ? firstAssetUrl(record.item_asset_refs, ["portrait", "icon", "head_icon"]) : "";
  }

  function bannerVisualUrl(banner?: BannerSummary | DisplayRecord["banner"] | null) {
    return firstAssetUrl(banner?.asset_refs, ["image", "background", "banner", "icon"]);
  }

  function hasRecordVisual(record?: DisplayRecord | null) {
    return Boolean(itemVisualUrl(record));
  }

  function hasBannerVisual(banner?: BannerSummary | DisplayRecord["banner"] | null) {
    return Boolean(bannerVisualUrl(banner));
  }

  function recordsHaveAnyVisual() {
    return records.value.some(hasRecordVisual);
  }

  function collectAssetRequestsFromRefs(assetRefs?: Record<string, unknown> | null) {
    return assetRefEntries(assetRefs).flatMap((entry): AssetResolveRequest[] =>
      typeof entry.value === "string" ? [{ asset_ref: entry.value, kind: entry.kind }] : [],
    );
  }

  function collectRecordAssetRequests(record: DisplayRecord) {
    return [
      ...collectAssetRequestsFromRefs(record.item_asset_refs),
      ...collectAssetRequestsFromRefs(record.secondary_item_asset_refs),
      ...collectAssetRequestsFromRefs(record.banner.asset_refs),
    ];
  }

  function collectVisibleAssetRequests() {
    const requests: AssetResolveRequest[] = [];
    for (const banner of bannerSummaries.value) requests.push(...collectAssetRequestsFromRefs(banner.asset_refs));
    for (const record of records.value) requests.push(...collectRecordAssetRequests(record));
    for (const hit of detail.value?.five_star_history ?? []) requests.push(...collectRecordAssetRequests(hit.record));
    for (const hit of detail.value?.four_star_history ?? []) requests.push(...collectRecordAssetRequests(hit.record));
    const seen = new Set<string>();
    return requests.filter((request) => {
      const key = assetCacheKey(request.asset_ref, request.kind);
      if (seen.has(key) || assetUrlCache.value[key]) return false;
      seen.add(key);
      return true;
    });
  }

  async function resolveVisibleAssets() {
    const requests = collectVisibleAssetRequests();
    if (!requests.length) return;
    try {
      const resolved = await api.assetsResolveRefs(requests);
      const next = { ...assetUrlCache.value };
      for (const item of resolved) {
        if (item.url) next[assetCacheKey(item.asset_ref, item.kind)] = item.url;
      }
      assetUrlCache.value = next;
    } catch (error) {
      if (!errorText.value) errorText.value = formatError(error);
    }
  }

  return {
    assetRefsCount,
    itemVisualUrl,
    bannerVisualUrl,
    hasRecordVisual,
    hasBannerVisual,
    recordsHaveAnyVisual,
    resolveVisibleAssets,
  };
}
