import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";

const masthead = (page: Page) => page.getByRole("banner");

test("without accounts, the work opens as it always did", async ({ page }) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(
    masthead(page).getByRole("button", { name: "Sign out" }),
  ).toHaveCount(0);
});

test("on a server, signing in comes before any of the work is asked for", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toHaveCount(0);
  expect(server.projectsAsked).toBe(0);
});

test("signing in opens the work and names who signed in", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect(masthead(page)).toContainText("Ada Lovelace");
});

test("signing in keeps the address that was asked for", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/?from=a-link");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  expect(new URL(page.url()).search).toBe("?from=a-link");
});

test("a wrong password says so and stays on signing in", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");

  await signIn(page, ADA.email, "not her password at all");

  await expect(page.getByRole("alert")).toHaveText(
    "This email address and password do not sign in.",
  );
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("an address not yet confirmed is told to enter its code", async ({
  page,
}) => {
  await aServerWithAccounts(page, { verified: false });
  await page.goto("/");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(page.getByRole("alert")).toHaveText(
    "This email address is not confirmed yet. Enter the code we sent to it.",
  );
});

test("signing out ends the session and asks to sign in again", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(masthead(page)).toContainText("Ada Lovelace");

  await masthead(page).getByRole("button", { name: "Sign out" }).click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  expect(server.signedOut).toBe(1);
});

test("signing in can be done in the reader's own language", async ({
  browser,
}) => {
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  await aServerWithAccounts(page);

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Anmelden" })).toBeVisible();
  await expect(page.getByLabel("Passwort")).toBeVisible();
  await context.close();
});
