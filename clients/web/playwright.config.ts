import { defineConfig, devices } from "@playwright/test";

process.env.LEPTOS_SITE_ROOT = "target/site-tests";
process.env.LEPTOS_SITE_ADDR = "127.0.0.1:3100";

export default defineConfig({
  testDir: "./e2e",
  outputDir: "../../target/web/e2e",
  fullyParallel: true,
  workers: process.env.CI ? 2 : 12,
  retries: 1,
  maxFailures: process.env.CI ? 20 : undefined,
  reporter: process.env.CI ? [["list"], ["github"]] : [["list"]],
  use: {
    baseURL: "http://127.0.0.1:3100",
    locale: "en-US",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: "cargo leptos serve --split -- --config clients/web/e2e/weaveling.toml",
    cwd: "../..",
    url: "http://127.0.0.1:3100/api/service/health",
    reuseExistingServer: true,
    timeout: 300_000,
  },
});
