import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./stress",
  outputDir: "../../target/stress/results",
  timeout: 30 * 60_000,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:8080",
    viewport: { width: 1440, height: 900 },
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1440, height: 900 } } }],
});
