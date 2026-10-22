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
  await aServerWithAccounts(page);
  await page.goto("/");

  await askToReset(page);
  await expect(page.getByText(`We sent a code to ${ADA.email}.`)).toBeVisible();
  await setNewPassword(page, THE_CODE, HER_NEW_PASSWORD);

  await expect(page.getByRole("status")).toHaveText(
    "Your new password is set. You can sign in with it now.",
  );
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  await signIn(page, ADA.email, HER_NEW_PASSWORD);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("an address nobody has goes on to the code as any other", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");

  await askToReset(page, "nobody@example.com");

  await expect(
    page.getByText("We sent a code to nobody@example.com."),
  ).toBeVisible();
  expect(server.resetsAsked).toBe(1);
});

test("a wrong code says so and empties the boxes, keeping the password", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  await setNewPassword(page, "000000", HER_NEW_PASSWORD);

  await expect(page.getByRole("alert")).toHaveText(
    "That is not the code we sent.",
  );
  await expect(page.getByLabel("Code")).toHaveValue("");
  await expect(page.getByLabel("New password")).toHaveValue(HER_NEW_PASSWORD);
});

test("a new password too short is refused, keeping the code", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  await setNewPassword(page, THE_CODE, "short");

  await expect(page.getByRole("alert")).toHaveText(
    "A password needs at least 12 characters; this one has 5.",
  );
  await expect(page.getByLabel("Code")).toHaveValue(THE_CODE);
});

test("a new code for the reset can be asked for", async ({ page }) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);
  await page.getByLabel("Code").fill("123");

  await page.getByRole("button", { name: "Send a new code" }).click();

  await expect(page.getByRole("status")).toHaveText(
    "A new code is on its way.",
  );
  await expect(page.getByLabel("Code")).toHaveValue("");
  expect(server.resetsAsked).toBe(2);
});

test("the old password no longer signs in once a new one is set", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);
  await setNewPassword(page, THE_CODE, HER_NEW_PASSWORD);
  await expect(page.getByRole("status")).toBeVisible();

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(page.getByRole("alert")).toHaveText(
    "This email address and password do not sign in.",
  );
});

test("from setting a new password, the way leads back to signing in", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await askToReset(page);

  await page.getByRole("button", { name: "Back to signing in" }).click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
});
