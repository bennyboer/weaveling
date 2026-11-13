import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_CODE,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";

const HER_NEW_PASSWORD = "a better horse, a better battery";

async function askToReset(page: Page, email = ADA.email) {
  await page.getByRole("button", { name: "Forgot your password?" }).click();
  await page.getByLabel("Email address").fill(email);
  await page.getByRole("button", { name: "Send a code" }).click();
}

async function setNewPassword(page: Page, code: string, password: string) {
  await page.getByLabel("Code").fill(code);
  await page.getByLabel("New password").fill(password);
  await page.getByRole("button", { name: "Set the new password" }).click();
}

test("a forgotten password is set anew by the code sent for it", async ({
  page,
}) => {
  // given: an author who forgot their password
  await aServerWithAccounts(page);
  await page.goto("/");

  // when: they ask to reset it
  await askToReset(page);
  // then: a code is sent to their address
  await expect(page.getByText(`We sent a code to ${ADA.email}.`)).toBeVisible();
  // when: they set a new password with the code
  await setNewPassword(page, THE_CODE, HER_NEW_PASSWORD);

  // then: they are told it is set, their address kept
  await expect(page.getByRole("status")).toHaveText(
    "Your new password is set. You can sign in with it now.",
  );
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  // and: it signs them in
  await signIn(page, ADA.email, HER_NEW_PASSWORD);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("an address nobody has goes on to the code as any other", async ({
  page,
}) => {
  // given: signed out
  const server = await aServerWithAccounts(page);
  await page.goto("/");

  // when: a reset is asked for an address nobody has
  await askToReset(page, "nobody@example.com");

  // then: it goes on to the code as any other
  await expect(
    page.getByText("We sent a code to nobody@example.com."),
  ).toBeVisible();
  expect(server.resetsAsked).toBe(1);
});

test("a wrong code says so and empties the boxes, keeping the password", async ({
  page,
}) => {
  // given: a reset asked for
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  // when: a new password is set with a wrong code
  await setNewPassword(page, "000000", HER_NEW_PASSWORD);

  // then: it says so, empties the code and keeps the password
  await expect(page.getByRole("alert")).toHaveText(
    "That is not the code we sent.",
  );
  await expect(page.getByLabel("Code")).toHaveValue("");
  await expect(page.getByLabel("New password")).toHaveValue(HER_NEW_PASSWORD);
});

test("a new password too short is refused, keeping the code", async ({
  page,
}) => {
  // given: a reset asked for
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  // when: a new password too short is set
  await setNewPassword(page, THE_CODE, "short");

  // then: it is refused, the code kept
  await expect(page.getByRole("alert")).toHaveText(
    "A password needs at least 12 characters; this one has 5.",
  );
  await expect(page.getByLabel("Code")).toHaveValue(THE_CODE);
});

test("a new code for the reset can be asked for", async ({ page }) => {
  // given: a reset asked for, a code half typed
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);
  await page.getByLabel("Code").fill("123");

  // when: a new code is asked for
  await page.getByRole("button", { name: "Send a new code" }).click();

  // then: it is on its way, the code box emptied
  await expect(page.getByRole("status")).toHaveText(
    "A new code is on its way.",
  );
  await expect(page.getByLabel("Code")).toHaveValue("");
  expect(server.resetsAsked).toBe(2);
});

test("the old password no longer signs in once a new one is set", async ({
  page,
}) => {
  // given: a new password set
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);
  await setNewPassword(page, THE_CODE, HER_NEW_PASSWORD);
  await expect(page.getByRole("status")).toBeVisible();

  // when: the old one is used to sign in
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: it does not sign in
  await expect(page.getByRole("alert")).toHaveText(
    "The email address or the password is wrong.",
  );
});

test("from setting a new password, the way leads back to signing in", async ({
  page,
}) => {
  // given: a reset asked for
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  // when: the way back is chosen
  await page.getByRole("button", { name: "Back to signing in" }).click();

  // then: signing in shows, the address kept
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
});
