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
  // given: signing up only by invitation
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";

  // when: the link is followed
  await page.goto(`/invitation#${THE_INVITATION}`);
  // then: it asks for a name and a password, not an address
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  await expect(page.getByLabel("Email address")).toHaveCount(0);
  // when: a name and a password are chosen
  await page.getByLabel("Your name").fill("Grace Hopper");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  // then: the newcomer is signed in at once
  await expect(accountButton(page, "Grace Hopper")).toBeVisible();
  await expect(page).toHaveURL(/\/$/);
  expect(server.signedIn).toBe(true);
});

test("a link that leads nowhere says so and lets the person sign in instead", async ({
  page,
}) => {
  // given: a link that leads nowhere
  await aServerWithAccounts(page);

  // when: it is followed and signed up through
  await page.goto("/invitation#nothing-anyone-was-given");
  await page.getByLabel("Your name").fill("Grace Hopper");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  // then: it says so
  await expect(page.getByRole("alert")).toContainText(
    "This invitation's link lets nobody sign up, or not any more.",
  );
  // when: signing in instead is chosen
  await page
    .getByRole("button", { name: "Already have an account? Sign in" })
    .click();
  // then: the way to sign in shows
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("an address that has an account already is told to sign in", async ({
  page,
}) => {
  // given: an invitation to an address that has an account
  const server = await aServerWithAccounts(page);
  server.invitationRefused = { status: 409, type: "email-address-has-account" };

  // when: it is signed up through
  await page.goto(`/invitation#${THE_INVITATION}`);
  await page.getByLabel("Your name").fill("Ada Lovelace");
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up" }).click();

  // then: the person is told to sign in instead
  await expect(page.getByRole("alert")).toContainText(
    "This email address already has an account. Sign in instead.",
  );
  expect(server.signedIn).toBe(false);
});

test("where signing up is by invitation, signing in offers no way to sign up", async ({
  page,
}) => {
  // given: signing up only by invitation
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";

  // when: the app is opened signed out
  await page.goto("/");

  // then: signing in offers no way to sign up
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
  // given: anyone may sign up
  await aServerWithAccounts(page);

  // when: the app is opened signed out
  await page.goto("/");

  // then: signing in offers signing up
  await expect(
    page.getByRole("button", { name: "No account yet? Sign up" }),
  ).toBeVisible();
});

async function signedInFollowingTheLink(page: Page, answered: (route: Route) => Promise<void>) {
  const server = await aServerWithAccounts(page);
  const taken: string[] = [];
  const declined: string[] = [];
  await page.route(
    (url) => url.pathname === "/api/accounts/current/invitations/accept",
    (route) => {
      taken.push(route.request().postDataJSON().token);
      return answered(route);
    },
  );
  await page.route(
    (url) => url.pathname === "/api/accounts/current/invitations/decline",
    (route) => {
      declined.push(route.request().postDataJSON().token);
      return answered(route);
    },
  );
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page, "Ada Lovelace")).toBeVisible();
  await page.goto(`/invitation#${THE_INVITATION}`);

  return { server, taken, declined };
}

test("a link opened while signed in is accepted there, and leads to one's projects", async ({
  page,
}) => {
  // given: a link opened while signed in
  const { server, taken } = await signedInFollowingTheLink(page, (route) =>
    route.fulfill({ status: 204 }),
  );

  // then: it offers to accept as the account signed in
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();
  await expect(page.getByRole("main")).toContainText(
    "You are signed in as ada@example.com.",
  );
  await expect(page.getByLabel("Your name")).toHaveCount(0);
  // when: it is accepted
  await page.getByRole("button", { name: "Accept" }).click();

  // then: the account's projects show, with a note it was accepted
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole("status").filter({ hasText: "Invitation accepted." })).toBeVisible();
  expect(taken).toEqual([THE_INVITATION]);
  expect(server.signedInAs).toBeNull();
});

test("a link for another address, opened signed in, says it is another's and stays", async ({
  page,
}) => {
  // given: a link for another address, opened while signed in
  await signedInFollowingTheLink(page, (route) =>
    route.fulfill({
      status: 403,
      contentType: "application/problem+json",
      body: JSON.stringify({
        type: "invitation-for-another-address",
        status: 403,
        detail: "invitation-for-another-address",
      }),
    }),
  );

  // when: it is accepted
  await page.getByRole("button", { name: "Accept" }).click();

  // then: it says it is another's
  await expect(page.getByRole("alert")).toHaveText(
    "This invitation was sent to another address. Sign in with the one it was sent to, to take it up.",
  );
  // and: stays
  await expect(page.getByRole("heading", { name: "You have been invited" })).toBeVisible();
});

