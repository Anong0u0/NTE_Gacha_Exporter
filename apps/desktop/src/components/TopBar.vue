<script setup lang="ts">
import { RefreshCw } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
      <header class="topbar">
        <div>
          <span class="eyebrow">{{ app.activeProfile?.name ?? app.activeProfileName }}</span>
          <h1>{{ app.navItems.find((item) => item.id === app.activeView)?.label }}</h1>
        </div>
        <div class="topbar-actions">
          <button type="button" :disabled="app.isWorkflowBusy" title="Refresh" @click="app.runTask('Dashboard updated', app.refreshAll)">
            <RefreshCw :size="17" />
          </button>
          <div class="status" :class="{ error: app.errorText }">{{ app.errorText || app.statusText }}</div>
        </div>
      </header>
</template>
