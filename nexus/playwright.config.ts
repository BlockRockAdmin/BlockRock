import { defineConfig } from "@playwright/test";
import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

// Database sintetico e privato, distinto da quello dell'utente.
const directory = mkdtempSync(join(tmpdir(), "nexus-e2e-"));
export default defineConfig({
  testDir: "./e2e",
  fullyParallel: false,
  workers: 1,
  timeout: 60000,
  use: {
    baseURL: "http://127.0.0.1:8790",
    viewport: { width: 1440, height: 1000 },
    launchOptions: { executablePath: process.env.NEXUS_CHROME_PATH },
    screenshot: "only-on-failure",
    trace: "retain-on-failure",
  },
  webServer: {
    command: `${process.env.NEXUS_PYTHON || "server/.venv/bin/python"} -m uvicorn server.app:create_app --factory --host 127.0.0.1 --port 8790 --no-access-log`,
    url: "http://127.0.0.1:8790/api/health",
    reuseExistingServer: false,
    env: {
      NEXUS_DATABASE: join(directory, "test.sqlite3"),
      NEXUS_ORIGIN: "http://127.0.0.1:8790",
      NEXUS_INVITATION: "nexus-e2e-invitation-not-a-real-secret",
    },
  },
});
