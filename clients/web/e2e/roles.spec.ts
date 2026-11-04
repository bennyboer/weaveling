import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  subscribedTo,
  toldOfMembers,
  type Server,
} from "./support/accounts";
import { API } from "./support/shell";

async function aProjectWithAnIdea(page: Page): Promise<string> {
  const made = await page.request.post(`${API}/projects`, {
    data: { name: `Roles ${Date.now()}`, language: "en" },
  });
  expect(made.ok()).toBe(true);
  const project = (await made.json()).id;
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project, title: "A loose thread" },
  });
  expect(captured.ok()).toBe(true);

  return project;
}

async function signedInAs(page: Page, role: string): Promise<{ server: Server; project: string }> {
  const project = await aProjectWithAnIdea(page);
  const server = await aServerWithAccounts(page);
  server.memberships = [{ project, role }];
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);

  return { server, project };
}

const views = (page: Page) => page.getByRole("navigation", { name: "Views" });

async function aBoardAndABook(page: Page, project: string) {
  const idea = (
    await (await page.request.get(`${API}/ideas?project=${project}`)).json()
  )[0].id;
  const board = await (await page.request.post(`${API}/boards`, { data: { project } })).json();
  const pinned = await page.request.post(`${API}/boards/${board.id}/ideas`, {
    data: { idea, spot: { x: 100, y: 100 }, size: { width: 168, height: 84 } },
    headers: { "If-Match": `"${board.version}"` },
  });
  expect(pinned.ok(), await pinned.text()).toBe(true);
  const outline = (await (await page.request.post(`${API}/outlines`, { data: { project } })).json()).id;
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { title: "Chapter One", under: null, after: null },
  });
  const section = (await added.json()).section;
  const second = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { title: "Chapter Two", under: null, after: section },
  });
  expect(second.status(), await second.text()).toBe(201);
  const stray = await page.request.post(`${API}/ideas`, { data: { project, title: "A stray thought" } });
  expect(stray.ok()).toBe(true);
  const attached = await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: { attachment: { kind: "idea", id: idea }, section, after: null },
  });
  expect(attached.ok(), await attached.text()).toBe(true);
}

