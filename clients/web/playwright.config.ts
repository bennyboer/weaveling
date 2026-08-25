import { defineConfig, devices } from "@playwright/test";

export default defineConfig({
  testDir: "./e2e",
  outputDir: "../../target/web/e2e",
  fullyParallel: false,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:8080",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: [
    {
      command: "cargo run -p weaveling-service-api",
      cwd: "../..",
      url: "http://127.0.0.1:3000/api/health",
      reuseExistingServer: true,
      timeout: 180_000,
    },
    {
      command: "trunk serve",
      url: "http://127.0.0.1:8080",
      reuseExistingServer: true,
      timeout: 180_000,
    },
  ],
});
