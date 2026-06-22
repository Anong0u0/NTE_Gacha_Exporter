import type { BannerSummary, DisplayRecord, ItemKind, RollBucket } from "../api";
import type { I18nKey } from "./i18n";

type Translator = (key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>) => string;
const english: Translator = (key, params) => {
  const fallback: Record<string, string> = {
    "common.unknown": "Unknown",
    "format.notApplicable": "N/A",
    "format.offRate": "Off-rate",
    "format.windowUnknown": "window unknown",
    "format.ongoing": "ongoing",
    "format.guaranteeBefore": "G before",
    "format.guaranteeAfter": "G after",
    "records.rollGift": "Gift",
    "records.rollSleep": "Sleep",
    "records.rollNotApplicable": "N/A",
    "records.itemKindCharacter": "Character",
    "records.itemKindFork": "Fork",
    "records.itemKindAppearance": "Appearance",
    "records.itemKindInventory": "Inventory",
    "records.itemKindVehicleModule": "Vehicle module",
    "records.itemKindUnknown": "Unknown",
    "capture.stateStarting": "Starting",
    "capture.stateRunning": "Running",
    "capture.stateStopping": "Stopping",
    "capture.stateCompleted": "Completed",
    "capture.stateFailed": "Failed",
    "capture.liveOnly": "Live only",
    "capture.fullUpdate": "Full update",
    "capture.autoPage": "Auto-page",
  };
  return (fallback[key] ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(params?.[name] ?? ""));
};

export function percent(value?: number | null) {
  if (value === null || value === undefined) return "-";
  return `${(value * 100).toFixed(1)}%`;
}

export function numberOrDash(value?: number | null) {
  return value === null || value === undefined ? "-" : value.toFixed(1);
}

export function formatTime(value?: string | null) {
  return value?.replace("T", " ").replace("Z", "") ?? "-";
}

export function formatResult(value: string, t: Translator = english) {
  if (value === "not_applicable") return t("format.notApplicable");
  if (value === "unknown") return t("common.unknown");
  return value === "off_rate" ? t("format.offRate") : "UP";
}

export function formatRecordResultBadge(value: string, t: Translator = english) {
  if (value === "unknown") return "";
  if (value === "not_applicable") return t("records.nonUpFiveStar");
  return formatResult(value, t);
}

export function formatRollBucket(value: RollBucket, t: Translator = english) {
  if (value === "gift") return t("records.rollGift");
  if (value === "sleep") return t("records.rollSleep");
  if (value === "not_applicable") return t("records.rollNotApplicable");
  return value;
}

export function formatItemKind(value: ItemKind, t: Translator = english) {
  if (value === "character") return t("records.itemKindCharacter");
  if (value === "fork") return t("records.itemKindFork");
  if (value === "appearance") return t("records.itemKindAppearance");
  if (value === "inventory") return t("records.itemKindInventory");
  if (value === "vehicle_module") return t("records.itemKindVehicleModule");
  return t("records.itemKindUnknown");
}

export function bannerTitle(banner?: BannerSummary | DisplayRecord["banner"] | null, t: Translator = english) {
  return banner?.title || banner?.banner_id || `${t("common.unknown")} ${t("common.banner").toLowerCase()}`;
}

export function bannerMeta(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  const parts = [banner?.version].filter(Boolean);
  if (parts.length) return parts.join(" · ");
  return banner && "resolution_issue" in banner ? (banner.resolution_issue ?? "") : "";
}

export function formatBannerWindow(start?: string | null, end?: string | null, t: Translator = english) {
  if (!start && !end) return t("format.windowUnknown");
  return `${start ?? t("common.unknown").toLowerCase()} -> ${end ?? t("format.ongoing")}`;
}

export function formatPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? "-";
}

export function formatGlobalPullNo(record: DisplayRecord) {
  return record.derived.global_pull_no ?? "-";
}

export function forkHitBadge(record: DisplayRecord) {
  if (record.pool_kind !== "fork_lottery" || record.derived.hit_rarity !== 5) return "";
  if (record.derived.rate_up_result === "up" && record.derived.fork_forced_up) return "G";
  if (record.derived.rate_up_result === "up") return "W";
  if (record.derived.rate_up_result === "off_rate") return "L";
  return "";
}

export function forkWinRate(summary?: { fork_observed_25_75_win_rate?: number | null } | null) {
  return percent(summary?.fork_observed_25_75_win_rate);
}

export function formatPity(record: DisplayRecord) {
  if (!record.derived.counts_as_pull) return "-";
  return String(record.derived.pity_5_after);
}

export function formatRollGiftProgress(record: DisplayRecord) {
  const progress = record.derived.roll_gift_progress_after;
  return progress == null ? "-" : String(progress);
}

export function formatGuarantee(record: DisplayRecord, t: Translator = english) {
  const parts = [
    record.derived.guarantee_5_before ? t("format.guaranteeBefore") : "",
    record.derived.guarantee_5_after ? t("format.guaranteeAfter") : "",
  ].filter(Boolean);
  return parts.join(" / ");
}

export function formatCaptureState(value?: string | null, t: Translator = english) {
  if (!value) return "-";
  if (value === "starting") return t("capture.stateStarting");
  if (value === "running") return t("capture.stateRunning");
  if (value === "stopping") return t("capture.stateStopping");
  if (value === "completed") return t("capture.stateCompleted");
  if (value === "failed") return t("capture.stateFailed");
  return value;
}

export function formatCaptureMode(value?: string | null, t: Translator = english) {
  if (value === "live_only") return t("capture.liveOnly");
  if (value === "auto_page_full") return t("capture.fullUpdate");
  return t("capture.autoPage");
}

export function captureRecordName(record: Record<string, unknown>) {
  return String(record.item_name ?? record.item_id ?? "-");
}

export function captureRecordMeta(record: Record<string, unknown>) {
  return String(record.pool_name ?? record.pool_id ?? record.record_type ?? "-");
}

export function formatError(error: unknown) {
  if (typeof error === "object" && error !== null && "message" in error) {
    const apiError = error as { code?: string; message?: string };
    return apiError.code ? `${apiError.code}: ${apiError.message ?? ""}` : (apiError.message ?? String(error));
  }
  return error instanceof Error ? error.message : String(error);
}
