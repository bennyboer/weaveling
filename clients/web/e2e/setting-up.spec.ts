import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_CODE,
  THE_OPERATOR_CODE,
  aServerWithAccounts,
  signIn,
  type Server,
} from "./support/accounts";

async function aFreshInstanceByInvitation(page: Page): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";
  server.operatorWanted = true;
  await page.goto("/");

  return server;
}

async function setUp(page: Page, code: string) {
  await page
    .getByRole("button", { name: "Set up this Weaveling instance" })
    .click();
  await page.getByLabel("Code from the server's log").fill(code);
  await page.getByLabel("Your name").fill(ADA.name);
  await page.getByLabel("Email address").fill(ADA.email);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up", exact: true }).click();
}

test("the first account signs up with the logged code and, confirmed and signed in, runs the instance", async ({
  page,
}) => {
  const server = await aFreshInstanceByInvitation(page);

  await setUp(page, ` ${THE_OPERATOR_CODE} `);
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await page.getByLabel("Code", { exact: true }).fill(THE_CODE);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();

  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  expect(server.operator).toBe(true);
  expect(server.signedUp).toEqual({ email: ADA.email, name: ADA.name });
});

test("a code that is not the logged one is refused in place", async ({
  page,
}) => {
  const server = await aFreshInstanceByInvitation(page);

  await setUp(page, "0123456789abcdef0123456789abcdef");

  await expect(page.getByRole("alert")).toContainText(
    "This is not the code the server logged.",
  );
  expect(server.signedUp).toBeNull();
});

test("once an instance has an operator, nobody is offered to set it up", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Forgot your password?" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Set up this Weaveling instance" }),
  ).toHaveCount(0);
});

test("where anyone may sign up, setting up is offered beside signing up while nobody runs the instance", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.operatorWanted = true;
  await page.goto("/");

  await expect(
    page.getByRole("button", { name: "No account yet? Sign up" }),
  ).toBeVisible();
  await setUp(page, THE_OPERATOR_CODE);
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await page.getByLabel("Code", { exact: true }).fill(THE_CODE);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();

  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  expect(server.operator).toBe(true);
});

test("the word under the heading stands apart from the first field", async ({
  page,
}) => {
  await aFreshInstanceByInvitation(page);
  await page
    .getByRole("button", { name: "Set up this Weaveling instance" })
    .click();

  const hint = await page
    .getByText("Nobody runs this Weaveling instance yet.")
    .boundingBox();
  const field = await page
    .locator(".field > span")
    .filter({ hasText: "Code from the server's log" })
    .boundingBox();
  const between = await page
    .locator(".field > span")
    .filter({ hasText: "Your name" })
    .boundingBox();
  const code = await page
    .getByLabel("Code from the server's log")
    .boundingBox();
  expect(hint && field && between && code).toBeTruthy();

  expect(
    field!.y - (hint!.y + hint!.height),
    "as much room as between one field and the next",
  ).toBeGreaterThanOrEqual(between!.y - (code!.y + code!.height));
});

test("setting up has an address of its own", async ({ page }) => {
  await aFreshInstanceByInvitation(page);

  await page.goto("/setup");

  await expect(
    page.getByRole("heading", { name: "Set up this Weaveling instance" }),
  ).toBeVisible();
  await expect(page.getByLabel("Code from the server's log")).toBeVisible();
});

test("signed in while nobody runs the instance, the menu leads to becoming its operator", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.operatorWanted = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  await page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" })
    .click();
  await page
    .getByRole("menuitem", { name: "Set up this Weaveling instance" })
    .click();

  await expect(page).toHaveURL(/\/setup$/);
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
});

test("signed in where someone runs the instance, setting up says so", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Set up this Weaveling instance" }),
  ).toHaveCount(0);

  await page.goto("/setup");

  await expect(page.getByRole("main")).toContainText(
    "This Weaveling instance is set up already",
  );
  await expect(page.getByLabel("Code")).toHaveCount(0);
});
