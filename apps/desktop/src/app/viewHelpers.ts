import type { BannerSummary, DisplayRecord, ForkResultMark, ItemKind, PityBadge, RollBucket } from "../api";
import type { I18nKey } from "./i18n";

type Translator = (key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>) => string;

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

export function formatQuantityName(name: string, count?: number | null) {
  const quantity = count ?? 1;
  return Number.isFinite(quantity) && quantity > 1 ? `${name} x${quantity}` : name;
}

export function formatResult(value: string, t: Translator) {
  if (value === "up") return "UP";
  if (value === "not_applicable") return t("format.notApplicable");
  if (value === "unknown") return t("common.unknown");
  if (value === "off_rate") return t("format.offRate");
  return "";
}

export function formatRecordResultBadge(value: string, t: Translator) {
  if (value === "unknown") return "";
  if (value === "not_applicable") return t("records.nonUpFiveStar");
  return formatResult(value, t);
}

export function primaryRecordBadge(record: DisplayRecord, t: Translator) {
  const forkBadge = forkHitBadge(record);
  if (forkBadge) return forkBadge;
  return formatRecordResultBadge(record.derived.rate_up_result, t);
}

export function isHitBadgeLabel(value: string) {
  return value === "W" || value === "G" || value === "L";
}

export function formatRollBucket(value: RollBucket, t: Translator) {
  if (value === "gift") return t("records.rollGift");
  if (value === "sleep") return t("records.rollSleep");
  if (value === "not_applicable") return t("records.rollNotApplicable");
  if (["1", "2", "3", "4", "5", "6"].includes(value)) return value;
  return "";
}

export function formatItemKind(value: ItemKind, t: Translator) {
  if (value === "character") return t("records.itemKindCharacter");
  if (value === "fork") return t("records.itemKindFork");
  if (value === "appearance") return t("records.itemKindAppearance");
  if (value === "inventory") return t("records.itemKindInventory");
  if (value === "vehicle_module") return t("records.itemKindVehicleModule");
  if (value === "unknown") return t("records.itemKindUnknown");
  return "";
}

export function bannerTitle(banner: BannerSummary | DisplayRecord["banner"] | null | undefined, t: Translator) {
  return banner?.title || banner?.banner_id || `${t("common.unknown")} ${t("common.banner").toLowerCase()}`;
}

export function bannerMeta(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  const parts = [banner?.version].filter(Boolean);
  if (parts.length) return parts.join(" · ");
  return banner && "resolution_issue" in banner ? (banner.resolution_issue ?? "") : "";
}

export function formatBannerWindow(start: string | null | undefined, end: string | null | undefined, t: Translator) {
  if (!start && !end) return t("format.windowUnknown");
  return `${start ?? t("common.unknown").toLowerCase()} -> ${end ?? t("format.ongoing")}`;
}

export function formatPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? "-";
}

export function formatPoolKindPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_pool_kind ?? "-";
}

export function forkHitBadge(record: DisplayRecord) {
  if (record.fork_result_mark === "win") return "W";
  if (record.fork_result_mark === "guaranteed") return "G";
  if (record.fork_result_mark === "lose") return "L";
  return "";
}

export function formatForkResultMark(value: ForkResultMark, t: Translator) {
  if (value === "win") return t("records.forkResultWin");
  if (value === "guaranteed") return t("records.forkResultGuaranteed");
  if (value === "lose") return t("records.forkResultLose");
  return "";
}

export function forkWinRate(summary?: { fork_observed_25_75_win_rate?: number | null } | null) {
  return percent(summary?.fork_observed_25_75_win_rate);
}

export function formatPity(record: DisplayRecord) {
  if (!record.derived.counts_as_pull) return "-";
  return String(record.derived.pity_5_after);
}

export function formatTenPullProgress(record: DisplayRecord) {
  const progress = record.derived.ten_pull_progress_before;
  return progress == null ? "-" : String(progress);
}

export function formatTenPullProgressSummary(progress?: number | null) {
  return progress == null ? "-" : `${progress}/10`;
}

export function formatPityBadge(record: DisplayRecord, t: Translator) {
  const badge = record.derived.pity_badge;
  if (!badge) return "";
  return formatPityBadgeValue(badge, t);
}

export function formatRolls(record: DisplayRecord) {
  if (record.pool_kind === "fork_lottery") return "-";
  return record.roll_label ?? record.roll_points ?? "-";
}

export function formatPityBadgeValue(value: PityBadge, t: Translator) {
  if (value === "fork_up_guarantee") return t("records.pityBadgeForkUpGuarantee");
  if (value === "fork_5star_guarantee") return t("records.pityBadgeForkFiveStarGuarantee");
  if (value === "fork_4star_guarantee") return t("records.pityBadgeForkFourStarGuarantee");
  return "";
}

export function formatCaptureState(value: string | null | undefined, t: Translator) {
  if (!value) return "-";
  if (value === "starting") return t("capture.stateStarting");
  if (value === "running") return t("capture.stateRunning");
  if (value === "stopping") return t("capture.stateStopping");
  if (value === "completed") return t("capture.stateCompleted");
  if (value === "failed") return t("capture.stateFailed");
  return "";
}

export function formatCaptureMode(value: string | null | undefined, t: Translator) {
  if (value === "live_only") return t("capture.liveOnly");
  if (value === "auto_page_full") return t("capture.fullUpdate");
  if (value === "auto_page_incremental") return t("capture.autoPage");
  return "";
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
