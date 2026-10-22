import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_CODE,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";

async function signUp(page: Page, password = HER_PASSWORD) {
  await page.getByRole("button", { name: "No account yet? Sign up" }).click();
  await page.getByLabel("Your name").fill(ADA.name);
  await page.getByLabel("Email address").fill(ADA.email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign up", exact: true }).click();
}

async function enterCode(page: Page, code: string) {
  await page.getByLabel("Code").fill(code);
  await page.getByRole("button", { name: "Confirm", exact: true }).click();
}

test("signing up asks for the code sent to the address", async ({ page }) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");

  await signUp(page);

  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await expect(page.getByText(`We sent a code to ${ADA.email}.`)).toBeVisible();
  expect(server.signedUp).toEqual({ email: ADA.email, name: ADA.name });
});

test("the right code confirms the address and leads back to signing in", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  await enterCode(page, THE_CODE);

  await expect(page.getByRole("status")).toHaveText(
    "Your address is confirmed. You can sign in now.",
  );
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("a wrong code says so and stays", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  await enterCode(page, "000000");

  await expect(page.getByRole("alert")).toHaveText(
    "That is not the code we sent.",
  );
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
});

test("a new code can be asked for", async ({ page }) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  await page.getByRole("button", { name: "Send a new code" }).click();

  await expect(page.getByRole("status")).toHaveText(
    "A new code is on its way.",
  );
  expect(server.codesSent).toBe(2);
});

test("a password too short is refused with its numbers", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");

  await signUp(page, "short");

  await expect(page.getByRole("alert")).toHaveText(
    "A password needs at least 12 characters; this one has 5.",
  );
  await expect(page.getByRole("heading", { name: "Sign up" })).toBeVisible();
});

test("an address not yet confirmed can go on to enter its code", async ({
  page,
}) => {
  await aServerWithAccounts(page, { verified: false });
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  await page.getByRole("button", { name: "Enter the code" }).click();
  await enterCode(page, THE_CODE);
  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("going between signing in and up keeps the address typed", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await page.getByLabel("Email address").fill(ADA.email);

  await page.getByRole("button", { name: "No account yet? Sign up" }).click();
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  await page
    .getByRole("button", { name: "Already have an account? Sign in" })
    .click();

  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
});
