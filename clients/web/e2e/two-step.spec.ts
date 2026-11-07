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
} from "./support/accounts";

const accountButton = (page: Page) =>
  page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" });

const twoStepInMenu = (page: Page) =>
  page.getByRole("menu").getByRole("menuitem", { name: /Two-step sign-in/ });

async function signedIn(page: Page, { secondFactors = true } = {}): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.secondFactors = secondFactors;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();

  return server;
}

async function settingUp(page: Page) {
  await accountButton(page).click();
  await twoStepInMenu(page).click();
  await page.getByRole("button", { name: "Set up" }).click();
  await expect(page.getByRole("heading", { name: "Set up two-step sign-in" })).toBeVisible();
}

async function entered(page: Page, code: string) {
  await page.getByLabel("Code", { exact: true }).fill(code);
}

test("two-step sign-in is off in the account menu until it is set up", async ({ page }) => {
  // given: an author signed in on an instance that keeps second factors
  await signedIn(page);

  // when: they open their account
  await accountButton(page).click();

  // then: two-step sign-in shows as off
  await expect(twoStepInMenu(page)).toContainText("Off");
  // when: they follow it
  await twoStepInMenu(page).click();
  // then: its page says what it does and offers to set it up
  await expect(page.getByRole("heading", { name: "Two-step sign-in" })).toBeVisible();
  await expect(page.getByText("a password alone signs nobody in")).toBeVisible();
  await expect(page.getByRole("button", { name: "Set up" })).toBeVisible();
});

test("an instance without a key does not offer two-step sign-in", async ({ page }) => {
  // given: an author signed in on an instance that cannot keep second factors
  await signedIn(page, { secondFactors: false });

  // when: they open their account
  await accountButton(page).click();

  // then: two-step sign-in is not offered
  await expect(page.getByRole("menuitem", { name: "Sign out" })).toBeVisible();
  await expect(twoStepInMenu(page)).toHaveCount(0);
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
  // then: two-step sign-in shows as on, with all ten codes left
  await expect(page.getByText("Recovery codes left: 10 of 10")).toBeVisible();
  await expect(page.getByRole("button", { name: "Set up anew" })).toBeVisible();
  // and: the codes are not shown again
  await expect(page.getByText(THE_RECOVERY_CODES[0])).toHaveCount(0);
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
