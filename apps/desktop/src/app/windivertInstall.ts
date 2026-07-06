import { api } from "../api";

export type WinDivertInstallStage =
  | "checking"
  | "downloading"
  | "verifying"
  | "installing"
  | "ready"
  | "failed";

type WinDivertInstallOptions = {
  onStage?: (stage: WinDivertInstallStage) => void | Promise<void>;
};

export async function ensureWinDivertInstalled(options: WinDivertInstallOptions = {}) {
  await setStage(options, "checking");
  const status = await api.windivertStatus(false);
  if (status.installed) {
    await setStage(options, "ready");
    return;
  }
  await installWinDivert(options);
}

export async function reinstallWinDivert(options: WinDivertInstallOptions = {}) {
  await setStage(options, "checking");
  await api.windivertStatus(false);
  await installWinDivert(options);
}

async function installWinDivert(options: WinDivertInstallOptions) {
  await setStage(options, "downloading");
  const report = await api.windivertInstall();
  await setStage(options, "verifying");
  if (!report.verified_sha256) throw new Error("WinDivert verification failed");
  await setStage(options, "installing");
  const status = await api.windivertStatus(false);
  if (!status.installed) throw new Error("WinDivert installation did not complete");
  await setStage(options, "ready");
}

async function setStage(options: WinDivertInstallOptions, stage: WinDivertInstallStage) {
  await options.onStage?.(stage);
}
