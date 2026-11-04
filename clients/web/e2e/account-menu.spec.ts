import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";

const masthead = (page: Page) => page.getByRole("banner");

const accountButton = (page: Page) =>
  masthead(page).getByRole("button", { name: "Account: Ada Lovelace" });

const menu = (page: Page) => page.getByRole("menu");

async function signedIn(page: Page) {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();
}

test("the account opens a menu naming who is signed in", async ({ page }) => {
  // given: an author signed in
  await signedIn(page);

  // when: they open their account
  await accountButton(page).click();

  // then: a menu names who is signed in
  await expect(accountButton(page)).toHaveAttribute("aria-expanded", "true");
  await expect(menu(page)).toContainText(ADA.name);
  await expect(menu(page)).toContainText(ADA.email);
  await expect(
    menu(page).getByRole("menuitem", { name: "Sign out" }),
  ).toBeVisible();
});

test("the menu closes again by its button, by Escape and by a click elsewhere", async ({
  page,
}) => {
  // given: an author signed in
  await signedIn(page);

  // when: the menu is opened and its button pressed again
  await accountButton(page).click();
  await accountButton(page).click();
  // then: it closes
  await expect(menu(page)).toHaveCount(0);

  // when: it is opened and escape pressed
  await accountButton(page).click();
  await page.keyboard.press("Escape");
  // then: it closes
  await expect(menu(page)).toHaveCount(0);

  // when: it is opened and somewhere else clicked
  await accountButton(page).click();
  await page.getByRole("heading", { name: "Your projects" }).click();
  // then: it closes
  await expect(menu(page)).toHaveCount(0);
  await expect(accountButton(page)).toHaveAttribute("aria-expanded", "false");
});

test("on a phone, the account takes no row of its own", async ({ page }) => {
  // given: a phone
  await page.setViewportSize({ width: 390, height: 800 });
  // and: an author signed in
  await signedIn(page);

  // then: the account sits on the masthead's row
  const account = await accountButton(page).boundingBox();
  const more = await masthead(page)
    .getByRole("button", { name: "More", exact: true })
    .boundingBox();

  expect(account && more).toBeTruthy();
  expect(
    Math.abs(account!.y + account!.height / 2 - (more!.y + more!.height / 2)),
    "the account sits beside the masthead's More, not on a line below it",
  ).toBeLessThan(4);
});
