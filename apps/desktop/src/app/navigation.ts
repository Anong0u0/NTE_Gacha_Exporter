import { Activity, FolderInput, History, Settings } from "lucide-vue-next";
import type { Component } from "vue";
import type { I18nKey } from "./i18n";

export type ViewId = "dashboard" | "records" | "import_export" | "settings";
export type NavItem = { id: ViewId; labelKey: I18nKey; icon: Component };

export const navItems = [
  { id: "dashboard", labelKey: "nav.dashboard", icon: Activity },
  { id: "records", labelKey: "nav.records", icon: History },
  { id: "import_export", labelKey: "nav.importExport", icon: FolderInput },
  { id: "settings", labelKey: "nav.settings", icon: Settings },
] satisfies NavItem[];
