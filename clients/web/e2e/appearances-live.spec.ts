import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, sceneCatalogued } from "./support/shell";

const projectSegment = (page: Page) =>
  new URL(page.url()).pathname.split("/")[2];

const idIn = (segment: string) => segment.split("-").pop() ?? segment;

const appearances = (page: Page) =>
  page.getByRole("list", { name: "Appears in" });

async function anIdea(page: Page, title: string) {
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: idIn(projectSegment(page)), title },
  });
  expect(captured.status()).toBe(201);

  return (await captured.json()).id as string;
}

async function noteInASection(page: Page, idea: string, section: string) {
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project: idIn(projectSegment(page)) },
  });
  expect(opened.status()).toBe(200);
  const { id: outline } = await opened.json();
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { under: null, after: null, title: section },
  });
  expect(added.status()).toBe(201);
  const attached = await page.request.post(
    `${API}/outlines/${outline}/attachments`,
    {
      data: {
        attachment: { kind: "idea", id: idea },
        section: (await added.json()).section,
        after: null,
      },
    },
  );
  expect(attached.status()).toBe(200);
}

async function linkFromAScene(page: Page, idea: string, titled: string) {
  const made = await page.request.post(`${API}/scenes`, {
    data: { project: idIn(projectSegment(page)) },
  });
  expect(made.status()).toBe(201);
  const { id: scene } = await made.json();
  expect(
    (
      await page.request.patch(`${API}/scenes/${scene}`, {
        data: { title: titled },
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await page.request.post(`${API}/scenes/${scene}/ideas`, {
        data: { idea },
      })
    ).status(),
  ).toBe(200);
  await sceneCatalogued(page, scene, titled);

  return scene as string;
}

async function inspect(page: Page, idea: string) {
  await page.goto(`/projects/${projectSegment(page)}/ideas/${idea}`);
}

function askedWhere(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    if (request.method() === "GET" && new URL(request.url()).pathname === "/api/appearances") {
      times += 1;
    }
  });

  return () => times;
}

async function inspectedLive(page: Page, idea: string) {
  const asks = askedWhere(page);
  await inspect(page, idea);
  await expect(page.getByRole("heading", { name: "A girl in a wood" })).toBeVisible();
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return asks;
}

test("a section another author notes the idea in shows in its inspector, named, without asking again", async ({
  page,
}) => {
  await aNewProject(page, "AppearingLive");
  const idea = await anIdea(page, "A girl in a wood");
  const asks = await inspectedLive(page, idea);
  await expect(page.getByText("Not in the book yet")).toBeVisible();

  await noteInASection(page, idea, "Chapter one");

  await expect(appearances(page).getByRole("link", { name: "Chapter one" })).toBeVisible();
  expect(asks()).toBe(1);
});

test("a scene that no longer draws on the idea leaves its inspector", async ({ page }) => {
  await aNewProject(page, "UnlinkedLive");
  const idea = await anIdea(page, "A girl in a wood");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("link", { name: "The clearing" })).toBeVisible({
      timeout: 1_000,
    });
  }).toPass();

  const unlinked = await page.request.delete(`${API}/scenes/${scene}/ideas/${idea}`);
  expect(unlinked.ok()).toBe(true);

  await expect(page.getByText("Not in the book yet")).toBeVisible();
});

test("a scene deleted leaves the inspector of every idea it drew on", async ({ page }) => {
  await aNewProject(page, "DeletedLive");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("listitem")).toHaveCount(2, { timeout: 1_000 });
  }).toPass();

  const deleted = await page.request.delete(`${API}/scenes/${scene}`);
  expect(deleted.ok()).toBe(true);

  await expect(appearances(page).getByRole("link", { name: "The clearing" })).toHaveCount(0);
  await expect(appearances(page).getByRole("link", { name: "Chapter one" })).toBeVisible();
});

test("places asked for and answered after newer ones were told are let go", async ({
  page,
}) => {
  await aNewProject(page, "AppearingOvertaken");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  const scene = await linkFromAScene(page, idea, "The clearing");
  let tell: (places: object[]) => void = () => {};
  await page.routeWebSocket(/\/api\/appearances\/live\?idea=/, (socket) => {
    tell = (places) =>
      socket.send(JSON.stringify({ of: idea, version: 1_000_000, event: { places } }));
  });
  let release = () => {};
  const asked = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answering = false;
  await page.route(/\/api\/appearances\?idea=/, async (route) => {
    answering = true;
    await asked;
    await route.fulfill({ json: [] });
  });
  await inspect(page, idea);
  await expect.poll(() => answering).toBe(true);

  tell([{ type: "scene", id: scene }]);
  await expect(appearances(page).getByRole("link", { name: "The clearing" })).toBeVisible();
  release();
  await page.waitForResponse(/\/api\/appearances\?idea=/);
  await page.waitForTimeout(200);

  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
    "the answer was asked for before the places were told",
  ).toBeVisible();
});

test("a scene the inspector hears of only through the idea's places is asked for, and named", async ({
  page,
}) => {
  await page.routeWebSocket(/\/api\/scenes\/live\?project=/, () => {});
  await aNewProject(page, "AppearingUnknownScene");
  const idea = await anIdea(page, "A girl in a wood");
  await inspectedLive(page, idea);
  await expect(page.getByText("Not in the book yet")).toBeVisible();

  await linkFromAScene(page, idea, "The clearing");

  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
    "the scenes are asked for again, since the place named one the inspector did not know",
  ).toBeVisible();
});
