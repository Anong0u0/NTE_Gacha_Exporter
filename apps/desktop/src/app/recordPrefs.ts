import type { ForkResultMark, ItemKind, PityBadge, PoolKind, RateUpResult, RollBucket, SortDirection } from "../api";
import { kindOrder, type PoolKindFilter } from "./options";

export const recordPageSizes = [5, 10, 20, 50, 100] as const;
export type RecordPageSize = (typeof recordPageSizes)[number];

export const recordColumnIds = ["index", "time", "banner", "item", "rarity", "pullNo", "fiveStarProgress", "tenPullProgress", "rolls"] as const;
export type RecordColumnId = (typeof recordColumnIds)[number];

export const recordColumnGridTracks: Record<RecordColumnId, string> = {
  index: "minmax(48px, 0.55fr)",
  time: "minmax(124px, 1.05fr)",
  banner: "minmax(136px, 1.15fr)",
  item: "minmax(170px, 1.35fr)",
  rarity: "minmax(40px, 0.45fr)",
  pullNo: "minmax(44px, 0.5fr)",
  fiveStarProgress: "minmax(72px, 0.75fr)",
  tenPullProgress: "minmax(76px, 0.8fr)",
  rolls: "minmax(48px, 0.55fr)",
};

export type RecordViewPrefs = {
  recordPoolKind: PoolKindFilter;
  recordBannerIds: string[];
  itemRarities: number[];
  focusedRarities: number[];
  rateUpResults: RateUpResult[];
  rollBuckets: RollBucket[];
  itemKinds: ItemKind[];
  forkResultMarks: ForkResultMark[];
  forkPityBadges: PityBadge[];
  dateFrom: string;
  dateTo: string;
  search: string;
  sortDirection: SortDirection;
  pageSize: RecordPageSize;
  visibleRecordColumns: RecordColumnId[];
  recordAdvancedFiltersOpen: boolean;
  latestFiveStarWallModes: Record<PoolKind, FiveStarWallMode>;
};

export type FiveStarWallMode = "all" | "focused";

export const defaultRecordViewPrefs: RecordViewPrefs = {
  recordPoolKind: "all",
  recordBannerIds: [],
  itemRarities: [],
  focusedRarities: [],
  rateUpResults: [],
  rollBuckets: [],
  itemKinds: [],
  forkResultMarks: [],
  forkPityBadges: [],
  dateFrom: "",
  dateTo: "",
  search: "",
  sortDirection: "desc",
  pageSize: 10,
  visibleRecordColumns: [...recordColumnIds],
  recordAdvancedFiltersOpen: false,
  latestFiveStarWallModes: {
    monopoly_limited: "all",
    monopoly_standard: "all",
    fork_lottery: "focused",
  },
};

export const rateUpResultOptions: RateUpResult[] = ["up", "off_rate", "not_applicable", "unknown"];
export const forkResultMarkOptions: ForkResultMark[] = ["win", "guaranteed", "lose"];
export const forkPityBadgeOptions: PityBadge[] = ["fork_up_guarantee", "fork_5star_guarantee", "fork_4star_guarantee"];

export function recordPrefsKey(profileName: string) {
  return `nte.recordView.v3:${profileName}`;
}

export function readRecordViewPrefs(profileName: string): RecordViewPrefs {
  if (!profileName) return { ...defaultRecordViewPrefs };
  try {
    const raw = window.localStorage.getItem(recordPrefsKey(profileName));
    if (!raw) return { ...defaultRecordViewPrefs };
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return { ...defaultRecordViewPrefs };
    const source = parsed as Record<string, unknown>;
    const recordPoolKind = source.recordPoolKind;
    const sortDirection = source.sortDirection;
    const pageSize = source.pageSize;
    return {
      recordPoolKind: recordPoolKind === "all" || kindOrder.includes(recordPoolKind as PoolKind) ? (recordPoolKind as PoolKindFilter) : defaultRecordViewPrefs.recordPoolKind,
      recordBannerIds: readStringArray(source.recordBannerIds),
      itemRarities: readNumberArray(source.itemRarities).filter(isRecordRarity),
      focusedRarities: readNumberArray(source.focusedRarities).filter(isRecordRarity),
      rateUpResults: readStringArray(source.rateUpResults).filter((result): result is RateUpResult => rateUpResultOptions.includes(result as RateUpResult)),
      rollBuckets: readStringArray(source.rollBuckets).filter((bucket): bucket is RollBucket => isRollBucket(bucket)),
      itemKinds: readStringArray(source.itemKinds).filter((itemKind): itemKind is ItemKind => isItemKind(itemKind)),
      forkResultMarks: readStringArray(source.forkResultMarks).filter((mark): mark is ForkResultMark => forkResultMarkOptions.includes(mark as ForkResultMark)),
      forkPityBadges: readStringArray(source.forkPityBadges).filter((badge): badge is PityBadge => forkPityBadgeOptions.includes(badge as PityBadge)),
      dateFrom: readString(source.dateFrom),
      dateTo: readString(source.dateTo),
      search: readString(source.search),
      sortDirection: sortDirection === "asc" || sortDirection === "desc" ? sortDirection : defaultRecordViewPrefs.sortDirection,
      pageSize: recordPageSizes.includes(pageSize as RecordPageSize) ? (pageSize as RecordPageSize) : defaultRecordViewPrefs.pageSize,
      visibleRecordColumns: readRecordColumnArray(source.visibleRecordColumns),
      recordAdvancedFiltersOpen: typeof source.recordAdvancedFiltersOpen === "boolean" ? source.recordAdvancedFiltersOpen : defaultRecordViewPrefs.recordAdvancedFiltersOpen,
      latestFiveStarWallModes: readFiveStarWallModes(source.latestFiveStarWallModes),
    };
  } catch {
    return { ...defaultRecordViewPrefs };
  }
}

export function isRecordRarity(value: number) {
  return value === 3 || value === 4 || value === 5;
}

function readString(value: unknown) {
  return typeof value === "string" ? value : "";
}

function readStringArray(value: unknown) {
  return Array.isArray(value) ? value.filter((item): item is string => typeof item === "string") : [];
}

function readNumberArray(value: unknown) {
  return Array.isArray(value) ? value.filter((item): item is number => Number.isInteger(item)) : [];
}

function isRollBucket(value: string): value is RollBucket {
  return value === "gift" || value === "sleep" || value === "1" || value === "2" || value === "3" || value === "4" || value === "5" || value === "6" || value === "not_applicable";
}

function isItemKind(value: string): value is ItemKind {
  return value === "character" || value === "fork" || value === "appearance" || value === "inventory" || value === "vehicle_module" || value === "unknown";
}

function readRecordColumnArray(value: unknown): RecordColumnId[] {
  if (!Array.isArray(value)) return [...defaultRecordViewPrefs.visibleRecordColumns];
  const columns = value.filter((item): item is RecordColumnId => isRecordColumnId(item));
  return recordColumnIds.filter((column) => columns.includes(column));
}

function isRecordColumnId(value: unknown): value is RecordColumnId {
  return typeof value === "string" && recordColumnIds.includes(value as RecordColumnId);
}

function readFiveStarWallModes(value: unknown): Record<PoolKind, FiveStarWallMode> {
  const result = { ...defaultRecordViewPrefs.latestFiveStarWallModes };
  if (typeof value !== "object" || value === null) return result;
  const source = value as Record<string, unknown>;
  for (const kind of kindOrder) {
    const mode = source[kind];
    if (mode === "all" || mode === "focused") result[kind] = mode;
  }
  return result;
}
