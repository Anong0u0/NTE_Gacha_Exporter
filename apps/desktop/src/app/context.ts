import { inject, provide, type InjectionKey } from "vue";
import type { AppState } from "./useApp";

const appContextKey: InjectionKey<AppState> = Symbol("app-context");

export function provideAppContext(app: AppState) {
  provide(appContextKey, app);
}

export function useAppContext() {
  const app = inject(appContextKey);
  if (!app) throw new Error("app context not provided");
  return app;
}
