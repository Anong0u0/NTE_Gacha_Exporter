type UpdateChannel = "stable" | "beta";

export type UpdatePackage = {
  version: string;
  channel: UpdateChannel;
  release_url: string;
  asset_name: string;
  download_url: string;
  sha256: string;
  size: number;
};

export type UpdateStatus = {
  portable_root: string;
  current_version: string;
  supported_layout: boolean;
  staged_version?: string | null;
  rollback_version?: string | null;
};

export type UpdateCheckReport = {
  current_version: string;
  channel: UpdateChannel;
  available: boolean;
  release_notes: string;
  package?: UpdatePackage | null;
};

export type UpdateStageReport = {
  package: UpdatePackage;
  archive_path: string;
  staging_path: string;
};
