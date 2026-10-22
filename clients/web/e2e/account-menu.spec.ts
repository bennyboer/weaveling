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
  await signedIn(page);

  await accountButton(page).click();

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
  await signedIn(page);

  await accountButton(page).click();
  await accountButton(page).click();
  await expect(menu(page)).toHaveCount(0);

  await accountButton(page).click();
  await page.keyboard.press("Escape");
  await expect(menu(page)).toHaveCount(0);

  await accountButton(page).click();
  await page.getByRole("heading", { name: "Your projects" }).click();
  await expect(menu(page)).toHaveCount(0);
  await expect(accountButton(page)).toHaveAttribute("aria-expanded", "false");
});

test("on a phone, the account takes no row of its own", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 800 });
  await signedIn(page);

  const account = await accountButton(page).boundingBox();
  const language = await masthead(page)
    .getByRole("combobox", { name: "Language" })
    .boundingBox();

  expect(account && language).toBeTruthy();
  expect(
    Math.abs(account!.y + account!.height / 2 - (language!.y + language!.height / 2)),
    "the account sits beside the language, not on a line below the masthead",
  ).toBeLessThan(4);
});
