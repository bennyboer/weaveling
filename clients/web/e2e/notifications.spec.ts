import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  endedOn,
  subscribedTo,
  toldOfMine,
  type Server,
  type ToYou,
} from "./support/accounts";

const SHARED: ToYou = {
  id: "invitation_7",
  project: "project_1",
  project_name: "Shared",
  role: "author",
  invited_by: "Grace Hopper",
  until: "2026-11-05T08:00:00Z",
};

const UNNAMED: ToYou = {
  id: "invitation_8",
  project: "project_2",
  role: "reader",
  until: "2026-11-05T08:00:00Z",
};

const bell = (page: Page) => page.locator(".masthead .bell-button");
const notifications = (page: Page) => page.getByRole("dialog", { name: "Notifications" });

async function signedInInvitedTo(page: Page, toYou: ToYou[]): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.toYou = toYou;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);

  return server;
}

async function signedInInGermanInvitedTo(page: Page, toYou: ToYou[]): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.toYou = toYou;
  await page.goto("/");
  await page.getByLabel("E-Mail-Adresse").fill(ADA.email);
  await page.getByLabel("Passwort").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Anmelden", exact: true }).click();
  await expect(page.getByRole("heading", { name: "Anmelden" })).toHaveCount(0);

  return server;
}

test("the bell tells how many invitations wait, and each says into what, from whom and as what", async ({
  page,
}) => {
  // given: an author invited into two projects, one unnamed
  await signedInInvitedTo(page, [SHARED, UNNAMED]);

  // then: the bell counts them
  await expect(bell(page)).toHaveAccessibleName("Notifications: 2 new");
  await expect(bell(page)).toContainText("2");
  // when: it is opened
  await bell(page).click();

  // then: each says into what, from whom when known, and as what
  await expect(notifications(page).getByRole("heading", { name: "Invitations" })).toBeVisible();
  const listed = notifications(page).getByRole("listitem");
  await expect(listed).toHaveCount(2);
  await expect(listed.nth(0)).toContainText("Shared");
  await expect(listed.nth(0)).toContainText("From Grace Hopper · Author");
  await expect(listed.nth(1)).toContainText("a project");
  await expect(listed.nth(1)).toContainText("Reader");
  await expect(listed.nth(1)).not.toContainText("From");
});

test("joining from the bell takes the invitation up and says so quietly", async ({
  page,
}) => {
  // given: the bell open on an invitation
  const server = await signedInInvitedTo(page, [SHARED]);
  await bell(page).click();

  // when: it is joined
  await notifications(page).getByRole("button", { name: "Join" }).click();

  // then: it is taken up, said so quietly
  await expect(page.getByRole("status")).toContainText(
    "You joined Shared. It appears in your list in a moment.",
  );
  expect(server.answered).toEqual(["accept:invitation_7"]);
  // and: the bell has nothing new
  await expect(notifications(page)).toContainText(
    "Nothing new just now.",
  );
  await expect(bell(page)).toHaveAccessibleName("Notifications");
});

test("joining an invitation a moment before it can be taken up joins it once it can", async ({
  page,
}) => {
  // given: the bell open on an invitation
  const server = await signedInInvitedTo(page, [SHARED]);
  await bell(page).click();
  // and: the first taking up refused, as the invitation is not yet one that can be taken up
  let tries = 0;
  await page.route(
    (url) => url.pathname.endsWith("/invitations/invitation_7/accept"),
    async (route) => {
      tries += 1;
      if (tries === 1) {
        return route.fulfill({
          status: 422,
          contentType: "application/problem+json",
          body: JSON.stringify({ type: "invitation-invalid", status: 422, detail: "invitation-invalid" }),
        });
      }
      return route.fallback();
    },
  );

  // when: it is joined
  await notifications(page).getByRole("button", { name: "Join" }).click();

  // then: it is joined
  await expect(page.getByRole("status")).toContainText(
    "You joined Shared. It appears in your list in a moment.",
  );
  expect(server.answered, "listed a moment before it could be taken up, it is taken up on trying again").toEqual([
    "accept:invitation_7",
  ]);
  // and: no refusal is shown
  await expect(notifications(page).getByRole("alert")).toHaveCount(0);
});

test("declining from the bell turns the invitation down and says so quietly", async ({
  page,
}) => {
  // given: the bell open on an invitation
  const server = await signedInInvitedTo(page, [SHARED]);
  await bell(page).click();

  // when: it is declined
  await notifications(page).getByRole("button", { name: "Decline" }).click();

  // then: it is turned down, said so quietly
  await expect(page.getByRole("status")).toContainText(
    "You turned down the invitation to Shared.",
  );
  expect(server.answered).toEqual(["decline:invitation_7"]);
  // and: it leaves the bell
  await expect(notifications(page).getByRole("listitem")).toHaveCount(0);
});

