import type { Ref } from "vue";

import { api, type Profile } from "../api";
import type { I18nKey } from "./i18n";

type ProfileActionsDeps = {
  profiles: Ref<Profile[]>;
  activeProfileName: Ref<string>;
  newProfileName: Ref<string>;
  profileRenameSource: Ref<string>;
  profileRenameName: Ref<string>;
  profileDeleteTarget: Ref<string>;
  locale: Ref<string>;
  uiLocale: Ref<string>;
  settingsUpdateChannel: Ref<string>;
  settingsCheckUpdates: Ref<boolean>;
  runTask(done: string, task: () => Promise<unknown>): Promise<void>;
  t(key: I18nKey, params?: Record<string, string | number | boolean | null | undefined>): string;
  saveRecordViewPrefs(profileName?: string): void;
  setActiveProfileName(profileName: string): void;
  copyRecordViewPrefs(sourceProfileName: string, targetProfileName: string): void;
  removeRecordViewPrefs(profileName: string): void;
  refreshAll(): Promise<void>;
};

export function createProfileActions(deps: ProfileActionsDeps) {
  async function loadProfiles() {
    deps.profiles.value = await api.listProfiles();
    if (!deps.profiles.value.some((profile) => profile.name === deps.activeProfileName.value) && deps.profiles.value.length > 0) {
      deps.setActiveProfileName(deps.profiles.value[0].name);
    }
  }

  async function createProfile() {
    const name = deps.newProfileName.value.trim();
    if (!name) return;
    await deps.runTask(deps.t("status.profileCreated"), async () => {
      const profile = await api.createProfile(name);
      deps.newProfileName.value = "";
      deps.profileDeleteTarget.value = "";
      await api.setActiveProfile(profile.name);
      deps.setActiveProfileName(profile.name);
      deps.removeRecordViewPrefs(profile.name);
      await loadProfiles();
      await deps.refreshAll();
    });
  }

  function startRenameProfile(profile: Profile) {
    deps.profileRenameSource.value = profile.name;
    deps.profileRenameName.value = profile.name;
    deps.profileDeleteTarget.value = "";
  }

  function cancelRenameProfile() {
    deps.profileRenameSource.value = "";
    deps.profileRenameName.value = "";
  }

  async function saveProfileRename() {
    const oldName = deps.profileRenameSource.value;
    const newName = deps.profileRenameName.value.trim();
    if (!oldName || !newName) return;
    if (oldName === newName) {
      cancelRenameProfile();
      return;
    }
    await deps.runTask(deps.t("status.profileRenamed"), async () => {
      if (oldName === deps.activeProfileName.value) deps.saveRecordViewPrefs(oldName);
      const profile = await api.renameProfile(oldName, newName);
      deps.copyRecordViewPrefs(oldName, profile.name);
      deps.removeRecordViewPrefs(oldName);
      if (deps.activeProfileName.value === oldName || profile.active) {
        deps.setActiveProfileName(profile.name);
      }
      deps.profileDeleteTarget.value = "";
      cancelRenameProfile();
      await loadProfiles();
      await deps.refreshAll();
    });
  }

  function requestDeleteProfile(profile: Profile) {
    if (deps.profiles.value.length <= 1) return;
    deps.profileRenameSource.value = "";
    deps.profileRenameName.value = "";
    deps.profileDeleteTarget.value = profile.name;
  }

  function cancelDeleteProfile() {
    deps.profileDeleteTarget.value = "";
  }

  async function confirmDeleteProfile(profile: Profile) {
    if (deps.profiles.value.length <= 1) return;
    await deps.runTask(deps.t("status.profileDeleted"), async () => {
      const settings = await api.deleteProfile(profile.name);
      deps.removeRecordViewPrefs(profile.name);
      if (profile.name !== deps.activeProfileName.value) deps.saveRecordViewPrefs();
      deps.setActiveProfileName(settings.active_profile);
      deps.locale.value = settings.locale;
      deps.uiLocale.value = settings.ui_locale || deps.uiLocale.value;
      deps.settingsUpdateChannel.value = settings.update_channel;
      deps.settingsCheckUpdates.value = settings.check_updates_on_startup;
      if (deps.profileRenameSource.value === profile.name) {
        cancelRenameProfile();
      }
      deps.profileDeleteTarget.value = "";
      await loadProfiles();
      await deps.refreshAll();
    });
  }

  async function selectProfile(profileName = deps.activeProfileName.value) {
    const previousProfileName = deps.activeProfileName.value;
    if (!profileName || profileName === previousProfileName) return;
    deps.saveRecordViewPrefs(previousProfileName);
    deps.setActiveProfileName(profileName);
    deps.profileDeleteTarget.value = "";
    cancelRenameProfile();
    await deps.runTask(deps.t("status.profileSelected"), async () => {
      try {
        const settings = await api.updateSettings({ active_profile: profileName });
        deps.locale.value = settings.locale;
        deps.uiLocale.value = settings.ui_locale || deps.uiLocale.value;
        deps.settingsUpdateChannel.value = settings.update_channel;
        deps.settingsCheckUpdates.value = settings.check_updates_on_startup;
        await loadProfiles();
        await deps.refreshAll();
      } catch (error) {
        deps.setActiveProfileName(previousProfileName);
        await deps.refreshAll();
        throw error;
      }
    });
  }

  return {
    loadProfiles,
    createProfile,
    startRenameProfile,
    cancelRenameProfile,
    saveProfileRename,
    requestDeleteProfile,
    cancelDeleteProfile,
    confirmDeleteProfile,
    selectProfile,
  };
}
