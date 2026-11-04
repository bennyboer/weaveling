import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, sceneCatalogued } from "./support/shell";
import { routingLive, subscribing } from "./support/live";

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

async function inspectedFollowingLive(page: Page, idea: string) {
  const followed = subscribing(page, "appearances");
  await inspect(page, idea);
  await followed;
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
  const followed = subscribing(page, "appearances");
  await inspect(page, idea);
  await followed;
  await expect(page.getByRole("heading", { name: "A girl in a wood" })).toBeVisible();
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return asks;
}

test("a section another author notes the idea in shows in its inspector, named, without asking again", async ({
  page,
}) => {
  // given: an idea in no section, inspected live
  await aNewProject(page, "AppearingLive");
  const idea = await anIdea(page, "A girl in a wood");
  const asks = await inspectedLive(page, idea);
  await expect(page.getByText("Not in the book yet")).toBeVisible();

  // when: another author notes it in a section
  await noteInASection(page, idea, "Chapter one");

  // then: the section shows in its inspector, named
  await expect(appearances(page).getByRole("link", { name: "Chapter one" })).toBeVisible();
  // and: nothing is asked for again
  expect(asks()).toBe(1);
});

test("a scene that no longer draws on the idea leaves its inspector", async ({ page }) => {
  // given: an idea a scene draws on, inspected live
  await aNewProject(page, "UnlinkedLive");
  const idea = await anIdea(page, "A girl in a wood");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("link", { name: "The clearing" })).toBeVisible({
      timeout: 1_000,
    });
  }).toPass();
  await inspectedFollowingLive(page, idea);
  await expect(appearances(page).getByRole("link", { name: "The clearing" })).toBeVisible();

  // when: the scene no longer draws on it
  const unlinked = await page.request.delete(`${API}/scenes/${scene}/ideas/${idea}`);
  expect(unlinked.ok()).toBe(true);

  // then: the idea is in the book nowhere
  await expect(page.getByText("Not in the book yet")).toBeVisible();
});

test("a scene deleted leaves the inspector of every idea it drew on", async ({ page }) => {
  // given: an idea noted in a section and drawn on by a scene, inspected live
  await aNewProject(page, "DeletedLive");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("listitem")).toHaveCount(2, { timeout: 1_000 });
  }).toPass();
  await inspectedFollowingLive(page, idea);
  await expect(appearances(page).getByRole("listitem")).toHaveCount(2);

  // when: the scene is deleted
  const deleted = await page.request.delete(`${API}/scenes/${scene}`);
  expect(deleted.ok()).toBe(true);

  // then: the scene leaves the inspector
  await expect(appearances(page).getByRole("link", { name: "The clearing" })).toHaveCount(0);
  // and: the section stays
  await expect(appearances(page).getByRole("link", { name: "Chapter one" })).toBeVisible();
});

test("places asked for and answered after newer ones were told are let go", async ({
  page,
}) => {
  // given: an idea noted in a section and drawn on by a scene
  await aNewProject(page, "AppearingOvertaken");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  const scene = await linkFromAScene(page, idea, "The clearing");
  // and: its places asked for and not yet answered
  const live = await routingLive(page, { withheld: ["appearances"] });
  const tell = (places: object[]) =>
    live.tell("appearances", { of: idea, version: 1_000_000, event: { places } });
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

  // when: newer places are told
  tell([{ type: "scene", id: scene }]);
  // then: the scene shows
  await expect(appearances(page).getByRole("link", { name: "The clearing" })).toBeVisible();
  // when: the older answer arrives
  release();
  await page.waitForResponse(/\/api\/appearances\?idea=/);
  await page.waitForTimeout(200);

  // then: the scene still shows
  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
    "the answer was asked for before the places were told",
  ).toBeVisible();
});

test("a scene the inspector hears of only through the idea's places is asked for, and named", async ({
  page,
}) => {
  // given: scenes not heard of live
  await routingLive(page, { withheld: ["scenes"] });
  // and: an idea in no section, inspected live
  await aNewProject(page, "AppearingUnknownScene");
  const idea = await anIdea(page, "A girl in a wood");
  await inspectedLive(page, idea);
  await expect(page.getByText("Not in the book yet")).toBeVisible();

  // when: a scene draws on it
  await linkFromAScene(page, idea, "The clearing");

  // then: the scene shows, named
  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
    "the scenes are asked for again, since the place named one the inspector did not know",
  ).toBeVisible();
});