test("an invitation that arrives while the bell is open shows at once", async ({
  page,
}) => {
  // given: the bell open with nothing new
  const server = await signedInInvitedTo(page, []);
  await bell(page).click();
  await expect(notifications(page)).toContainText(
    "Nothing new just now.",
  );
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);

  // when: an invitation arrives
  server.toYou = [SHARED];
  toldOfMine(server);

  // then: it shows at once
  await expect(notifications(page).getByRole("listitem")).toContainText("Shared");
  await expect(bell(page)).toHaveAccessibleName("Notifications: 1 new");
});

test("an invitation no longer there is refused in the bell and the others stay", async ({
  page,
}) => {
  // given: the bell open on two invitations, one no longer there
  const server = await signedInInvitedTo(page, [SHARED, UNNAMED]);
  await bell(page).click();
  server.toYou = [UNNAMED];

  // when: that one is joined
  await notifications(page)
    .getByRole("listitem")
    .filter({ hasText: "Shared" })
    .getByRole("button", { name: "Join" })
    .click();

  // then: it is refused
  await expect(notifications(page).getByRole("alert")).toBeVisible();
  expect(server.answered).toEqual([]);
});

test("the bell is there only while signed in, and stops following on signing out", async ({
  page,
}) => {
  // given: signed out
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  // then: there is no bell
  await expect(bell(page)).toHaveCount(0);
  // when: an author signs in
  await signIn(page, ADA.email, HER_PASSWORD);
  // then: the bell shows, following invitations
  await expect(bell(page)).toBeVisible();
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);

  // when: they sign out
  await page.getByRole("button", { name: "Account: Ada Lovelace" }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();

  // then: the bell goes and nothing is followed
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(bell(page)).toHaveCount(0);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(0);
  await expect
    .poll(() => server.liveSockets.length, {
      message: "with nothing followed, the one socket is closed",
    })
    .toBe(0);
});

test("the bell speaks the reader's language", async ({ browser }) => {
  // given: a reader in german, invited
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  const server = await signedInInGermanInvitedTo(page, [SHARED]);

  // then: the bell counts in german
  await expect(bell(page)).toHaveAccessibleName("Benachrichtigungen: 1 neu");
  // when: it is opened
  await bell(page).click();
  // then: the invitation is told in german
  await expect(page.getByRole("dialog", { name: "Benachrichtigungen" })).toContainText(
    "Von Grace Hopper · Autor:in",
  );
  // when: it is declined
  await page.getByRole("button", { name: "Ablehnen" }).click();
  // then: that is told in german
  await expect(page.getByRole("status")).toContainText(
    "Du hast die Einladung zu Shared abgelehnt.",
  );
  expect(server.answered).toEqual(["decline:invitation_7"]);
  await context.close();
});

for (const width of [1280, 320]) {
  test(`the bell's invitations fit the page at ${width}px`, async ({ page }) => {
    // given: a window of the given width
    await page.setViewportSize({ width, height: 800 });
    // and: an author invited
    await signedInInvitedTo(page, [SHARED]);

    // when: the bell is opened
    await bell(page).click();

    // then: it fits the page
    const shown = await notifications(page).boundingBox();
    expect(shown).not.toBeNull();
    expect(shown!.x).toBeGreaterThanOrEqual(0);
    expect(shown!.x + shown!.width).toBeLessThanOrEqual(width);
  });
}

test("a bell that fell behind is subscribed again at once and lists anew", async ({ page }) => {
  // given: a bell following invitations
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const [asked, listed] = [server.subscribing, server.mineListed];

  // when: an invitation arrives while it falls behind
  server.toYou = [SHARED];
  endedOn(server, "invitations", "behind");

  // then: it subscribes again at once and lists anew
  await expect.poll(() => server.subscribing).toBe(asked + 1);
  await expect.poll(() => server.mineListed).toBeGreaterThan(listed);
  await expect(bell(page)).toHaveAccessibleName("Notifications: 1 new");
});

test("a bell the server knows nothing of yet asks again a moment later", async ({ page }) => {
  // given: a bell following invitations
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const asked = server.subscribing;

  // when: the server knows nothing of it
  endedOn(server, "invitations", "not-there");

  // then: it subscribes again a moment later
  await expect.poll(() => server.subscribing, { timeout: 3000 }).toBe(asked + 1);
});

test("a bell no longer let in is not asked for again", async ({ page }) => {
  // given: a bell following invitations
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const asked = server.subscribing;

  // when: it is no longer let in
  endedOn(server, "invitations", "access-lost");

  // then: it does not subscribe again
  await page.waitForTimeout(500);
  expect(server.subscribing).toBe(asked);
});

