import { Activity, History, Settings } from "lucide-vue-next";
import type { Component } from "vue";
import type { I18nKey } from "./i18n";

export type ViewId = "dashboard" | "records" | "settings";
export type NavItem = { id: ViewId; labelKey: I18nKey; icon: Component };

export const navItems = [
  { id: "dashboard", labelKey: "nav.dashboard", icon: Activity },
  { id: "records", labelKey: "nav.records", icon: History },
  { id: "settings", labelKey: "nav.settings", icon: Settings },
] satisfies NavItem[];
