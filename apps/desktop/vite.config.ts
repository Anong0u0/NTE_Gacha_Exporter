import { execFileSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

type CargoMetadata = {
  packages: Array<{
    name: string;
    version: string;
  }>;
};

function readAppVersion() {
  const projectRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const raw = execFileSync("cargo", ["metadata", "--no-deps", "--format-version", "1"], {
    cwd: projectRoot,
    encoding: "utf8",
  });
  const metadata = JSON.parse(raw) as CargoMetadata;
  const desktopPackage = metadata.packages.find((pkg) => pkg.name === "nte-gacha-exporter-desktop");
  if (!desktopPackage) {
    throw new Error("Cargo metadata missing nte-gacha-exporter-desktop package");
  }
  return desktopPackage.version;
}

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  define: {
    __NTE_APP_VERSION__: JSON.stringify(readAppVersion()),
  },
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("node_modules/echarts")) return "charts";
          if (id.includes("node_modules/vue")) return "vue";
          if (id.includes("node_modules/lucide-vue-next")) return "icons";
        },
      },
    },
  },
  server: {
    port: 5173,
    strictPort: true,
  },
});