test("the invitation's page without a token is only the way to sign in", async ({
  page,
}) => {
  // given: signed out
  await aServerWithAccounts(page);

  // when: the invitation's page is opened without a token
  await page.goto("/invitation");

  // then: only the way to sign in shows
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("the token never travels to the server in the address", async ({
  page,
}) => {
  // given: everything asked of the server watched
  await aServerWithAccounts(page);
  const asked: string[] = [];
  page.on("request", (request) => asked.push(request.url()));

  // when: the link is followed
  await page.goto(`/invitation#${THE_INVITATION}`);
  await expect(
    page.getByRole("heading", { name: "You have been invited" }),
  ).toBeVisible();

  // then: the token was never sent
  expect(asked.filter((url) => url.includes(THE_INVITATION))).toEqual([]);
});

test("the word under the invitation's heading stands apart from the first field", async ({
  page,
}) => {
  // given: the link followed
  await aServerWithAccounts(page);
  await page.goto(`/invitation#${THE_INVITATION}`);

  // then: the word under the heading stands apart from the first field
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
  // given: signed in
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page, "Ada Lovelace")).toBeVisible();

  // when: the invitation's page is opened without a token
  await page.goto("/invitation");

  // then: nothing is offered to accept
  await expect(page.getByRole("heading", { name: "You have been invited" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Accept" })).toHaveCount(0);
});

test("the note after accepting speaks the reader's language", async ({ page }) => {
  // given: a link opened while signed in, read in german
  await signedInFollowingTheLink(page, (route) => route.fulfill({ status: 204 }));
  await page.getByLabel("Language").selectOption("de");

  // when: it is accepted
  await page.getByRole("button", { name: "Annehmen" }).click();

  // then: the note says so in german
  await expect(
    page.getByRole("status").filter({ hasText: "Einladung angenommen." }),
  ).toBeVisible();
});

test("a link opened while signed in is declined there, and leads to one's projects", async ({
  page,
}) => {
  // given: a link opened while signed in
  const { taken, declined } = await signedInFollowingTheLink(page, (route) =>
    route.fulfill({ status: 204 }),
  );

  // when: it is declined
  await page.getByRole("button", { name: "Decline" }).click();

  // then: the account's projects show, with a note it was declined
  await expect(page).toHaveURL(/\/$/);
  await expect(
    page.getByRole("status").filter({ hasText: "You turned down the invitation." }),
  ).toBeVisible();
  expect(declined).toEqual([THE_INVITATION]);
  expect(taken).toEqual([]);
});

test("a link run out, opened signed in, says so on declining and stays", async ({
  page,
}) => {
  // given: a link run out, opened while signed in
  await signedInFollowingTheLink(page, (route) =>
    route.fulfill({
      status: 422,
      contentType: "application/problem+json",
      body: JSON.stringify({ type: "invitation-expired", status: 422, detail: "run out" }),
    }),
  );

  // when: it is declined
  await page.getByRole("button", { name: "Decline" }).click();

  // then: it says it has run out
  await expect(page.getByRole("alert")).toContainText(
    "This invitation has run out. Ask for a new one.",
  );
  // and: stays
  await expect(page).toHaveURL(/\/invitation/);
});

test("whoever follows the link may decline it without an account", async ({
  page,
}) => {
  // given: signed out
  const server = await aServerWithAccounts(page);

  // when: the link is followed and declined
  await page.goto(`/invitation#${THE_INVITATION}`);
  await page.getByRole("button", { name: "Decline the invitation instead" }).click();

  // then: it is declined, without an account
  await expect(page.getByRole("heading", { name: "Invitation declined" })).toBeVisible();
  await expect(page.getByRole("main")).toContainText(
    "Whoever invited you sees that, and the link signs nobody up any more.",
  );
  expect(server.declinedByLink).toEqual([THE_INVITATION]);
  expect(server.signedIn).toBe(false);
  // when: signing in is chosen
  await page.getByRole("button", { name: "Already have an account? Sign in" }).click();
  // then: the way to sign in shows
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("a link that leads nowhere says so on declining and stays to be signed up through", async ({
  page,
}) => {
  // given: a link run out
  const server = await aServerWithAccounts(page);
  server.invitationRefused = { status: 422, type: "invitation-expired" };

  // when: it is followed and declined
  await page.goto(`/invitation#${THE_INVITATION}`);
  await page.getByRole("button", { name: "Decline the invitation instead" }).click();

  // then: it says it has run out
  await expect(page.getByRole("alert")).toContainText(
    "This invitation has run out. Ask for a new one.",
  );
  // and: stays to be signed up through
  await expect(page.getByRole("heading", { name: "You have been invited" })).toBeVisible();
  expect(server.declinedByLink).toEqual([]);
});

test("declining by the link speaks the reader's language", async ({ browser }) => {
  // given: a reader in german
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  await aServerWithAccounts(page);

  // when: the link is followed and declined
  await page.goto(`/invitation#${THE_INVITATION}`);
  await page.getByRole("button", { name: "Einladung stattdessen ablehnen" }).click();

  // then: it says so in german
  await expect(page.getByRole("heading", { name: "Einladung abgelehnt" })).toBeVisible();
  await context.close();
});

for (const width of [1280, 320]) {
  test(`accepting and declining sit side by side or below each other at ${width}px`, async ({
    page,
  }) => {
    // given: a window of the given width
    await page.setViewportSize({ width, height: 800 });
    // and: a link opened while signed in
    await signedInFollowingTheLink(page, (route) => route.fulfill({ status: 204 }));

    // then: accepting and declining fit beside or below each other without overlapping
    const accept = await page.getByRole("button", { name: "Accept" }).boundingBox();
    const decline = await page.getByRole("button", { name: "Decline" }).boundingBox();
    expect(accept && decline).toBeTruthy();
    expect(decline!.x + decline!.width).toBeLessThanOrEqual(width);
    expect(
      decline!.y >= accept!.y + accept!.height || decline!.x >= accept!.x + accept!.width,
      "they do not overlap",
    ).toBe(true);
  });
}
