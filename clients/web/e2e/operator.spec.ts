import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_OPERATOR_CODE,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";
import { withAccounts } from "./support/signed-in";

const accountButton = (page: Page) =>
  page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" });

const operatorInMenu = (page: Page) =>
  page.getByRole("menu").getByRole("menuitem", { name: "Manage instance" });

async function signedIn(page: Page) {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();

  return server;
}

async function enterCode(page: Page, code: string) {
  await page.getByLabel("Code").fill(code);
  await page.getByRole("button", { name: "Become the operator" }).click();
}

test("the code from the log makes whoever is signed in the operator", async ({
  page,
}) => {
  const server = await signedIn(page);
  await accountButton(page).click();
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toBeVisible();
  await expect(operatorInMenu(page)).toHaveCount(0);

  await page.goto("/instance");
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
  await enterCode(page, THE_OPERATOR_CODE);

  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  expect(server.operator).toBe(true);
});

test("the operator finds the way back through the account's menu", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.operator = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  await accountButton(page).click();
  await operatorInMenu(page).click();

  await expect(page).toHaveURL(/\/instance\/accounts$/);
  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
});

test("the hint stands apart from the field it explains", async ({ page }) => {
  await signedIn(page);
  await page.goto("/instance");

  const hint = await page
    .getByText("Enter the code the server wrote to its log when it started.")
    .boundingBox();
  const label = await page.getByText("Code", { exact: true }).boundingBox();

  expect(hint && label).toBeTruthy();
  expect(label!.y - (hint!.y + hint!.height)).toBeGreaterThanOrEqual(20);
});

test("a wrong code says so and makes no operator", async ({ page }) => {
  const server = await signedIn(page);
  await page.goto("/instance");

  await enterCode(page, "not the code at all");

  await expect(page.getByRole("alert")).toHaveText(
    "This is not the code the server logged.",
  );
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
  expect(server.operator).toBe(false);
});

test("without accounts there is no operator to become", async ({ page }) => {
  test.skip(withAccounts, "the server these tests run against keeps accounts");

  await page.goto("/instance");

  await expect(
    page.getByText(
      "This Weaveling instance keeps no accounts, so it has no operator.",
    ),
  ).toBeVisible();
  await expect(page.getByLabel("Code")).toHaveCount(0);
});
