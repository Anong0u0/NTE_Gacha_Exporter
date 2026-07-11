import type { Ref } from "vue";

import { api, type Profile, type Settings } from "../api";
import type { I18nKey } from "./i18n";

type ProfileActionsDeps = {
  profiles: Ref<Profile[]>;
  activeProfileName: Ref<string>;
  applySettings(settings: Settings): void;
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

  async function createProfile(name: string) {
    const profileName = name.trim();
    if (!profileName) return false;
    let succeeded = false;
    await deps.runTask(deps.t("status.profileCreated"), async () => {
      const profile = await api.createProfile(profileName);
      await api.setActiveProfile(profile.name);
      deps.setActiveProfileName(profile.name);
      deps.removeRecordViewPrefs(profile.name);
      await loadProfiles();
      succeeded = true;
      await deps.refreshAll();
    });
    return succeeded;
  }

  async function renameProfile(oldName: string, name: string) {
    const newName = name.trim();
    if (!oldName || !newName) return false;
    if (oldName === newName) return true;
    let succeeded = false;
    await deps.runTask(deps.t("status.profileRenamed"), async () => {
      if (oldName === deps.activeProfileName.value) deps.saveRecordViewPrefs(oldName);
      const profile = await api.renameProfile(oldName, newName);
      deps.copyRecordViewPrefs(oldName, profile.name);
      deps.removeRecordViewPrefs(oldName);
      if (deps.activeProfileName.value === oldName || profile.active) {
        deps.setActiveProfileName(profile.name);
      }
      await loadProfiles();
      succeeded = true;
      await deps.refreshAll();
    });
    return succeeded;
  }

  async function deleteProfile(profile: Profile) {
    if (deps.profiles.value.length <= 1) return false;
    let succeeded = false;
    await deps.runTask(deps.t("status.profileDeleted"), async () => {
      const settings = await api.deleteProfile(profile.name);
      deps.removeRecordViewPrefs(profile.name);
      if (profile.name !== deps.activeProfileName.value) deps.saveRecordViewPrefs();
      deps.applySettings(settings);
      await loadProfiles();
      succeeded = true;
      await deps.refreshAll();
    });
    return succeeded;
  }

  async function selectProfile(profileName = deps.activeProfileName.value) {
    const previousProfileName = deps.activeProfileName.value;
    if (!profileName || profileName === previousProfileName) return;
    deps.saveRecordViewPrefs(previousProfileName);
    deps.setActiveProfileName(profileName);
    await deps.runTask(deps.t("status.profileSelected"), async () => {
      try {
        const settings = await api.updateSettings({ active_profile: profileName });
        deps.applySettings(settings);
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
    renameProfile,
    deleteProfile,
    selectProfile,
  };
}
