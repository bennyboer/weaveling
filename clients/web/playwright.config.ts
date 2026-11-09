import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  outputDir: "../../target/web/e2e",
  fullyParallel: true,
  workers: process.env.CI ? 2 : 4,
  retries: 1,
  reporter: process.env.CI ? [["list"], ["github"]] : [["list"]],
  use: {
    baseURL: "http://127.0.0.1:3000",
    locale: "en-US",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "cargo leptos serve -- --config clients/web/e2e/weaveling.toml",
    cwd: "../..",
    url: "http://127.0.0.1:3000/api/service/health",
    reuseExistingServer: true,
    timeout: 300_000,
  },
});
