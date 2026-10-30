import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  subscribedTo,
  toldOfNotices,
  type Noticed,
  type Server,
} from "./support/accounts";

const MADE_AN_AUTHOR: Noticed = {
  id: "notice_1",
  noted_at: "2026-10-30T09:00:00Z",
  about: { kind: "role-changed", project: "project_1", role: "author", by: "account_ada" },
  names: { account_ada: "Ada Lovelace", project_1: "Shared" },
};

const MADE_A_READER: Noticed = {
  id: "notice_2",
  noted_at: "2026-10-29T09:00:00Z",
  about: { kind: "role-changed", project: "project_2", role: "reader" },
};

const bell = (page: Page) => page.locator(".masthead .bell-button");
const notifications = (page: Page) => page.getByRole("dialog", { name: "Notifications" });

async function signedInNoticed(page: Page, notices: Noticed[]): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.notices = notices;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);

  return server;
}

test("a role changed while one was away is in the bell, saying who changed it and where", async ({
  page,
}) => {
  await signedInNoticed(page, [MADE_AN_AUTHOR, MADE_A_READER]);

  await expect(bell(page)).toHaveAccessibleName("Notifications: 2 new");
  await bell(page).click();

  const roles = notifications(page).locator("section", { hasText: "Roles" });
  await expect(roles.getByRole("listitem")).toHaveText([
    /Ada Lovelace made you an author of Shared\./,
    /You are now a reader of a project\./,
  ]);
});

test.describe("in UTC", () => {
  test.use({ timezoneId: "UTC" });

  test("each notice says when it was noted", async ({ page }) => {
    await signedInNoticed(page, [MADE_AN_AUTHOR]);
    await bell(page).click();

    const when = notifications(page).getByRole("listitem").locator("time");
    await expect(when).toHaveText("30 Oct 2026, 09:00");
    await expect(when).toHaveAttribute("datetime", "2026-10-30T09:00:00Z");
  });
});

test("a notice dismissed is put away for good", async ({ page }) => {
  const server = await signedInNoticed(page, [MADE_AN_AUTHOR, MADE_A_READER]);
  await bell(page).click();

  await notifications(page)
    .getByRole("listitem")
    .filter({ hasText: "Shared" })
    .getByRole("button", { name: "Dismiss" })
    .click();

  await expect(notifications(page).getByRole("listitem")).toHaveCount(1);
  await expect(bell(page)).toHaveAccessibleName("Notifications: 1 new");
  expect(server.dismissed).toEqual(["notice_1"]);
});

test("a notice gone meanwhile is put away without a word", async ({ page }) => {
  const server = await signedInNoticed(page, [MADE_AN_AUTHOR]);
  await bell(page).click();
  server.notices = [];

  await notifications(page).getByRole("button", { name: "Dismiss" }).click();

  await expect(notifications(page).getByRole("listitem")).toHaveCount(0);
  await expect(notifications(page).getByRole("alert")).toHaveCount(0);
});

test("a notice that cannot be put away says so and stays", async ({ page }) => {
  await signedInNoticed(page, [MADE_AN_AUTHOR]);
  await page.route(
    (url) => url.pathname.endsWith("/dismissed"),
    (route) => route.fulfill({ status: 500, json: { type: "unexpected" } }),
  );
  await bell(page).click();

  await notifications(page).getByRole("button", { name: "Dismiss" }).click();

  await expect(notifications(page).getByRole("alert")).toBeVisible();
  await expect(notifications(page).getByRole("listitem")).toHaveCount(1);
});

test("a role changed while the bell is open shows at once", async ({ page }) => {
  const server = await signedInNoticed(page, []);
  await bell(page).click();
  await expect(notifications(page)).toContainText("Nothing new just now.");
  await expect.poll(() => subscribedTo(server, "notifications").length).toBe(1);

  server.notices = [MADE_AN_AUTHOR];
  toldOfNotices(server, MADE_AN_AUTHOR.id);

  await expect(notifications(page).getByRole("listitem")).toHaveText([
    /Ada Lovelace made you an author of Shared\./,
  ]);
});

test("a changed role is told in the reader's language", async ({ browser }) => {
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  const server = await aServerWithAccounts(page);
  server.notices = [MADE_AN_AUTHOR, MADE_A_READER];
  await page.goto("/");
  await page.getByLabel("E-Mail-Adresse").fill(ADA.email);
  await page.getByLabel("Passwort").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Anmelden", exact: true }).click();

  await page.locator(".masthead .bell-button").click();

  const told = page.getByRole("dialog", { name: "Benachrichtigungen" });
  await expect(told.getByRole("heading", { name: "Rollen" })).toBeVisible();
  await expect(told.getByRole("listitem")).toHaveText([
    /Ada Lovelace hat deine Rolle in Shared zu Autor:in geändert\./,
    /Deine Rolle in einem Projekt ist jetzt Leser:in\./,
  ]);
  await context.close();
});

for (const width of [320, 1280]) {
  test(`a changed role and its Dismiss fit the bell at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await signedInNoticed(page, [MADE_AN_AUTHOR]);
    await bell(page).click();

    const shown = (await notifications(page).boundingBox())!;
    const dismiss = (await notifications(page).getByRole("button", { name: "Dismiss" }).boundingBox())!;
    expect(shown.x).toBeGreaterThanOrEqual(0);
    expect(shown.x + shown.width).toBeLessThanOrEqual(width);
    expect(dismiss.x + dismiss.width).toBeLessThanOrEqual(shown.x + shown.width);
  });
}
