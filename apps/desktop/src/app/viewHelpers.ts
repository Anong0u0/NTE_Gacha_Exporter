import type { BannerSummary, DisplayRecord } from "../api";

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

export function formatResult(value: string) {
  if (value === "not_applicable") return "N/A";
  if (value === "unknown") return "Unknown";
  return value === "off_rate" ? "Off-rate" : "UP";
}

export function bannerTitle(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  return banner?.title || banner?.banner_id || "Unknown banner";
}

export function bannerMeta(banner?: BannerSummary | DisplayRecord["banner"] | null) {
  const parts = [banner?.version, banner?.phase, banner?.source_confidence].filter(Boolean);
  if (parts.length) return parts.join(" · ");
  return banner && "status" in banner ? banner.status : "unknown";
}

export function formatBannerWindow(start?: string | null, end?: string | null) {
  if (!start && !end) return "window unknown";
  return `${start ?? "unknown"} -> ${end ?? "ongoing"}`;
}

export function formatPullNo(record: DisplayRecord) {
  return record.derived.pull_no_in_banner ?? record.derived.pull_no_in_pool_kind ?? "-";
}

export function formatPity(record: DisplayRecord) {
  return `5★ ${record.derived.pity_5_before}->${record.derived.pity_5_after} · 4★ ${record.derived.pity_4_before}->${record.derived.pity_4_after}`;
}

export function formatGuarantee(record: DisplayRecord) {
  const before = record.derived.guarantee_5_before ? "G before" : "normal";
  const after = record.derived.guarantee_5_after ? "G after" : "normal";
  return `${before} / ${after}`;
}

export function formatCaptureState(value?: string | null) {
  if (!value) return "-";
  if (value === "starting") return "Starting";
  if (value === "running") return "Running";
  if (value === "stopping") return "Stopping";
  if (value === "completed") return "Completed";
  if (value === "failed") return "Failed";
  return value;
}

export function formatCaptureMode(value?: string | null) {
  if (value === "live_only") return "Live only";
  if (value === "auto_page_full") return "Full update";
  return "Auto-page";
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
