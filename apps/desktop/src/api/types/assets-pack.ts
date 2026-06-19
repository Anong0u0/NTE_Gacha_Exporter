import type { UpdateChannel } from "./update";

export type AssetsPackStatus = {
  installed: boolean;
  compatible: boolean;
  current_app_version: string;
  expected_map_hash: string;
  installed_app_version?: string | null;
  installed_map_hash?: string | null;
  source_commit?: string | null;
  file_count: number;
  install_path: string;
};

export type AssetsPackPackage = {
  app_version: string;
  map_hash: string;
  release_url: string;
  asset_name: string;
  download_url: string;
  manifest_name: string;
  manifest_url: string;
  sha256: string;
  size: number;
  source_commit: string;
  file_count: number;
};

export type AssetsPackCheckReport = {
  current_app_version: string;
  expected_map_hash: string;
  channel: UpdateChannel;
  installed: boolean;
  compatible: boolean;
  package?: AssetsPackPackage | null;
};

export type AssetsPackInstallReport = {
  app_version: string;
  map_hash: string;
  source_commit: string;
  file_count: number;
  install_path: string;
};

export type AssetResolveRequest = {
  asset_ref: string;
  kind?: string | null;
};

export type AssetResolveResult = {
  asset_ref: string;
  kind?: string | null;
  url?: string | null;
};
