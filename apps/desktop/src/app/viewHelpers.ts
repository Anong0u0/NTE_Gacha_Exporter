import type { BannerSummary, DisplayRecord } from "../api";
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

export function parseOptionalNumber(value: string) {
  const trimmed = value.trim();
  if (!trimmed) return null;
  const parsed = Number(trimmed);
  return Number.isFinite(parsed) ? parsed : null;
}

export function formatTime(value?: string | null) {
  return value?.replace("T", " ").replace("Z", "") ?? "-";
}

export function formatResult(value: string, t: Translator = english) {
  if (value === "not_applicable") return t("format.notApplicable");
  if (value === "unknown") return t("common.unknown");
  return value === "off_rate" ? t("format.offRate") : "UP";
}

export function bannerTitle(banner?: BannerSummary | DisplayRecord["banner"] | null, t: Translator = english) {
  return banner?.title || banner?.banner_id || `${t("common.unknown")} ${t("common.banner").toLowerCase()}`;
}

export function bannerMeta(banner?: BannerSummary | DisplayRecord["banner"] | null, t: Translator = english) {
  const parts = [banner?.version].filter(Boolean);
  if (parts.length) return parts.join(" · ");
  return banner && "status" in banner ? banner.status : t("common.unknown").toLowerCase();
}

export function formatBannerWindow(start?: string | null, end?: string | null, t: Translator = english) {
  if (!start && !end) return t("format.windowUnknown");
  return `${start ?? t("common.unknown").toLowerCase()} -> ${end ?? t("format.ongoing")}`;
}

export function formatPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? "-";
}

export function formatPity(record: DisplayRecord) {
  return `5★ ${record.derived.pity_5_before}->${record.derived.pity_5_after} · 4★ ${record.derived.pity_4_before}->${record.derived.pity_4_after}`;
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
