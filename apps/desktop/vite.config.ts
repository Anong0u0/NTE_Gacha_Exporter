import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
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
