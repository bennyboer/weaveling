import { test, expect, type Page } from "@playwright/test";

import {
  captureIdea,
  openTheBoard,
  openTheOutline,
  openThePool,
} from "./support/shell";
import {
  aBookInTheWritingView,
  openTheWritingView,
  scene,
} from "./support/writing";

const refused = (page: Page) =>
  page.evaluate(
    () => (window as unknown as { refused: string[] }).refused ?? [],
  );

test.beforeEach(async ({ page }) => {
  await page.addInitScript(() => {
    const kept: string[] = [];
    (window as unknown as { refused: string[] }).refused = kept;
    document.addEventListener("securitypolicyviolation", (event) =>
      kept.push(`${event.violatedDirective} ${event.blockedURI}`),
    );
  });
});

test("the page is handed out with its security policy", async ({ page }) => {
  // when: the app opens
  const answered = await page.goto("/");

  // then: it is told what may run and that no other site frames it
  const headers = answered!.headers();
  expect(headers["content-security-policy"]).toContain(
    "frame-ancestors 'none'",
  );
  expect(headers["x-content-type-options"]).toBe("nosniff");
});

test("writing, the outline and the board run with nothing refused by the policy", async ({
  page,
}) => {
  // given: a book open in the writing view
  const made = await aBookInTheWritingView(page, "Shielded", {
    "Chapter 1": 1,
  });
  const [first] = made["Chapter 1"];

  // when: a scene is written in
  await scene(page, first).click();
  await page.keyboard.type("It was a dark night.");
  await expect(scene(page, first)).toContainText("It was a dark night.");
  // and: the outline, the board and the pool are opened, an idea captured in it
  await openTheOutline(page);
  await openTheBoard(page);
  await openThePool(page);
  await captureIdea(page, "A lighthouse");
  await openTheWritingView(page);

  // then: the policy refused nothing
  expect(await refused(page)).toEqual([]);
});
