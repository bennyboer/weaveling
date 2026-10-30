import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  endedOn,
  subscribedTo,
  toldOfMembers,
} from "./support/accounts";
import { API } from "./support/shell";
import { withAccounts } from "./support/signed-in";
import { watchingLive } from "./support/live";

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
    .poll(() => subscribedTo(server, "members").length, {
      message: "the list and one's roles follow the members",
    })
    .toBe(2);

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
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
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
  test.skip(withAccounts, "the server these tests run against keeps accounts");
  const followed = watchingLive(page);

  await page.goto("/");
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect.poll(() => followed.topics()).toContain("projects");

  expect(followed.topics()).not.toContain("members");
});

test("the list is asked for once on coming in, and again when the members' channel comes back", async ({
  page,
}) => {
  let asked = 0;
  page.on("request", (request) => {
    if (
      request.method() === "GET" &&
      new URL(request.url()).pathname === "/api/projects"
    ) {
      asked += 1;
    }
  });
  const server = await signedIn(page);
  await expect.poll(() => asked).toBe(1);
  await page.evaluate(() => new Promise((settled) => setTimeout(settled, 300)));
  expect(
    asked,
    "the members' channel opening the first time asks for nothing",
  ).toBe(1);

  const asking = server.subscribing;
  endedOn(server, "members", "behind");

  await expect
    .poll(() => server.subscribing, {
      message: "the list and one's roles follow the members again",
    })
    .toBe(asking + 2);
  await expect
    .poll(() => asked, {
      message: "what was missed meanwhile is caught up by listing again",
    })
    .toBe(2);
});
