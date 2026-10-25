import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  toldOfMembers,
} from "./support/accounts";
import { API } from "./support/shell";

const rowFor = (page: Page, title: string) =>
  page.getByRole("listitem").filter({ hasText: title });

async function aProject(
  page: Page,
  named: string,
): Promise<{ id: string; title: string }> {
  const title = `${named} ${Date.now()}`;
  const made = await page.request.post(`${API}/projects`, {
    data: { name: title, language: "en" },
  });
  expect(made.ok()).toBe(true);
  const { id } = await made.json();

  return { id, title };
}

async function listedWithout(page: Page, hidden: string) {
  await page.route(
    (url) => url.pathname === "/api/projects",
    async (route) => {
      if (route.request().method() !== "GET") {
        return route.fallback();
      }
      const answered = await route.fetch();
      const listed = (await answered.json()).filter(
        (project: { id: string }) => project.id !== hidden,
      );
      return route.fulfill({ response: answered, json: listed });
    },
  );
}

async function signedIn(page: Page) {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect
    .poll(() => server.memberships.length, {
      message: "the list follows the members",
    })
    .toBe(1);

  return server;
}

test("a project one is let into joins the list, though it is not found at first", async ({
  page,
}) => {
  const { id, title } = await aProject(page, "Let in");
  await listedWithout(page, id);
  let missing = 2;
  await page.route(
    (url) => url.pathname === `/api/projects/${id}`,
    (route) =>
      missing-- > 0
        ? route.fulfill({
            status: 404,
            contentType: "application/problem+json",
            body: JSON.stringify({
              type: "not-found",
              status: 404,
              detail: "not yet",
            }),
          })
        : route.fallback(),
  );
  const server = await signedIn(page);
  await expect(page.getByRole("heading", { name: "Your projects" })).toBeVisible();
  await expect(rowFor(page, title)).toHaveCount(0);

  toldOfMembers(server, id, {
    name: "JOINED",
    account: ADA.account,
    role: "author",
  });

  await expect(
    rowFor(page, title),
    "asked again until the project answers, as the list of members may be a moment behind",
  ).toBeVisible();
});

test("a project one is let out of leaves the list", async ({ page }) => {
  const { id, title } = await aProject(page, "Let out");
  const server = await signedIn(page);
  await expect(rowFor(page, title)).toBeVisible();

  toldOfMembers(server, id, { name: "REMOVED", account: ADA.account });

  await expect(rowFor(page, title)).toHaveCount(0);
});

test("a new role changes nothing in the list", async ({ page }) => {
  const kept = await aProject(page, "Kept");
  const gone = await aProject(page, "Gone");
  const server = await signedIn(page);
  await expect(rowFor(page, kept.title)).toBeVisible();
  await expect(rowFor(page, gone.title)).toBeVisible();

  toldOfMembers(server, kept.id, {
    name: "ROLE_CHANGED",
    account: ADA.account,
    to: "reader",
  });
  toldOfMembers(server, "project_unheard_of", {
    name: "LEFT",
    account: ADA.account,
  });
  toldOfMembers(server, gone.id, { name: "LEFT", account: ADA.account });

  await expect(
    rowFor(page, gone.title),
    "what came after was taken in",
  ).toHaveCount(0);
  await expect(rowFor(page, kept.title)).toBeVisible();
});

test("without accounts nobody follows the members", async ({ page }) => {
  const followed: string[] = [];
  page.on("websocket", (socket) =>
    followed.push(new URL(socket.url()).pathname),
  );

  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Your projects" })).toBeVisible();
  await expect.poll(() => followed).toContain("/api/projects/live");

  expect(followed).not.toContain("/api/members/live");
});
