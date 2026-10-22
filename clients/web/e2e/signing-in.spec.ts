import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";
import { withAccounts } from "./support/signed-in";

const masthead = (page: Page) => page.getByRole("banner");

const accountButton = (page: Page) =>
  masthead(page).getByRole("button", { name: /^Account: / });

test("without accounts, the work opens as it always did", async ({ page }) => {
  test.skip(withAccounts, "the server these tests run against keeps accounts");
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(accountButton(page)).toHaveCount(0);
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
  await expect(
    masthead(page).getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toHaveText("A");
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
  await expect(accountButton(page)).toBeVisible();

  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();

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

test("a server out of reach says so, and can be asked again", async ({
  page,
}) => {
  let reachable = false;
  await page.route(
    (url) => url.pathname === "/api/service/mode",
    (route) =>
      reachable ? route.fulfill({ json: { accounts: false } }) : route.abort(),
  );

  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "The server is out of reach" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toHaveCount(0);

  reachable = true;
  await page.getByRole("button", { name: "Try again" }).click();

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("a session run out while working leads back to signing in", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();

  await page.route(
    (url) => url.pathname === "/api/projects",
    (route) =>
      route.request().method() === "POST"
        ? route.fulfill({
            status: 401,
            contentType: "application/problem+json",
            body: JSON.stringify({
              type: "not-signed-in",
              status: 401,
              detail: "sign in first",
            }),
          })
        : route.fallback(),
  );
  await page.getByPlaceholder("A working title…").fill("Outlived");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page
    .getByRole("dialog", { name: "New project" })
    .getByRole("button", { name: "Create project" })
    .click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

for (const [minutes, said] of [
  [1, "Too many tries. Please wait a minute before trying again."],
  [3, "Too many tries. Please wait 3 minutes before trying again."],
] as const) {
  test(`too many tries say to wait ${minutes} minute(s)`, async ({ page }) => {
    await aServerWithAccounts(page);
    await page.route(
      (url) => url.pathname === "/api/sessions",
      (route) =>
        route.fulfill({
          status: 429,
          headers: { "Retry-After": String(minutes * 60) },
          contentType: "application/problem+json",
          body: JSON.stringify({
            type: "too-many-tries",
            status: 429,
            detail: "too many tries",
            minutes,
          }),
        }),
    );
    await page.goto("/");

    await signIn(page, ADA.email, HER_PASSWORD);

    await expect(page.getByRole("alert")).toHaveText(said);
    await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  });
}
