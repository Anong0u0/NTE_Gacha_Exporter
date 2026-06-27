import type { CaptureMode, ImportReport } from "./base";

type CaptureState = "starting" | "running" | "stopping" | "completed" | "failed";

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

export type PendingAdminCapture = {
  profile_name: string;
  locale: string;
  mode: CaptureMode;
};
