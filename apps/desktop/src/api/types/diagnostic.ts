type DiagnosticState = "starting" | "running" | "stopping" | "completed" | "failed";

export type DiagnosticStatusSummary = {
  verdict: string;
  findings: string[];
  packets_seen: number;
  decoded_packets: number;
  dropped_packets: number;
  duplicate_packets: number;
  rows_count: number;
  external_ok: boolean;
};

export type DiagnosticStatus = {
  session_id: string;
  state: DiagnosticState;
  started_at: number;
  updated_at: number;
  duration_seconds: number;
  elapsed_seconds: number;
  stage: string;
  progress: number;
  support_zip_path?: string | null;
  error?: {
    code: string;
    message: string;
    support_path?: string | null;
    support_image_path?: string | null;
  } | null;
  summary?: DiagnosticStatusSummary | null;
};

export type PendingAdminDiagnostic = {
  duration_seconds: number;
};
