import type { CaptureMode, ImportReport } from "./base";

type CaptureState = "starting" | "running" | "stopping" | "completed" | "failed" | "cancelled";

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

type AutoPageStatus = {
  state: string;
  message: string;
  kind: string;
  step?: string | null;
  pool?: string | null;
  current_page?: number | null;
  total_pages?: number | null;
  technical_detail?: string | null;
  completed_pools?: string[];
  skipped_pools?: string[];
};

export type CaptureStatus = {
  session_id: string;
  state: CaptureState;
  mode: CaptureMode;
  records_count: number;
  latest_records: Record<string, unknown>[];
  counters: CaptureCounters;
  attempts?: CaptureAttemptSummary[];
  started_at: number;
  updated_at: number;
  target?: CaptureTarget | null;
  auto_page?: AutoPageStatus | null;
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
