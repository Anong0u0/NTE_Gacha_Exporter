const WINDOWS_FILENAME_UTF16_LIMIT = 255;

export type ProfileAgentAction = "row" | "select" | "menu" | "rename" | "delete" | "delete-confirm";

export function profileAgentId(action: ProfileAgentAction, name: string) {
  return `profile-${action}-${encodeURIComponent(name.normalize("NFC"))}`;
}

export function profileDefaultFilename(name: string, suffix: string) {
  const normalized = name.normalize("NFC").trim() || "profile";
  const availableUnits = Math.max(1, WINDOWS_FILENAME_UTF16_LIMIT - suffix.length);
  return `${truncateUtf16(normalized, availableUnits)}${suffix}`;
}

function truncateUtf16(value: string, maxUnits: number) {
  let result = "";
  let units = 0;
  for (const character of value) {
    if (units + character.length > maxUnits) break;
    result += character;
    units += character.length;
  }
  return result;
}
