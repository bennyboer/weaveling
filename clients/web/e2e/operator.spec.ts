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

async function signedInWithNoOperatorYet(page: Page) {
  const server = await aServerWithAccounts(page);
  server.operatorWanted = true;
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
  // given: an author signed in, the instance without an operator
  const server = await signedInWithNoOperatorYet(page);
  // when: their account menu is opened
  await accountButton(page).click();
  // then: it offers no managing of the instance
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toBeVisible();
  await expect(operatorInMenu(page)).toHaveCount(0);

  // when: the instance is opened and the code from the log entered
  await page.goto("/instance");
  await expect(page).toHaveURL(/\/setup$/);
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
  await enterCode(page, THE_OPERATOR_CODE);

  // then: they manage the instance as its operator
  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  expect(server.operator).toBe(true);
});

test("the operator finds the way back through the account's menu", async ({
  page,
}) => {
  // given: the operator signed in
  const server = await aServerWithAccounts(page);
  server.operator = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: managing the instance is chosen from their account menu
  await accountButton(page).click();
  await operatorInMenu(page).click();

  // then: the instance opens
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
});

test("the hint stands apart from the field it explains", async ({ page }) => {
  // given: an author asked for the operator's code
  await signedInWithNoOperatorYet(page);
  await page.goto("/instance");

  // then: the hint stands apart from the field
  const hint = await page
    .getByText("Enter the code the server wrote to its log when it started.")
    .boundingBox();
  const label = await page.getByText("Code", { exact: true }).boundingBox();

  expect(hint && label).toBeTruthy();
  expect(label!.y - (hint!.y + hint!.height)).toBeGreaterThanOrEqual(20);
});

test("a wrong code says so and makes no operator", async ({ page }) => {
  // given: an author asked for the operator's code
  const server = await signedInWithNoOperatorYet(page);
  await page.goto("/instance");

  // when: a wrong code is entered
  await enterCode(page, "not the code at all");

  // then: it says so and asks again
  await expect(page.getByRole("alert")).toHaveText(
    "This is not the code the server logged.",
  );
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
  // and: no operator is made
  expect(server.operator).toBe(false);
});

test("without accounts there is no operator to become", async ({ page }) => {
  // given: a server without accounts
  test.skip(withAccounts, "the server these tests run against keeps accounts");

  // when: the instance is opened
  await page.goto("/instance");

  // then: it says there is no operator, asking for no code
  await expect(
    page.getByText(
      "This Weaveling instance has no accounts, so it has no operator.",
    ),
  ).toBeVisible();
  await expect(page.getByLabel("Code")).toHaveCount(0);
});
