import { createServer } from "node:net";
import { availableParallelism } from "node:os";
import { defineConfig } from "@playwright/test";

delete process.env.NO_COLOR;

const workerCount = Number(process.env.E2E_WORKERS || availableParallelism());
const e2eHost = "127.0.0.1";
const defaultE2EPort = Number(process.env.E2E_PORT || 5174);
const resolvedBaseURLEnv = "NTE_E2E_RESOLVED_BASE_URL";
const resolvedCommandEnv = "NTE_E2E_RESOLVED_COMMAND";
const resolvedReuseEnv = "NTE_E2E_REUSE_EXISTING_SERVER";

const isPortAvailable = (port) =>
  new Promise((resolve) => {
    const server = createServer();

    server.unref();
    server.once("error", () => resolve(false));
    server.listen({ host: e2eHost, port }, () => {
      server.close(() => resolve(true));
    });
  });

const findAvailablePort = async (startPort) => {
  if (!Number.isInteger(startPort) || startPort < 1 || startPort > 65535) {
    throw new Error(`Invalid E2E_PORT: ${startPort}`);
  }

  for (let port = startPort; port <= 65535; port += 1) {
    if (await isPortAvailable(port)) return port;
  }

  throw new Error(`No available E2E port found from ${startPort}`);
};

const resolveServerTarget = async () => {
  if (process.env[resolvedBaseURLEnv] && process.env[resolvedCommandEnv]) {
    return {
      baseURL: process.env[resolvedBaseURLEnv],
      command: process.env[resolvedCommandEnv],
      reuseExistingServer: process.env[resolvedReuseEnv] === "true",
    };
  }

  let serverTarget;

  if (process.env.E2E_BASE_URL) {
    const url = new URL(process.env.E2E_BASE_URL);
    const port = Number(url.port || (url.protocol === "https:" ? 443 : 80));

    serverTarget = {
      baseURL: process.env.E2E_BASE_URL,
      command: `bun run dev -- --host ${url.hostname} --port ${port} --strictPort`,
      reuseExistingServer: true,
    };
  } else {
    const port = await findAvailablePort(defaultE2EPort);

    serverTarget = {
      baseURL: `http://${e2eHost}:${port}`,
      command: `bun run dev -- --host ${e2eHost} --port ${port} --strictPort`,
      reuseExistingServer: false,
    };
  }

  process.env[resolvedBaseURLEnv] = serverTarget.baseURL;
  process.env[resolvedCommandEnv] = serverTarget.command;
  process.env[resolvedReuseEnv] = String(serverTarget.reuseExistingServer);

  return serverTarget;
};

const serverTarget = await resolveServerTarget();

export default defineConfig({
  testDir: "./tests/e2e",
  workers: workerCount,
  timeout: 60_000,
  expect: {
    timeout: 10_000,
  },
  use: {
    baseURL: serverTarget.baseURL,
    headless: true,
    trace: "on-first-retry",
  },
  webServer: {
    command: serverTarget.command,
    url: serverTarget.baseURL,
    reuseExistingServer: serverTarget.reuseExistingServer,
    timeout: 120_000,
  },
});
