<script setup lang="ts">
import { Check, Plus, Trash2, X } from "lucide-vue-next";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { Profile } from "../api";
import { useAppContext } from "../app/context";
import { profileAgentId } from "../app/profileNames";

type ProfileDialogMode = "create" | "rename" | "delete" | null;

const props = defineProps<{
  mode: ProfileDialogMode;
  profile: Profile | null;
}>();
const emit = defineEmits<{
  close: [];
}>();

const app = useAppContext();
const dialogEl = ref<HTMLFormElement | null>(null);
const inputEl = ref<HTMLInputElement | null>(null);
const cancelButton = ref<HTMLButtonElement | null>(null);
const profileName = ref("");
const submitted = ref(false);

const isNameDialog = computed(() => props.mode === "create" || props.mode === "rename");
const title = computed(() => {
  if (props.mode === "create") return app.t("profile.create");
  if (props.mode === "rename") return app.t("profile.rename");
  return app.t("profile.delete");
});
const submitLabel = computed(() => {
  if (props.mode === "create") return app.t("profile.create");
  if (props.mode === "rename") return app.t("profile.renameSave");
  return app.t("common.delete");
});
const canSubmit = computed(() => {
  if (app.isWorkflowBusy) return false;
  if (props.mode === "delete") return Boolean(props.profile);
  const name = profileName.value.trim();
  if (!name) return false;
  return props.mode !== "rename" || name !== props.profile?.name;
});

watch(
  () => [props.mode, props.profile?.name] as const,
  async ([mode, name]) => {
    submitted.value = false;
    profileName.value = mode === "rename" ? (name ?? "") : "";
    if (!mode) return;
    await nextTick();
    if (isNameDialog.value) {
      inputEl.value?.focus();
      if (mode === "rename") inputEl.value?.select();
    } else {
      cancelButton.value?.focus();
    }
  },
  { immediate: true },
);

function close() {
  if (app.isWorkflowBusy) return;
  emit("close");
}

async function submit() {
  if (!canSubmit.value) return;
  submitted.value = false;
  let succeeded = false;
  if (props.mode === "create") {
    succeeded = await app.createProfile(profileName.value);
  } else if (props.mode === "rename" && props.profile) {
    succeeded = await app.renameProfile(props.profile.name, profileName.value);
  } else if (props.mode === "delete" && props.profile) {
    succeeded = await app.deleteProfile(props.profile);
  }
  if (succeeded) emit("close");
  else submitted.value = true;
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Tab" || !dialogEl.value) return;
  const focusable = [...dialogEl.value.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled)")];
  if (!focusable.length) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault();
    last.focus();
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault();
    first.focus();
  }
}

function onDocumentKeydown(event: KeyboardEvent) {
  if (!props.mode || event.key !== "Escape") return;
  event.preventDefault();
  close();
}

onMounted(() => document.addEventListener("keydown", onDocumentKeydown));
onBeforeUnmount(() => document.removeEventListener("keydown", onDocumentKeydown));
</script>

<template>
  <Teleport to="body">
    <div v-if="mode" class="update-dialog-backdrop profile-dialog-backdrop" @pointerdown.self="close">
      <form
        ref="dialogEl"
        class="update-dialog profile-dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="profile-dialog-title"
        @submit.prevent="submit"
        @keydown="onKeydown"
      >
        <header class="update-dialog-head">
          <h2 id="profile-dialog-title">{{ title }}</h2>
          <button type="button" :disabled="app.isWorkflowBusy" :title="app.t('common.close')" :aria-label="app.t('common.close')" @click="close">
            <X :size="17" />
          </button>
        </header>

        <div class="update-dialog-body profile-dialog-body">
          <label v-if="isNameDialog" class="field">
            <span>{{ app.t("profile.createName") }}</span>
            <input
              ref="inputEl"
              v-model.trim="profileName"
              :data-agent-id="mode === 'create' ? 'profile-create-input' : 'profile-rename-input'"
              name="profile-name"
              required
              maxlength="255"
              placeholder="new_profile"
              autocomplete="off"
              :disabled="app.isWorkflowBusy"
              @input="submitted = false"
            />
          </label>
          <p v-else-if="profile" class="profile-dialog-copy">
            {{ app.t("profile.deleteConfirm", { name: profile.name }) }}
          </p>
          <p v-if="submitted && app.errorText" class="profile-dialog-error" role="alert">{{ app.errorText }}</p>
        </div>

        <footer class="update-dialog-actions profile-dialog-actions">
          <button ref="cancelButton" type="button" :disabled="app.isWorkflowBusy" data-agent-id="profile-dialog-cancel" @click="close">
            {{ app.t("common.cancel") }}
          </button>
          <button
            type="submit"
            :class="{ primary: mode !== 'delete', danger: mode === 'delete' }"
            :data-agent-id="mode === 'create' ? 'profile-create-submit' : mode === 'rename' ? 'profile-rename-save' : profileAgentId('delete-confirm', profile?.name ?? '')"
            :disabled="!canSubmit"
          >
            <Plus v-if="mode === 'create'" :size="16" />
            <Check v-else-if="mode === 'rename'" :size="16" />
            <Trash2 v-else :size="16" />
            <span>{{ submitLabel }}</span>
          </button>
        </footer>
      </form>
    </div>
  </Teleport>
</template>
