<script setup lang="ts">
import { RefreshCw } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <header class="topbar">
        <div>
          <span class="eyebrow">{{ app.activeProfile?.name ?? app.activeProfileName }}</span>
          <h1>{{ app.t(app.navItems.find((item) => item.id === app.activeView)?.labelKey ?? "nav.dashboard") }}</h1>
        </div>
        <div class="topbar-actions">
          <button type="button" :disabled="app.isWorkflowBusy" :title="app.t('status.dashboardUpdated')" @click="app.runTask(app.t('status.dashboardUpdated'), app.refreshAll)">
            <RefreshCw :size="17" />
          </button>
          <div class="status" :class="{ error: app.errorText }">{{ app.errorText || app.statusText }}</div>
        </div>
      </header>
</template>
