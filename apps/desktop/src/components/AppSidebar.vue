<script setup lang="ts">
import { Circle, CircleCheck, MoreHorizontal, Pencil, Plus, Trash2 } from "lucide-vue-next";
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Profile } from "../api";
import { useAppContext } from "../app/context";
import { profileAgentId } from "../app/profileNames";
import ProfileDialog from "./ProfileDialog.vue";

type ProfileDialogMode = "create" | "rename" | "delete" | null;

const app = useAppContext();
const profileListEl = ref<HTMLElement | null>(null);
const createButtonEl = ref<HTMLButtonElement | null>(null);
const profileMenuEl = ref<HTMLElement | null>(null);
const menuProfile = ref<Profile | null>(null);
const menuTriggerEl = ref<HTMLButtonElement | null>(null);
const menuPosition = ref({ top: "0px", left: "0px" });
const menuPositioned = ref(false);
const dialogMode = ref<ProfileDialogMode>(null);
const dialogProfile = ref<Profile | null>(null);
let dialogReturnFocus: HTMLElement | null = null;

function profileSelectionLabel(profile: Profile) {
  return `${profile.name}, ${app.t(profile.active ? "common.active" : "common.inactive")}`;
}

async function openProfileMenu(profile: Profile, event: MouseEvent) {
  const trigger = event.currentTarget;
  if (!(trigger instanceof HTMLButtonElement)) return;
  if (menuProfile.value?.name === profile.name) {
    await closeProfileMenu(true);
    return;
  }

  menuProfile.value = profile;
  menuTriggerEl.value = trigger;
  menuPositioned.value = false;
  await nextTick();
  positionProfileMenu();
  menuPositioned.value = true;
  await nextTick();
  profileMenuEl.value?.querySelector<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')?.focus();
}

function positionProfileMenu() {
  const trigger = menuTriggerEl.value;
  const menu = profileMenuEl.value;
  if (!trigger || !menu) return;
  const triggerRect = trigger.getBoundingClientRect();
  const menuRect = menu.getBoundingClientRect();
  const edge = 8;
  const gap = 6;
  const top = triggerRect.bottom + gap + menuRect.height <= window.innerHeight - edge
    ? triggerRect.bottom + gap
    : triggerRect.top - menuRect.height - gap;
  const left = Math.min(
    window.innerWidth - menuRect.width - edge,
    Math.max(edge, triggerRect.right - menuRect.width),
  );
  menuPosition.value = { top: `${Math.max(edge, top)}px`, left: `${left}px` };
}

async function closeProfileMenu(restoreFocus: boolean) {
  const trigger = menuTriggerEl.value;
  menuProfile.value = null;
  menuTriggerEl.value = null;
  menuPositioned.value = false;
  if (!restoreFocus) return;
  await nextTick();
  if (trigger?.isConnected) trigger.focus();
}

function onMenuKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    event.preventDefault();
    void closeProfileMenu(true);
    return;
  }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  const items = [...(profileMenuEl.value?.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)') ?? [])];
  if (!items.length) return;
  event.preventDefault();
  const current = items.indexOf(document.activeElement as HTMLButtonElement);
  const direction = event.key === "ArrowDown" ? 1 : -1;
  items[(current + direction + items.length) % items.length].focus();
}

function onDocumentPointerDown(event: PointerEvent) {
  const target = event.target;
  if (!(target instanceof Node) || !menuProfile.value) return;
  if (profileMenuEl.value?.contains(target) || menuTriggerEl.value?.contains(target)) return;
  void closeProfileMenu(false);
}

function onWindowResize() {
  void closeProfileMenu(false);
}

function openCreateDialog() {
  dialogReturnFocus = createButtonEl.value;
  dialogProfile.value = null;
  dialogMode.value = "create";
}

function openMenuDialog(mode: Exclude<ProfileDialogMode, "create" | null>) {
  const profile = menuProfile.value;
  if (!profile || (mode === "delete" && app.profiles.length <= 1)) return;
  dialogReturnFocus = menuTriggerEl.value;
  dialogProfile.value = profile;
  dialogMode.value = mode;
  void closeProfileMenu(false);
}

async function closeDialog() {
  const returnTarget = dialogReturnFocus;
  dialogMode.value = null;
  dialogProfile.value = null;
  dialogReturnFocus = null;
  await nextTick();
  if (returnTarget?.isConnected) {
    returnTarget.focus();
    return;
  }
  const activeRow = [...(profileListEl.value?.querySelectorAll<HTMLElement>(".sidebar-profile-row") ?? [])]
    .find((row) => row.dataset.agentId === profileAgentId("row", app.activeProfileName));
  const activeSelect = activeRow?.querySelector<HTMLButtonElement>(".profile-select");
  if (activeSelect) activeSelect.focus();
  else createButtonEl.value?.focus();
}

