/// <reference types="vite/client" />

declare global {
  const __NTE_APP_VERSION__: string;

  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

export {};
