import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_APP_CODE,
  THE_NEW_RECOVERY_CODES,
  THE_RECOVERY_CODES,
  aServerWithAccounts,
  signIn,
  type Server,
} from "./support/accounts";

const accountButton = (page: Page) =>
  page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" });

async function withASecondFactor(page: Page): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.totpOffered = true;
  server.secondFactor = {
    since: "2026-11-06T22:14:20Z",
    recovery_codes_left: 10,
    answered_by_totp: true,
    answered_by_key: false,
  };
  await page.goto("/");

  return server;
}

async function owingTheSecondStep(page: Page): Promise<Server> {
  const server = await withASecondFactor(page);
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByLabel("The code your app shows for Weaveling")).toBeVisible();

  return server;
}

async function answeredWithARecoveryCode(page: Page, code: string) {
  await page.getByRole("button", { name: "Use another way" }).click();
  await page.getByRole("button", { name: /^Recovery code/ }).click();
  await page.getByLabel("One of your recovery codes").fill(code);
  await page.getByRole("button", { name: "Confirm" }).click();
}

test("a password alone asks for the app's code, and the code signs in once it is whole", async ({
  page,
}) => {
  // given: an author with two-step sign-in
  await withASecondFactor(page);

  // when: they sign in with their password
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: they are asked for the code their app shows, still signing in as themselves
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(page.getByText(ADA.email)).toBeVisible();
  // when: they type the six digits
  await page.getByLabel("The code your app shows for Weaveling").fill(THE_APP_CODE);
  // then: they are signed in
  await expect(accountButton(page)).toBeVisible();
});

test("a wrong code is told and the boxes are emptied for the next", async ({ page }) => {
  // given: an author owing the second step
  await owingTheSecondStep(page);

  // when: they type a code that is not the app's
  await page.getByLabel("The code your app shows for Weaveling").fill("000000");

  // then: they are told so
  await expect(page.getByText("That is not the code your app shows.")).toBeVisible();
  // and: the boxes are empty again
  await expect(page.getByLabel("The code your app shows for Weaveling")).toHaveValue("");
});

test("a recovery code signs in instead, and a note says how many are left and how to set up anew", async ({
  page,
}) => {
  // given: an author owing the second step
  await owingTheSecondStep(page);

  // when: they use a recovery code, typed in capitals without its dash
  await answeredWithARecoveryCode(page, THE_RECOVERY_CODES[0].replace("-", "").toUpperCase());

  // then: they are signed in
  await expect(accountButton(page)).toBeVisible();
  // and: a note says nine are left, with the way to set up anew
  const note = page.getByRole("status").filter({ hasText: "recovery code" });
  await expect(note).toContainText("9 are left");
  await expect(note.getByRole("link", { name: "Set up two-step sign-in again" })).toBeVisible();
  // when: they dismiss it
  await note.getByRole("button", { name: "Dismiss" }).click();
  // then: it is gone
  await expect(note).toHaveCount(0);
});

test("a recovery code used once is refused the second time", async ({ page }) => {
  // given: an author who signed in by a recovery code and signed out again
  const server = await owingTheSecondStep(page);
  await answeredWithARecoveryCode(page, THE_RECOVERY_CODES[0]);
  await expect(accountButton(page)).toBeVisible();
  server.signedIn = false;
  await page.reload();
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: they use the same code again
  await answeredWithARecoveryCode(page, THE_RECOVERY_CODES[0]);

  // then: it is refused
  await expect(
    page.getByText("That is not one of your recovery codes, or it was used already."),
  ).toBeVisible();
});

test("the last recovery code turns the note into an alert", async ({ page }) => {
  // given: an author owing the second step, with one recovery code left
  const server = await owingTheSecondStep(page);
  server.recoveryCodes = [THE_RECOVERY_CODES[9]];

  // when: they use it
  await answeredWithARecoveryCode(page, THE_RECOVERY_CODES[9]);

  // then: they are told urgently that it was the last
  await expect(page.getByRole("alert")).toContainText("That was your last recovery code.");
});

test("coming back while the second step is owed asks for it again", async ({ page }) => {
  // given: an author owing the second step
  await owingTheSecondStep(page);

  // when: they reload the page
  await page.reload();

  // then: they are asked for the code again, not for the password
  await expect(page.getByLabel("The code your app shows for Weaveling")).toBeVisible();
  await expect(page.getByLabel("Password")).toHaveCount(0);
});

test("someone else at the second step goes back to signing in", async ({ page }) => {
  // given: an author owing the second step
  const server = await owingTheSecondStep(page);

  // when: they say it is not them
  await page.getByRole("button", { name: "not you?" }).click();

  // then: they are back at the password
  await expect(page.getByLabel("Password")).toBeVisible();
  // and: the session that owed the step is ended
  await expect.poll(() => server.owing).toBe(false);
});

test("a second step that ran out goes back to signing in, saying why", async ({ page }) => {
  // given: an author owing the second step, whose session ran out meanwhile
  const server = await owingTheSecondStep(page);
  server.owing = false;

  // when: they type the code
  await page.getByLabel("The code your app shows for Weaveling").fill(THE_APP_CODE);

  // then: they are back at the password, told it took too long
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(page.getByText("Signing in took too long.")).toBeVisible();
});

test("new recovery codes are drawn for the password, and shown once", async ({ page }) => {
  // given: an author signed in with two-step sign-in, on its page
  await owingTheSecondStep(page);
  await page.getByLabel("The code your app shows for Weaveling").fill(THE_APP_CODE);
  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Security" }).click();

  // when: they draw new codes with their password
  await page.getByRole("button", { name: "Create new codes" }).click();
  const asking = page.getByRole("main").locator("form");
  await asking.getByLabel("Your password, to be sure it’s you").fill(HER_PASSWORD);
  await asking.getByRole("button", { name: "Create new codes" }).click();

  // then: the new codes are shown, the old ones said to be over
  await expect(
    page.getByText("New recovery codes were created. The old ones no longer work."),
  ).toBeVisible();
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(THE_NEW_RECOVERY_CODES);
});
