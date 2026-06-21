<script setup lang="ts">
import { provideAppContext } from "./app/context";
import { useApp } from "./app/useApp";
import AppSidebar from "./components/AppSidebar.vue";
import DashboardView from "./components/DashboardView.vue";
import RecordsView from "./components/RecordsView.vue";
import SettingsView from "./components/SettingsView.vue";
import TopBar from "./components/TopBar.vue";

const app = useApp();
provideAppContext(app);
</script>

<template>
  <div class="app-shell">
    <AppSidebar />
    <main class="workspace">
      <TopBar />
      <DashboardView v-if="app.activeView === 'dashboard'" />
      <RecordsView v-else-if="app.activeView === 'records'" />
      <SettingsView v-else />
    </main>
  </div>
</template>
