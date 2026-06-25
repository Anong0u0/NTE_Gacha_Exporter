<script setup lang="ts">
import { Check, Pencil, Plus, Trash2, UserRound, X } from "lucide-vue-next";
import { useAppContext } from "../app/context";

const app = useAppContext();
</script>

<template>
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">NTE</div>
        <div>
          <strong>Gacha Exporter</strong>
          <span>{{ app.t("app.subtitle") }}</span>
        </div>
      </div>

      <section class="profile-panel" aria-labelledby="profile-panel-title">
        <div class="profile-panel-head">
          <div>
            <span id="profile-panel-title" class="eyebrow">{{ app.t("common.profile") }}</span>
            <strong>{{ app.activeProfile?.name ?? app.activeProfileName }}</strong>
          </div>
          <span class="profile-count">{{ app.t("profile.count", { count: app.profiles.length }) }}</span>
        </div>

        <div class="profile-list sidebar-profile-list">
          <div
            v-for="profile in app.profiles"
            :key="profile.name"
            class="profile-row sidebar-profile-row"
            :class="{ active: profile.active, 'pending-delete': app.profileDeleteTarget === profile.name }"
            :data-agent-id="`profile-row-${profile.name}`"
          >
            <template v-if="app.profileRenameSource === profile.name">
              <label class="field profile-edit-field">
                <span>{{ app.t("profile.rename") }}</span>
                <input
                  v-model.trim="app.profileRenameName"
                  :aria-label="app.t('profile.rename')"
                  :disabled="app.isWorkflowBusy"
                  @keyup.enter="app.saveProfileRename"
                  @keyup.esc="app.cancelRenameProfile"
                />
              </label>
              <div class="profile-actions">
                <button
                  type="button"
                  data-agent-id="profile-rename-save"
                  :disabled="app.isWorkflowBusy || !app.profileRenameName.trim()"
                  :title="app.t('profile.renameSave')"
                  @click="app.saveProfileRename"
                >
                  <Check :size="16" />
                </button>
                <button type="button" :disabled="app.isWorkflowBusy" :title="app.t('profile.renameCancel')" @click="app.cancelRenameProfile">
                  <X :size="16" />
                </button>
              </div>
            </template>
            <template v-else>
              <button
                class="profile-select"
                type="button"
                :data-agent-id="`profile-select-${profile.name}`"
                :disabled="app.isWorkflowBusy || profile.active"
                @click="app.selectProfile(profile.name)"
              >
                <UserRound :size="16" />
                <span>
                  <strong>{{ profile.name }}</strong>
                  <small>{{ profile.active ? app.t("common.active") : app.t("common.inactive") }}</small>
                </span>
              </button>
              <div class="profile-actions">
                <template v-if="app.profileDeleteTarget === profile.name">
                  <button
                    class="danger"
                    type="button"
                    :data-agent-id="`profile-delete-confirm-${profile.name}`"
                    :disabled="app.isWorkflowBusy"
                    :title="app.t('profile.deleteConfirm', { name: profile.name })"
                    @click="app.confirmDeleteProfile(profile)"
                  >
                    {{ app.t("common.delete") }}
                  </button>
                  <button
                    type="button"
                    :data-agent-id="`profile-delete-cancel-${profile.name}`"
                    :disabled="app.isWorkflowBusy"
                    :title="app.t('profile.renameCancel')"
                    @click="app.cancelDeleteProfile"
                  >
                    <X :size="16" />
                  </button>
                </template>
                <template v-else>
                  <button
                    type="button"
                    :data-agent-id="`profile-rename-${profile.name}`"
                    :disabled="app.isWorkflowBusy"
                    :title="app.t('profile.rename')"
                    @click="app.startRenameProfile(profile)"
                  >
                    <Pencil :size="16" />
                  </button>
                  <button
                    type="button"
                    :data-agent-id="`profile-delete-${profile.name}`"
                    :disabled="app.isWorkflowBusy || app.profiles.length <= 1"
                    :title="app.profiles.length <= 1 ? app.t('profile.deleteLastDisabled') : app.t('profile.delete')"
                    @click="app.requestDeleteProfile(profile)"
                  >
                    <Trash2 :size="16" />
                  </button>
                </template>
              </div>
            </template>
          </div>
        </div>

        <form class="inline-form profile-create-form" @submit.prevent="app.createProfile">
          <label class="field">
            <span>{{ app.t("profile.createName") }}</span>
            <input v-model.trim="app.newProfileName" data-agent-id="profile-create-input" placeholder="new_profile" autocomplete="off" />
          </label>
          <button
            type="submit"
            data-agent-id="profile-create-submit"
            :disabled="app.isWorkflowBusy || !app.newProfileName.trim()"
            :title="app.t('profile.create')"
          >
            <Plus :size="16" />
          </button>
        </form>
      </section>

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
          <span>{{ app.t(item.labelKey) }}</span>
        </button>
      </nav>
    </aside>
</template>
