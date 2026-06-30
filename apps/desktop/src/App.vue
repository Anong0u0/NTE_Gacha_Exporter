<script setup lang="ts">
import { provideAppContext } from "./app/context";
import { useApp } from "./app/useApp";
import AppSidebar from "./components/AppSidebar.vue";
import CaptureStalledDialog from "./components/CaptureStalledDialog.vue";
import DashboardView from "./components/DashboardView.vue";
import DiagnosticDialog from "./components/DiagnosticDialog.vue";
import RecordsView from "./components/RecordsView.vue";
import SettingsView from "./components/SettingsView.vue";
import TopBar from "./components/TopBar.vue";
import UpdateDialog from "./components/UpdateDialog.vue";

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
    <UpdateDialog />
    <CaptureStalledDialog />
    <DiagnosticDialog />
  </div>
</template>
