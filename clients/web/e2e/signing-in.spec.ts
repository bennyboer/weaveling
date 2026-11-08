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
  // when: the page opens on a server without accounts
  await page.goto("/");

  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  // and: nobody is asked to sign in or named
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(accountButton(page)).toHaveCount(0);
});

test("on a server, signing in comes before any of the work is asked for", async ({
  page,
}) => {
  // given: a server with accounts
  const server = await aServerWithAccounts(page);

  // when: the page opens
  await page.goto("/");

  // then: signing in is asked for
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  // and: none of the work is shown or asked for
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toHaveCount(0);
  expect(server.projectsAsked).toBe(0);
});

test("signing in opens the work and names who signed in", async ({ page }) => {
  // given: a server with accounts
  await aServerWithAccounts(page);
  await page.goto("/");

  // when: an author signs in
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  // and: she is named
  await expect(
    masthead(page).getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toHaveText("A");
});

test("signing in keeps the address that was asked for", async ({ page }) => {
  // given: a server with accounts, opened from a link
  await aServerWithAccounts(page);
  await page.goto("/?from=a-link");

  // when: an author signs in
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  // and: the link's address is kept
  expect(new URL(page.url()).search).toBe("?from=a-link");
});

test("a wrong password says so and stays on signing in", async ({ page }) => {
  // given: a server with accounts
  await aServerWithAccounts(page);
  await page.goto("/");

  // when: an author signs in with a wrong password
  await signIn(page, ADA.email, "not her password at all");

  // then: she is told it does not sign in
  await expect(page.getByRole("alert")).toHaveText(
    "This email address and password do not sign in.",
  );
  // and: she stays on signing in
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("an address not yet confirmed is told to enter its code", async ({
  page,
}) => {
  // given: a server with accounts whose address is not confirmed
  await aServerWithAccounts(page, { verified: false });
  await page.goto("/");

  // when: the author signs in
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: she is told to enter the code sent to her
  await expect(page.getByRole("alert")).toHaveText(
    "This email address is not confirmed yet. Enter the code we sent to it.",
  );
});

test("signing out ends the session and asks to sign in again", async ({
  page,
}) => {
  // given: an author signed in
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();

  // when: she signs out
  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();

  // then: she is asked to sign in again
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  // and: her session is ended
  expect(server.signedOut).toBe(1);
});

test("signing in can be done in the reader's own language", async ({
  browser,
}) => {
  // given: a server with accounts, read in german
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  await aServerWithAccounts(page);

  // when: the page opens
  await page.goto("/");

  // then: signing in is asked in german
  await expect(page.getByRole("heading", { name: "Anmelden" })).toBeVisible();
  await expect(page.getByLabel("Passwort")).toBeVisible();
  await context.close();
});

test("a server out of reach says so, and can be asked again", async ({
  page,
}) => {
  // given: a server out of reach
  let reachable = false;
  await page.route(
    (url) => url.pathname === "/api/service/mode",
    (route) =>
      reachable ? route.fulfill({ json: { accounts: null } }) : route.abort(),
  );

  // when: the page opens
  await page.goto("/");

  // then: it says the server is out of reach
  await expect(
    page.getByRole("heading", { name: "The server is out of reach" }),
  ).toBeVisible();
  // and: none of the work and no signing in is shown
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toHaveCount(0);

  // when: the server can be reached again and is asked again
  reachable = true;
  await page.getByRole("button", { name: "Try again" }).click();

  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("a session run out while working leads back to signing in", async ({
  page,
}) => {
  // given: an author signed in
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();

  // and: her session run out
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
  // when: she creates a project
  await page.getByPlaceholder("A working title…").fill("Outlived");
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page
    .getByRole("dialog", { name: "New project" })
    .getByRole("button", { name: "Create project" })
    .click();

  // then: she is asked to sign in again
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  // when: she signs in
  await signIn(page, ADA.email, HER_PASSWORD);
  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

for (const [minutes, said] of [
  [1, "Too many tries. Please wait a minute before trying again."],
  [3, "Too many tries. Please wait 3 minutes before trying again."],
] as const) {
  test(`too many tries say to wait ${minutes} minute(s)`, async ({ page }) => {
    // given: a server that refuses signing in for too many tries
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

    // when: an author signs in
    await signIn(page, ADA.email, HER_PASSWORD);

    // then: she is told how long to wait
    await expect(page.getByRole("alert")).toHaveText(said);
    // and: she stays on signing in
    await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  });
}
