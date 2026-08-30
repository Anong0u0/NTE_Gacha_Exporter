import type { CaptureMode, ImportReport } from "./base";

type CaptureState = "starting" | "running" | "stopping" | "completed" | "partial" | "failed" | "cancelled";

type CaptureCounters = {
  packets_seen: number;
  decoded_packets: number;
  dropped_packets: number;
  duplicate_packets?: number;
  filter_restarts?: number;
};

type CaptureTarget = {
  pid?: string | number;
  interface?: string;
  ports?: number[];
  bpf?: string;
  capture_strategy?: string;
  strategy_reason?: string;
  pppoe_detection?: Record<string, unknown>;
  attempts?: CaptureAttemptSummary[];
};

type CaptureAttemptSummary = {
  attempt_index: number;
  capture_strategy: string;
  strategy_reason: string;
  started_at: number;
  ended_at: number;
  counters: CaptureCounters;
};

type AutoPageReason = {
  code: string;
  message: string;
};

export type CapturePartialReason = AutoPageReason & {
  code:
    | "user_stopped"
    | "capture_incomplete"
    | "capture_stopped_during_drain"
    | "auto_page_capture_window_stalled"
    | "automation_interrupted";
  retryable_with_slower_paging: boolean;
  support_path?: string | null;
  support_image_path?: string | null;
};

type AutoPageStatusFields = {
  completed_pools?: string[];
  skipped_pools?: string[];
  visited_pages?: number;
  visited_pages_by_pool?: Record<string, number>;
  required_pages_by_pool?: Record<string, number>;
  decoded_pages_by_pool?: Record<string, number>;
  missing_pages_by_pool?: Record<string, number>;
  last_page_by_pool?: Record<string, number>;
};

type AutoPageTerminalFields = AutoPageStatusFields & {
  message: string;
  capture_drain_warning: AutoPageReason | null;
  completed_pools: string[];
  skipped_pools: string[];
  visited_pages: number;
  visited_pages_by_pool: Record<string, number>;
  required_pages_by_pool: Record<string, number>;
  decoded_pages_by_pool: Record<string, number>;
  missing_pages_by_pool: Record<string, number>;
  last_page_by_pool: Record<string, number>;
};

type AutoPageRunningStatus = AutoPageStatusFields & {
  state: "running";
  message: string;
  kind: string;
  step: string | null;
  pool: string | null;
  current_page: number | null;
  total_pages: number | null;
  technical_detail: string;
  elapsed_seconds: number;
};

type AutoPageDrainingStatus = AutoPageStatusFields & {
  state: "draining";
  message: string;
  kind: "draining";
  required_pages_by_pool: Record<string, number>;
  decoded_pages_by_pool: Record<string, number>;
  missing_pages_by_pool: Record<string, number>;
  last_decoded_at: number | null;
};

type AutoPageCompletedStatus = AutoPageTerminalFields & {
  state: "completed";
  reason: null;
};

type AutoPagePartialStatus = AutoPageTerminalFields & {
  state: "partial";
  reason: CapturePartialReason;
};

type AutoPageFailedStatus = AutoPageTerminalFields & {
  state: "failed" | "cancelled";
  reason: AutoPageReason | null;
};

type AutoPageTerminalStatus =
  | AutoPageCompletedStatus
  | AutoPagePartialStatus
  | AutoPageFailedStatus;

export type AutoPageStatus =
  | AutoPageRunningStatus
  | AutoPageDrainingStatus
  | AutoPageTerminalStatus;

export type CaptureStatus = {
  session_id: string;
  state: CaptureState;
  profile_name: string;
  locale: string;
  mode: CaptureMode;
  records_count: number;
  latest_records: Record<string, unknown>[];
  counters: CaptureCounters;
  attempts?: CaptureAttemptSummary[];
  started_at: number;
  updated_at: number;
  target?: CaptureTarget | null;
  auto_page: AutoPageStatus | null;
  raw_path?: string | null;
  error?: {
    code: string;
    message: string;
    support_path?: string | null;
    support_image_path?: string | null;
  } | null;
  import_report?: ImportReport | null;
};

export type CaptureStartOptions = {
  page_record_min_wait_ms?: number;
  capture_backend?: "pktmon" | "windivert";
};

export type PendingAdminCapture = {
  profile_name: string;
  locale: string;
  mode: CaptureMode;
  options?: CaptureStartOptions;
};
