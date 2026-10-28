import { test, expect, type Page, type Route } from "@playwright/test";

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

async function signedInFollowingTheLink(page: Page, answered: (route: Route) => Promise<void>) {
  const server = await aServerWithAccounts(page);
  const taken: string[] = [];
  await page.route(
    (url) => url.pathname === "/api/accounts/current/invitations",
    (route) => {
      taken.push(route.request().postDataJSON().token);
      return answered(route);
    },
  );
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page, "Ada Lovelace")).toBeVisible();
  await page.goto(`/invitation#${THE_INVITATION}`);

  return { server, taken };
}

test("a link opened while signed in is accepted there, and leads to one's projects", async ({
  page,
}) => {
  const { server, taken } = await signedInFollowingTheLink(page, (route) =>
    route.fulfill({ status: 204 }),
  );

  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  await expect(page.getByRole("main")).toContainText(
    "You are signed in as ada@example.com.",
  );
  await expect(page.getByLabel("Your name")).toHaveCount(0);
  await page.getByRole("button", { name: "Accept" }).click();

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("status").filter({ hasText: "Invitation accepted." })).toBeVisible();
  expect(taken).toEqual([THE_INVITATION]);
  expect(server.signedInAs).toBeNull();
});

test("a link for another address, opened signed in, says whose it is and stays", async ({
  page,
}) => {
  await signedInFollowingTheLink(page, (route) =>
    route.fulfill({
      status: 403,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "invitation-for-another-address",
        status: 403,
        detail: "invitation-for-another-address",
        to: "grace@example.com",
      }),
    }),
  );

  await page.getByRole("button", { name: "Accept" }).click();

  await expect(page.getByRole("alert")).toHaveText(
    "This invitation was sent to grace@example.com. Sign in with that address to take it up.",
  );
  await expect(page.getByRole("heading", { name: "You have been invited" })).toBeVisible();
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

test("the word under the invitation's heading stands apart from the first field", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto(`/invitation#${THE_INVITATION}`);

  const hint = await page
    .getByText("Choose your name and a password.")
    .boundingBox();
  const field = await page
    .locator(".field > span")
    .filter({ hasText: "Your name" })
    .boundingBox();
  const between = await page
    .locator(".field > span")
    .filter({ hasText: /^Password$/ })
    .boundingBox();
  const name = await page.getByLabel("Your name").boundingBox();
  expect(hint && field && between && name).toBeTruthy();

  expect(
    field!.y - (hint!.y + hint!.height),
    "as much room as between one field and the next",
  ).toBeGreaterThanOrEqual(between!.y - (name!.y + name!.height));
});

test("the invitation's page without a token, signed in, has nothing to accept", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page, "Ada Lovelace")).toBeVisible();

  await page.goto("/invitation");

  await expect(page.getByRole("heading", { name: "You have been invited" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Accept" })).toHaveCount(0);
});