test("a reader is shown no way to change the project, and no writing view", async ({ page }) => {
  // given: a reader signed in
  const { project } = await signedInAs(page, "reader");
  // and: a project with a board and a book
  await aBoardAndABook(page, project);
  // and: every change asked of the server recorded
  const changes: string[] = [];
  page.on("request", (request) => {
    if (request.method() !== "GET" && /\/api\/((boards|outlines)\/.|ideas)/.test(request.url())) {
      changes.push(`${request.method()} ${request.url()}`);
    }
  });

  // when: she opens the writing view
  await page.goto(`/projects/${project}/write`);

  // then: she is taken to the board instead
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/board$`));
  // and: the views offer no writing
  await expect(views(page).getByRole("link", { name: "Board" })).toBeVisible();
  await expect(views(page).getByRole("link", { name: "Write" })).toHaveCount(0);
  // and: nothing on the board can be renamed, captured, moved or unpinned
  const card = page.getByRole("article", { name: "A loose thread" });
  await expect(card).toBeVisible();
  await expect(page.locator(".pinned .grip").first()).toBeHidden();
  await card.dblclick();
  await expect(page.locator(".board textarea"), "no name to change").toHaveCount(0);
  await page.locator(".corkboard").dblclick({ position: { x: 600, y: 420 } });
  await expect(page.locator(".board textarea"), "no idea to capture").toHaveCount(0);
  await card.press("Enter");
  await card.press("ArrowRight");
  const at = (await card.boundingBox())!;
  await page.mouse.move(at.x + 20, at.y + 20);
  await page.mouse.down();
  await page.mouse.move(at.x + 120, at.y + 90, { steps: 5 });
  await page.mouse.up();
  await card.click();
  await expect(page.getByRole("button", { name: "Open A loose thread" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Rename A loose thread" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Unpin A loose thread" })).toHaveCount(0);
  await expect(page.getByLabel("Idea name")).not.toBeEditable();

  // when: she turns to the outline
  await views(page).getByRole("link", { name: "Outline" }).click();
  // then: nothing in the book can be added, renamed, moved or placed
  await expect(page.getByRole("button", { name: "Add a section" })).toBeDisabled();
  const title = page.getByRole("textbox", { name: "Section Chapter One" });
  await expect(title).not.toBeEditable();
  await title.press("Enter");
  await title.press("Tab");
  await title.press("Alt+ArrowDown");
  await page.getByRole("textbox", { name: "Section Chapter Two" }).press("Alt+ArrowUp");
  await expect(page.getByRole("button", { name: "Place A stray thought" })).toBeDisabled();
  await expect(page.getByRole("toolbar", { name: "Actions for Chapter One" })).toHaveCount(0);
  await expect(page.locator(".outline .grip")).toHaveCount(0);
  await expect(page.getByRole("button", { name: /^Take .* out of the book$/ })).toHaveCount(0);

  // when: she turns to the ideas
  await views(page).getByRole("link", { name: "Ideas" }).click();
  // then: she sees them but cannot capture one
  await expect(page.getByRole("link", { name: "A loose thread" })).toBeVisible();
  await expect(page.getByPlaceholder("What is the idea?")).toHaveCount(0);
  // and: she has asked no change of the server
  expect(changes, "a reader asks no change of the server").toEqual([]);
});

test("a reader may not pin an idea waiting beside the board", async ({ page }) => {
  // given: a reader signed in
  const { project } = await signedInAs(page, "reader");

  // when: she opens the board
  await page.goto(`/projects/${project}/board`);

  // then: the idea waiting beside it cannot be pinned
  await expect(page.getByRole("button", { name: "Pin A loose thread" })).toBeDisabled();
});

test("an author writes and captures, and sees the project's details without changing them", async ({
  page,
}) => {
  // given: an author signed in
  const { project } = await signedInAs(page, "author");

  // when: she opens the ideas
  await page.goto(`/projects/${project}/ideas`);

  // then: she may capture and write
  await expect(page.getByPlaceholder("What is the idea?")).toBeVisible();
  await expect(views(page).getByRole("link", { name: "Write" })).toBeVisible();
  // and: the project details are shown but cannot be changed
  await page.getByRole("button", { name: "Project details" }).click();
  const details = page.getByRole("dialog", { name: "Project details" });
  await expect(details.getByLabel("Title")).toBeDisabled();
  await expect(details.getByRole("button", { name: "Save" })).toHaveCount(0);
});

test("only an owner may rename or delete a project from the list", async ({ page }) => {
  // given: an author of one project and owner of another
  const { project } = await signedInAs(page, "author");
  const owned = await aProjectWithAnIdea(page);
  const server = await aServerWithAccounts(page);
  server.memberships = [
    { project, role: "author" },
    { project: owned, role: "owner" },
  ];
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: only the owned project offers more actions in the list
  const rowOf = (id: string) => page.locator(`li:has(a[href*="${id}"])`);
  await expect(rowOf(owned).getByRole("button", { name: "More actions" })).toBeVisible();
  await expect(rowOf(project).getByRole("button", { name: "More actions" })).toHaveCount(0);
});

test("a role changed while one is online is told, and what one may do follows it", async ({
  page,
}) => {
  // given: a reader signed in, looking at the ideas
  const { server, project } = await signedInAs(page, "reader");
  await page.goto(`/projects/${project}/ideas`);
  await expect(page.getByPlaceholder("What is the idea?")).toHaveCount(0);
  await expect
    .poll(() => subscribedTo(server, "members").filter((one) => one.about.mine === "true").length)
    .toBeGreaterThan(0);

  // when: her role is changed to author while she is online
  server.memberships = [{ project, role: "author" }];
  toldOfMembers(server, project, { name: "ROLE_CHANGED", account: ADA.account, to: "author" });

  // then: she is told of her new role
  await expect(page.getByRole("status")).toContainText("Your role has been changed to Author.");
  // and: she may capture and write
  await expect(page.getByPlaceholder("What is the idea?")).toBeVisible();
  await expect(views(page).getByRole("link", { name: "Write" })).toBeVisible();
});
