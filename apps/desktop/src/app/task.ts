import type { Ref } from "vue";

type TaskRunnerDeps = {
  busy: Ref<boolean>;
  statusText: Ref<string>;
  errorText: Ref<string>;
  formatError(error: unknown): string;
};

export function createTaskRunner(deps: TaskRunnerDeps) {
  return async function runTask(done: string, task: () => Promise<unknown>) {
    deps.busy.value = true;
    deps.errorText.value = "";
    try {
      await task();
      deps.statusText.value = done;
    } catch (error) {
      deps.errorText.value = deps.formatError(error);
    } finally {
      deps.busy.value = false;
    }
  };
}
