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
  await signedInInvitedTo(page, [SHARED, UNNAMED]);

  await expect(bell(page)).toHaveAccessibleName("Notifications: 2 new");
  await expect(bell(page)).toContainText("2");
  await bell(page).click();

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
  const server = await signedInInvitedTo(page, [SHARED]);
  await bell(page).click();

  await notifications(page).getByRole("button", { name: "Join" }).click();

  await expect(page.getByRole("status")).toContainText(
    "You joined Shared. It appears in your list in a moment.",
  );
  expect(server.answered).toEqual(["accept:invitation_7"]);
  await expect(notifications(page)).toContainText(
    "Nothing new just now.",
  );
  await expect(bell(page)).toHaveAccessibleName("Notifications");
});

test("declining from the bell turns the invitation down and says so quietly", async ({
  page,
}) => {
  const server = await signedInInvitedTo(page, [SHARED]);
  await bell(page).click();

  await notifications(page).getByRole("button", { name: "Decline" }).click();

  await expect(page.getByRole("status")).toContainText(
    "You turned down the invitation to Shared.",
  );
  expect(server.answered).toEqual(["decline:invitation_7"]);
  await expect(notifications(page).getByRole("listitem")).toHaveCount(0);
});

test("an invitation that arrives while the bell is open shows at once", async ({
  page,
}) => {
  const server = await signedInInvitedTo(page, []);
  await bell(page).click();
  await expect(notifications(page)).toContainText(
    "Nothing new just now.",
  );
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);

  server.toYou = [SHARED];
  toldOfMine(server);

  await expect(notifications(page).getByRole("listitem")).toContainText("Shared");
  await expect(bell(page)).toHaveAccessibleName("Notifications: 1 new");
});

test("an invitation no longer there is refused in the bell and the others stay", async ({
  page,
}) => {
  const server = await signedInInvitedTo(page, [SHARED, UNNAMED]);
  await bell(page).click();
  server.toYou = [UNNAMED];

  await notifications(page)
    .getByRole("listitem")
    .filter({ hasText: "Shared" })
    .getByRole("button", { name: "Join" })
    .click();

  await expect(notifications(page).getByRole("alert")).toBeVisible();
  expect(server.answered).toEqual([]);
});

test("the bell is there only while signed in, and stops following on signing out", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(bell(page)).toHaveCount(0);
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(bell(page)).toBeVisible();
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);

  await page.getByRole("button", { name: "Account: Ada Lovelace" }).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(bell(page)).toHaveCount(0);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(0);
  await expect
    .poll(() => server.liveSockets, { message: "with nothing followed, the one socket is closed" })
    .toBe(0);
});

test("the bell speaks the reader's language", async ({ browser }) => {
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  const server = await signedInInGermanInvitedTo(page, [SHARED]);

  await expect(bell(page)).toHaveAccessibleName("Benachrichtigungen: 1 neu");
  await bell(page).click();
  await expect(page.getByRole("dialog", { name: "Benachrichtigungen" })).toContainText(
    "Von Grace Hopper · Autor:in",
  );
  await page.getByRole("button", { name: "Ablehnen" }).click();
  await expect(page.getByRole("status")).toContainText(
    "Du hast die Einladung zu Shared abgelehnt.",
  );
  expect(server.answered).toEqual(["decline:invitation_7"]);
  await context.close();
});

for (const width of [1280, 320]) {
  test(`the bell's invitations fit the page at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await signedInInvitedTo(page, [SHARED]);

    await bell(page).click();

    const shown = await notifications(page).boundingBox();
    expect(shown).not.toBeNull();
    expect(shown!.x).toBeGreaterThanOrEqual(0);
    expect(shown!.x + shown!.width).toBeLessThanOrEqual(width);
  });
}

test("a bell that fell behind is subscribed again at once and lists anew", async ({ page }) => {
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const [asked, listed] = [server.subscribing, server.mineListed];

  server.toYou = [SHARED];
  endedOn(server, "invitations", "behind");

  await expect.poll(() => server.subscribing).toBe(asked + 1);
  await expect.poll(() => server.mineListed).toBeGreaterThan(listed);
  await expect(bell(page)).toHaveAccessibleName("Notifications: 1 new");
});

test("a bell the server knows nothing of yet asks again a moment later", async ({ page }) => {
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const asked = server.subscribing;

  endedOn(server, "invitations", "not-there");

  await expect.poll(() => server.subscribing, { timeout: 3000 }).toBe(asked + 1);
});

test("a bell no longer let in is not asked for again", async ({ page }) => {
  const server = await signedInInvitedTo(page, []);
  await expect.poll(() => subscribedTo(server, "invitations").length).toBe(1);
  const asked = server.subscribing;

  endedOn(server, "invitations", "access-lost");

  await page.waitForTimeout(500);
  expect(server.subscribing).toBe(asked);
});

