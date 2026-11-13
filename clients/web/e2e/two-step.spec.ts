import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_APP_CODE,
  THE_RECOVERY_CODES,
  THE_TOTP_SECRET,
  aServerWithAccounts,
  signIn,
  type Server,
  confirmedByHerPassword,
} from "./support/accounts";

const accountButton = (page: Page) =>
  page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" });

const securityInMenu = (page: Page) =>
  page.getByRole("menu").getByRole("menuitem", { name: "Security" });

async function signedIn(page: Page, { totpOffered = true } = {}): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.totpOffered = totpOffered;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();

  return server;
}

async function settingUp(page: Page) {
  await accountButton(page).click();
  await securityInMenu(page).click();
  await page.getByRole("button", { name: "Set up" }).click();
  await confirmedByHerPassword(page);
  await expect(page.getByRole("heading", { name: "Set up two-step sign-in" })).toBeVisible();
}

async function entered(page: Page, code: string) {
  await page.getByLabel("Code", { exact: true }).fill(code);
}

test("the account menu leads to the security page, which offers a second step", async ({
  page,
}) => {
  // given: an author signed in on an instance that keeps second factors
  await signedIn(page);

  // when: they open their account and follow its security
  await accountButton(page).click();
  await securityInMenu(page).click();
  // then: the security page says what a second step does and offers to set up an app
  await expect(page).toHaveURL(/\/account\/security$/);
  await expect(page.getByRole("heading", { name: "Security" })).toBeVisible();
  await expect(page.getByText("a password alone is not enough to sign in")).toBeVisible();
  await expect(page.getByRole("button", { name: "Set up an authenticator app" })).toBeVisible();
  // and: the path names the account, then the page, one step apart
  const path = page.getByRole("navigation", { name: "Path" });
  await expect(path.getByRole("listitem").nth(1)).toHaveText("Your account");
  await expect(path.getByRole("listitem").nth(2)).toHaveText("Security");
  await expect(path.locator(".between")).toHaveCount(2);
});

test("an instance without a key does not offer two-step sign-in", async ({ page }) => {
  // given: an author signed in on an instance that cannot keep second factors
  await signedIn(page, { totpOffered: false });

  // when: they open their account
  await accountButton(page).click();

  // then: no security page is offered
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toBeVisible();
  await expect(securityInMenu(page)).toHaveCount(0);
});

test("setting up shows the QR code and the key to type, and changes nothing until confirmed", async ({
  page,
}) => {
  // given: an author signed in
  await signedIn(page);

  // when: they begin setting up two-step sign-in
  await settingUp(page);

  // then: a QR code and the key in groups of four are shown
  await expect(page.getByRole("img", { name: "QR code for your authenticator app" })).toBeVisible();
  await expect(page.getByText("JBSW Y3DP EHPK 3PXP KZ4M 2QHT JBSW Y3DP")).toBeVisible();
  // when: they cancel
  await page.getByRole("button", { name: "Cancel" }).click();
  // then: two-step sign-in is still off
  await expect(page.getByRole("button", { name: "Set up" })).toBeVisible();
});

test("copying the key says it was copied, for a moment", async ({ page, context }) => {
  // given: an author setting up two-step sign-in, whose browser lets the page copy
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await signedIn(page);
  await settingUp(page);

  // when: they copy the key
  await page.getByRole("button", { name: "Copy key" }).click();

  // then: the button says it was copied
  await expect(page.getByRole("button", { name: "Copied" })).toBeVisible();
  // and: the key is on the clipboard
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(THE_TOTP_SECRET);
  // and: a moment later the button is as it was
  await expect(page.getByRole("button", { name: "Copy key" })).toBeVisible();
});

