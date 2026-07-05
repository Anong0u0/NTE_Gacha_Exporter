export type WinDivertInstallStatus = {
  platform_supported: boolean;
  installed: boolean;
  version: string;
  install_dir: string;
  dll_path: string;
  sys_path: string;
  license_path: string;
  download_url: string;
  zip_sha256: string;
  loadable: boolean;
  error?: string | null;
};

export type WinDivertInstallReport = {
  status: WinDivertInstallStatus;
  downloaded: boolean;
  verified_sha256: string;
  installed_files: string[];
};
