import { Activity, FolderInput, History, Settings } from "lucide-vue-next";

export type ViewId = "dashboard" | "records" | "import_export" | "settings";

export const navItems = [
  { id: "dashboard" as const, label: "Dashboard", icon: Activity },
  { id: "records" as const, label: "Records", icon: History },
  { id: "import_export" as const, label: "Import/Export", icon: FolderInput },
  { id: "settings" as const, label: "Settings", icon: Settings },
];