test("a code of the app turns it on and shows the recovery codes once, to be kept before going on", async ({
  page,
}) => {
  // given: an author setting up two-step sign-in
  await signedIn(page);
  await settingUp(page);

  // when: they confirm the code their app shows
  await entered(page, THE_APP_CODE);

  // then: they are told it is on and that they were signed out elsewhere
  await expect(
    page.getByText("Two-step sign-in is on. You were signed out everywhere else."),
  ).toBeVisible();
  // and: the ten recovery codes are shown
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(THE_RECOVERY_CODES);
  // and: going on waits until they say they kept them
  const done = page.getByRole("button", { name: "Done" });
  await expect(done).toBeDisabled();
  await page.getByLabel("I have kept them somewhere safe").check();
  await done.click();
  // then: the app shows as a way of answering the second step, with all ten codes left
  const ways = page.getByRole("main").getByRole("list");
  await expect(ways).toContainText("Authenticator app");
  await expect(ways).toContainText("10 of 10 left");
  await page.getByRole("button", { name: "Actions for the authenticator app" }).click();
  await expect(page.getByRole("menuitem", { name: "Set up again" })).toBeVisible();
  // and: the codes are not shown again
  await expect(page.getByText(THE_RECOVERY_CODES[0])).toHaveCount(0);
});

test("the password turns it off, and it says so", async ({ page }) => {
  // given: an author signed in with two-step sign-in on, on its page
  const server = await signedIn(page);
  server.secondFactor = {
    since: "2026-11-06T22:14:20Z",
    recovery_codes_left: 10,
    answered_by_totp: true,
    answered_by_key: false,
  };
  await page.goto("/account/security");

  // when: they turn it off with a password that is not theirs
  await page.getByRole("button", { name: "Actions for the authenticator app" }).click();
  await page.getByRole("menuitem", { name: "Turn off" }).click();
  const asking = page.getByRole("main").locator("form");
  await asking.getByLabel("Your password, to be sure it’s you").fill("not her password");
  await asking.getByRole("button", { name: "Turn off" }).click();
  // then: they are told so, and it stays on
  await expect(page.getByText("That is not your password.")).toBeVisible();
  expect(server.secondFactor).not.toBeNull();
  // when: they give their password
  await asking.getByLabel("Your password, to be sure it’s you").fill(HER_PASSWORD);
  await asking.getByRole("button", { name: "Turn off" }).click();
  // then: they are told it is off, and may set it up again
  await expect(
    page.getByText("Two-step sign-in is off. You were signed out everywhere else."),
  ).toBeVisible();
  await expect(page.getByRole("button", { name: "Set up an authenticator app" })).toBeVisible();
  expect(server.secondFactor).toBeNull();
});

test("a wrong code is told and can be entered again", async ({ page }) => {
  // given: an author setting up two-step sign-in
  await signedIn(page);
  await settingUp(page);

  // when: they enter a code that is not the app's
  await entered(page, "000000");

  // then: they are told so
  await expect(page.getByText("That is not the code your app shows.")).toBeVisible();
  // and: the boxes are empty for the next code
  await expect(page.getByLabel("Code", { exact: true })).toHaveValue("");
  // when: they enter the right one
  await entered(page, THE_APP_CODE);
  // then: it is on
  await expect(page.getByRole("heading", { name: "Keep these recovery codes" })).toBeVisible();
});

test("the author stays signed in and following live after the server renews the session", async ({
  page,
}) => {
  // given: an author following live and setting up two-step sign-in
  const server = await signedIn(page);
  await expect.poll(() => server.liveSockets.length).toBe(1);
  const first = server.liveSockets[0];
  await settingUp(page);

  // when: they confirm, and the server ends the session the socket was opened under
  await entered(page, THE_APP_CODE);

  // then: they are still signed in
  await expect(page.getByRole("heading", { name: "Keep these recovery codes" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  // and: they follow live again under the new session
  await expect
    .poll(() => server.liveSockets.filter((socket) => socket !== first).length)
    .toBe(1);
});

test("setting up asks for her password first, and a wrong one shows nothing to set up", async ({
  page,
}) => {
  // given: she is signed in
  await signedIn(page);

  // when: she begins setting up an app with a wrong password
  await accountButton(page).click();
  await securityInMenu(page).click();
  await page.getByRole("button", { name: "Set up" }).click();
  await confirmedByHerPassword(page, "not her password");

  // then: she is told it is not her password, and no code is shown
  await expect(page.getByText("That is not your password.")).toBeVisible();
  await expect(page.getByRole("heading", { name: "Set up two-step sign-in" })).toHaveCount(0);
});
