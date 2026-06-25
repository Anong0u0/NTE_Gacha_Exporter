import { open, save } from "@tauri-apps/plugin-dialog";
import type { Ref } from "vue";

import { api, type BackupReport, type ImportReport, type RestoreReport, type Settings } from "../api";
import type { I18nKey } from "./i18n";
import type { ExportMode, ImportMode } from "./options";

export type DataOperationKind = "import" | "export" | "backup" | "restore";

type DataOperationDeps = {
  activeProfileName: Ref<string>;
  locale: Ref<string>;
  importPath: Ref<string>;
  importMode: Ref<ImportMode>;
  exportPath: Ref<string>;
  exportMode: Ref<ExportMode>;
  backupPath: Ref<string>;
  restorePath: Ref<string>;
  lastReport: Ref<ImportReport | null>;
  lastBackup: Ref<BackupReport | null>;
  lastRestore: Ref<RestoreReport | null>;
  lastDataOperation: Ref<DataOperationKind | null>;
  applySettings(settings: Settings): void;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
  saveRecordViewPrefs(profileName?: string): void;
  loadProfiles(): Promise<void>;
  refreshAll(): Promise<void>;
};

export function createDataOperations(deps: DataOperationDeps) {
  async function pickImportFile(mode: ImportMode) {
    deps.importMode.value = mode;
    const selected = await open({
      title: mode === "raw" ? deps.t("import.rawJsonl") : deps.t("import.publicJson"),
      multiple: false,
      filters:
        mode === "raw"
          ? [{ name: "Raw JSONL", extensions: ["jsonl"] }]
          : [{ name: "Public JSON", extensions: ["json"] }],
    });
    if (typeof selected === "string") {
      deps.importPath.value = selected;
      await runImport();
    }
  }

  async function runImport() {
    const path = deps.importPath.value.trim();
    if (!path) return;
    await deps.runTask(deps.t("status.importCompleted"), async () => {
      deps.lastReport.value =
        deps.importMode.value === "raw"
          ? await api.importRawJsonl(deps.activeProfileName.value, path, deps.locale.value)
          : await api.importPublicJson(deps.activeProfileName.value, path);
      deps.lastDataOperation.value = "import";
      await deps.refreshAll();
    });
  }

  async function pickExportFile(mode: ExportMode) {
    deps.exportMode.value = mode;
    const selected = await save({
      title: mode === "json" ? deps.t("import.publicJson") : "CSV",
      defaultPath: mode === "json" ? `${deps.activeProfileName.value}-history.json` : `${deps.activeProfileName.value}-history.csv`,
      filters:
        mode === "json"
          ? [{ name: "Public JSON", extensions: ["json"] }]
          : [{ name: "CSV", extensions: ["csv"] }],
    });
    if (typeof selected === "string") {
      deps.exportPath.value = selected;
      await runExport();
    }
  }

  async function runExport() {
    const path = deps.exportPath.value.trim();
    if (!path) return;
    await deps.runTask(deps.t("status.exportCompleted"), async () => {
      if (deps.exportMode.value === "json") {
        await api.exportPublicJson(deps.activeProfileName.value, path, deps.locale.value);
      } else {
        await api.exportCsv(deps.activeProfileName.value, path, deps.locale.value);
      }
      deps.lastDataOperation.value = "export";
    });
  }

  async function pickBackupFile() {
    const selected = await save({
      title: deps.t("import.createBackup"),
      defaultPath: `${deps.activeProfileName.value}-nte-data-backup.zip`,
      filters: [{ name: "Backup zip", extensions: ["zip"] }],
    });
    if (typeof selected === "string") {
      deps.backupPath.value = selected;
      await runBackup();
    }
  }

  async function runBackup() {
    const path = deps.backupPath.value.trim();
    await deps.runTask(deps.t("status.backupCreated"), async () => {
      deps.lastBackup.value = await api.createBackup(path || null);
      deps.lastDataOperation.value = "backup";
    });
  }

  async function pickRestoreFile() {
    const selected = await open({
      title: deps.t("import.restoreBackup"),
      multiple: false,
      filters: [{ name: "Backup zip", extensions: ["zip"] }],
    });
    if (typeof selected === "string") {
      deps.restorePath.value = selected;
      await runRestore();
    }
  }

  async function runRestore() {
    const path = deps.restorePath.value.trim();
    if (!path) return;
    await deps.runTask(deps.t("status.backupRestored"), async () => {
      deps.lastRestore.value = await api.restoreBackup(path);
      deps.lastDataOperation.value = "restore";
      const settings = await api.getSettings();
      deps.saveRecordViewPrefs();
      deps.applySettings(settings);
      await deps.loadProfiles();
      await deps.refreshAll();
    });
  }

  return {
    pickImportFile,
    runImport,
    pickExportFile,
    runExport,
    pickBackupFile,
    runBackup,
    pickRestoreFile,
    runRestore,
  };
}
