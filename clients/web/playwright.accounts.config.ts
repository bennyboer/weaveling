import { resolve } from "node:path";
import { defineConfig } from "@playwright/test";

import served from "./playwright.config";

const drop = resolve(__dirname, "../../target/web/mail-drop", `${Date.now()}`);
const signedIn = resolve(__dirname, "../../target/web/e2e-accounts/signed-in.json");

process.env.WEAVELING_E2E_ACCOUNTS = "on";
process.env.WEAVELING_E2E_MAIL_DROP = drop;
process.env.WEAVELING_E2E_SIGNED_IN = signedIn;

const [api, client] = [served.webServer].flat();

const inherited = Object.fromEntries(
  Object.entries(process.env).filter(
    (entry): entry is [string, string] => entry[1] !== undefined,
  ),
);

export default defineConfig({
  ...served,
  outputDir: "../../target/web/e2e-accounts",
  globalSetup: "./e2e/support/signed-in.ts",
  use: { ...served.use, storageState: signedIn },
  webServer: [
    {
      ...api,
      env: {
        ...inherited,
        WEAVELING_ACCOUNTS: "on",
        WEAVELING_MAIL_DROP: drop,
      },
      reuseExistingServer: false,
    },
    client,
  ],
});
