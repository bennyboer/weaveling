import type { Page } from "@playwright/test";

import { routingLive } from "./live";
import { withAccounts } from "./signed-in";

/** Where there are accounts the alarm is an operator's alone, and the account the tests sign in with is none: the page is told it is one, and the refusals' topic is answered in the server's stead, as the server follows only an operator on it. */
export async function seeingTheAlarm(page: Page) {
  if (!withAccounts) {
    return;
  }
  await page.route("**/api/sessions/current", async (route) => {
    const response = await route.fetch();
    await route.fulfill({
      response,
      json: { ...(await response.json()), operator: true },
    });
  });
  await routingLive(page, { withheld: ["refusals"] });
}
