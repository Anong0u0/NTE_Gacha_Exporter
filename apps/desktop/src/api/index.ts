import { mockApi } from "./mock";
import { tauriApi } from "./tauri";
import type { AppApi } from "./types";

const isTauri = () => Boolean(window.__TAURI_INTERNALS__);

export const api: AppApi = isTauri() ? tauriApi : mockApi;
