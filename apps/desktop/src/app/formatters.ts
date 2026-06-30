import type { DisplayRecord } from "../api";
import { type I18nKey, uiLocaleDisplayName } from "./i18n";
import {
  bannerMeta,
  bannerTitle,
  captureRecordMeta,
  captureRecordName,
  forkHitBadge,
  forkWinRate,
  formatBannerWindow,
  formatCaptureMode,
  formatCaptureState,
  formatError,
  formatPity,
  formatPityBadge,
  formatPoolKindPullNo,
  formatPullNo,
  formatQuantityName,
  formatRecordResultBadge,
  formatResult,
  formatRolls,
  formatTenPullProgress,
  formatTenPullProgressSummary,
  formatTime,
  isHitBadgeLabel,
  numberOrDash,
  percent,
  primaryRecordBadge,
} from "./viewHelpers";

type Translator = (
  key: I18nKey,
  params?: Record<string, string | number | boolean | null | undefined>,
) => string;

export function createAppFormatters(t: Translator) {
  return {
    uiLocaleName: (value: string) => uiLocaleDisplayName(value),
    percent,
    numberOrDash,
    formatTime,
    formatResult: (value: string) => formatResult(value, t),
    bannerTitle: (banner?: Parameters<typeof bannerTitle>[0]) => bannerTitle(banner, t),
    bannerMeta: (banner?: Parameters<typeof bannerMeta>[0]) => bannerMeta(banner),
    formatBannerWindow: (start?: string | null, end?: string | null) =>
      formatBannerWindow(start, end, t),
    formatPullNo,
    formatPoolKindPullNo,
    formatPity: (record: DisplayRecord) => formatPity(record),
    formatTenPullProgress: (record: DisplayRecord) => formatTenPullProgress(record),
    formatTenPullProgressSummary,
    formatPityBadge: (record: DisplayRecord) => formatPityBadge(record, t),
    formatRolls,
    formatQuantityName,
    formatRecordResultBadge: (value: string) => formatRecordResultBadge(value, t),
    primaryRecordBadge: (record: DisplayRecord) => primaryRecordBadge(record, t),
    isHitBadgeLabel,
    forkHitBadge,
    forkWinRate,
    captureRecordName,
    captureRecordMeta,
    formatError,
    formatCaptureState: (value?: string | null) => formatCaptureState(value, t),
    formatCaptureMode: (value?: string | null) => formatCaptureMode(value, t),
  };
}
