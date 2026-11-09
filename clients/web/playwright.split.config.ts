import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";

import served from "./playwright.config";

const [server] = [served.webServer].flat();

process.env.WEAVELING_E2E_SPLIT = "on";

const inherited = Object.fromEntries(
  Object.entries(process.env).filter(
    (entry): entry is [string, string] => entry[1] !== undefined,
  ),
);

export default defineConfig({
  ...served,
  outputDir: "../../target/web/e2e-split",
  webServer: {
    ...server,
    command:
      "cargo leptos serve --release --split -- --config clients/web/e2e/weaveling.toml",
    env: {
      ...inherited,
      CARGO_TARGET_DIR: resolve(__dirname, "../../target/split"),
    },
    reuseExistingServer: false,
    timeout: 900_000,
  },
});
