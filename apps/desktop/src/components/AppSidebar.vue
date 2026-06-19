<script setup lang="ts">
import { Plus } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">NTE</div>
        <div>
          <strong>Gacha Exporter</strong>
          <span>local tracker</span>
        </div>
      </div>

      <label class="field">
        <span>Profile</span>
        <select v-model="app.activeProfileName" :disabled="app.isWorkflowBusy" @change="app.selectProfile">
          <option v-for="profile in app.profiles" :key="profile.name" :value="profile.name">
            {{ profile.name }}
          </option>
        </select>
      </label>

      <form class="inline-form" @submit.prevent="app.createProfile">
        <input v-model="app.newProfileName" placeholder="new_profile" />
        <button type="submit" :disabled="app.isWorkflowBusy || !app.newProfileName.trim()" title="Create profile">
          <Plus :size="16" />
        </button>
      </form>

      <nav class="nav-list">
        <button
          v-for="item in app.navItems"
          :key="item.id"
          :data-agent-id="`nav-${item.id}`"
          :class="{ active: app.activeView === item.id }"
          type="button"
          @click="app.activeView = item.id"
        >
          <component :is="item.icon" :size="18" />
          <span>{{ item.label }}</span>
        </button>
      </nav>
    </aside>
</template>