async function selectProfile(profileName: string) {
  await closeProfileMenu(false);
  await app.selectProfile(profileName);
}

async function scrollActiveProfileIntoView() {
  await nextTick();
  const activeRow = [...(profileListEl.value?.querySelectorAll<HTMLElement>(".sidebar-profile-row") ?? [])]
    .find((row) => row.dataset.agentId === profileAgentId("row", app.activeProfileName));
  activeRow?.scrollIntoView({ block: "nearest", inline: "nearest" });
}

watch(
  () => `${app.activeProfileName}\0${app.profiles.map((profile) => profile.name).join("\0")}`,
  () => {
    if (menuProfile.value && !app.profiles.some((profile) => profile.name === menuProfile.value?.name)) {
      void closeProfileMenu(false);
    }
    void scrollActiveProfileIntoView();
  },
  { immediate: true, flush: "post" },
);

onMounted(() => {
  document.addEventListener("pointerdown", onDocumentPointerDown);
  window.addEventListener("resize", onWindowResize);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocumentPointerDown);
  window.removeEventListener("resize", onWindowResize);
});
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
        <div class="profile-panel-title">
          <span id="profile-panel-title" class="eyebrow">{{ app.t("common.profile") }}</span>
          <span class="profile-count" :title="app.t('profile.count', { count: app.profiles.length })" :aria-label="app.t('profile.count', { count: app.profiles.length })">
            {{ app.profiles.length }}
          </span>
        </div>
        <button
          ref="createButtonEl"
          class="profile-create-button"
          type="button"
          data-agent-id="profile-create-open"
          :disabled="app.isWorkflowBusy"
          :title="app.t('profile.create')"
          :aria-label="app.t('profile.create')"
          @click="openCreateDialog"
        >
          <Plus :size="17" />
        </button>
      </div>

      <div ref="profileListEl" class="sidebar-profile-list" data-agent-id="profile-list" @scroll.passive="closeProfileMenu(false)">
        <div
          v-for="profile in app.profiles"
          :key="profile.name"
          class="sidebar-profile-row"
          :class="{ active: profile.active }"
          :data-agent-id="profileAgentId('row', profile.name)"
        >
          <button
            class="profile-select"
            type="button"
            :data-agent-id="profileAgentId('select', profile.name)"
            :disabled="app.isWorkflowBusy"
            :aria-current="profile.active ? 'true' : undefined"
            :aria-label="profileSelectionLabel(profile)"
            :title="profile.name"
            @click="selectProfile(profile.name)"
          >
            <CircleCheck v-if="profile.active" :size="17" />
            <Circle v-else :size="17" />
            <strong>{{ profile.name }}</strong>
          </button>
          <button
            class="profile-more-button"
            type="button"
            :data-agent-id="profileAgentId('menu', profile.name)"
            :disabled="app.isWorkflowBusy"
            :title="app.t('profile.actionsFor', { name: profile.name })"
            :aria-label="app.t('profile.actionsFor', { name: profile.name })"
            aria-haspopup="menu"
            :aria-expanded="menuProfile?.name === profile.name"
            :aria-controls="menuProfile?.name === profile.name ? 'profile-actions-menu' : undefined"
            @click="openProfileMenu(profile, $event)"
          >
            <MoreHorizontal :size="17" />
          </button>
        </div>
      </div>
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

  <Teleport to="body">
    <div
      v-if="menuProfile"
      id="profile-actions-menu"
      ref="profileMenuEl"
      class="profile-actions-menu"
      :class="{ 'is-positioned': menuPositioned }"
      :style="menuPosition"
      role="menu"
      :aria-label="app.t('profile.actionsFor', { name: menuProfile.name })"
      @keydown="onMenuKeydown"
    >
      <button
        type="button"
        role="menuitem"
        :data-agent-id="profileAgentId('rename', menuProfile.name)"
        :disabled="app.isWorkflowBusy"
        @click="openMenuDialog('rename')"
      >
        <Pencil :size="16" />
        <span>{{ app.t("profile.rename") }}</span>
      </button>
      <button
        class="danger-item"
        type="button"
        role="menuitem"
        :data-agent-id="profileAgentId('delete', menuProfile.name)"
        :disabled="app.isWorkflowBusy || app.profiles.length <= 1"
        :title="app.profiles.length <= 1 ? app.t('profile.deleteLastDisabled') : app.t('profile.delete')"
        @click="openMenuDialog('delete')"
      >
        <Trash2 :size="16" />
        <span>{{ app.t("profile.delete") }}</span>
      </button>
    </div>
  </Teleport>

  <ProfileDialog :mode="dialogMode" :profile="dialogProfile" @close="closeDialog" />
</template>
