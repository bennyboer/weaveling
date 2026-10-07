import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";

import served from "./playwright.config";

const data = resolve(__dirname, "../../target/web/local-data", `${Date.now()}`);

const [api, client] = [served.webServer].flat();

const inherited = Object.fromEntries(
  Object.entries(process.env).filter(
    (entry): entry is [string, string] => entry[1] !== undefined,
  ),
);

export default defineConfig({
  ...served,
  outputDir: "../../target/web/e2e-local",
  webServer: [
    {
      ...api,
      env: { ...inherited, WEAVELING_DATA: data },
      reuseExistingServer: false,
    },
    client,
  ],
});
