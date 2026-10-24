import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_INVITATION,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";

const accountButton = (page: Page, name: string) =>
  page.getByRole("banner").getByRole("button", { name: `Account: ${name}` });

test("whoever follows the link chooses a name and a password and is signed in at once", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";

  await page.goto(`/invitation#${THE_INVITATION}`);
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  await expect(page.getByLabel("Email address")).toHaveCount(0);
  await page.getByLabel("Your name").fill("Grace Hopper");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  await expect(accountButton(page, "Grace Hopper")).toBeVisible();
  await expect(page).toHaveURL(/\/$/);
  expect(server.signedIn).toBe(true);
});

test("a link that leads nowhere says so and lets the person sign in instead", async ({
  page,
}) => {
  await aServerWithAccounts(page);

  await page.goto("/invitation#nothing-anyone-was-given");
  await page.getByLabel("Your name").fill("Grace Hopper");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  await expect(page.getByRole("alert")).toContainText(
    "This invitation's link lets nobody sign up, or not any more.",
  );
  await page
    .getByRole("button", { name: "Already have an account? Sign in" })
    .click();
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("an address that has an account already is told to sign in", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  server.invitationRefused = { status: 409, type: "email-address-has-account" };

  await page.goto(`/invitation#${THE_INVITATION}`);
  await page.getByLabel("Your name").fill("Ada Lovelace");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  await expect(page.getByRole("alert")).toContainText(
    "This email address already has an account. Sign in instead.",
  );
  expect(server.signedIn).toBe(false);
});

test("where signing up is by invitation, signing in offers no way to sign up", async ({
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
    page.getByRole("button", { name: "No account yet? Sign up" }),
  ).toHaveCount(0);
});

test("where anyone may sign up, signing in still offers it", async ({
  page,
}) => {
  await aServerWithAccounts(page);

  await page.goto("/");

  await expect(
    page.getByRole("button", { name: "No account yet? Sign up" }),
  ).toBeVisible();
});

test("a link opened while signed in says whose session it is and how to accept", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page, "Ada Lovelace")).toBeVisible();

  await page.goto(`/invitation#${THE_INVITATION}`);

  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  await expect(page.getByRole("main")).toContainText(
    "You are already signed in as ada@example.com.",
  );
  await expect(page.getByLabel("Your name")).toHaveCount(0);
  expect(server.signedInAs).toBeNull();
});

test("the invitation's page without a token is only the way to sign in", async ({
  page,
}) => {
  await aServerWithAccounts(page);

  await page.goto("/invitation");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("the token never travels to the server in the address", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  const asked: string[] = [];
  page.on("request", (request) => asked.push(request.url()));

  await page.goto(`/invitation#${THE_INVITATION}`);
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();

  expect(asked.filter((url) => url.includes(THE_INVITATION))).toEqual([]);
});
