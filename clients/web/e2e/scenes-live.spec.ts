import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import {
  aLooseScene,
  aNewProject,
  API,
  aSceneMade,
  openTheOutline,
  sceneCatalogued,
} from "./support/shell";
import {
  flow,
  label,
  menuItem,
  openTheWritingView,
  projectOf,
  rail,
  saveState,
  scene,
  sceneActions,
  titled,
} from "./support/writing";
import { routingLive } from "./support/live";

const sceneCarried = (frame: string | Buffer) => {
  const bytes = Buffer.from(frame);
  const length = bytes[1];

  return bytes.subarray(2, 2 + length).toString();
};

async function holdingBackTheTextOfOneScene(page: Page) {
  const held = { scene: "" };
  await page.routeWebSocket(/\/api\/scenes\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (sceneCarried(frame) !== held.scene) {
        socket.send(frame);
      }
    });
  });

  return held;
}

async function standingInForTheScenesSocket(page: Page) {
  const stand = { tell: (_summaries: { id: string; title: string; opening: string }[]) => {} };
  let told = 0;
  const live = await routingLive(page, { withheld: ["scenes"] });
  stand.tell = (summaries) => {
    for (const summary of summaries) {
      told += 1;
      live.tell("scenes", {
        of: summary.id,
        version: 1_000_000 + told,
        event: { project: "", ...summary },
      });
    }
  };

  return stand;
}

function scenesAskedFor(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    if (request.method() === "GET" && new URL(request.url()).pathname === "/api/scenes") {
      times += 1;
    }
  });

  return () => times;
}

const sides = (page: Page) => page.getByRole("navigation", { name: "Sides of the work" });

test("a scene another author renames is renamed in the margin without asking again", async ({
  page,
}) => {
  // given: a scene in the book, open in the writing view
  await aNewProject(page, "MarginLive");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await titled(page, first, "The arrival");
  // and: how often the scenes are asked for kept
  const asks = scenesAskedFor(page);
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  // when: another author renames it
  await titled(page, first, "The loom");

  // then: the margin shows the new name
  await expect(label(page, first)).toHaveText("The loom");
  // and: the scenes were not asked for again
  expect(asks()).toBe(1);
});

test("the opening of a scene whose text is not here follows what is written elsewhere", async ({
  page,
}) => {
  // given: one scene whose text is not here
  const held = await holdingBackTheTextOfOneScene(page);
  // and: the book open in the writing view
  await aNewProject(page, "OpeningLive");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  held.scene = first;
  await openTheWritingView(page);

  // when: the scene is written in another window
  const elsewhere = await page.context().newPage();
  await elsewhere.goto(page.url());
  await scene(elsewhere, first).click();
  await elsewhere.keyboard.type("The loom stood silent");
  await expect(saveState(elsewhere)).toHaveText("All saved");

  // then: its opening in the margin follows what was written there
  await expect(label(page, first)).toContainText("The loom stood silent", { timeout: 10_000 });
});

test("a scene another author makes outside the book is offered on the other side", async ({
  page,
}) => {
  // given: a book open in the writing view, with no scenes outside it
  await aNewProject(page, "LooseLive");
  await aBookWithScenes(page, { "Part One": 1 });
  // and: how often the scenes are asked for kept
  const asks = scenesAskedFor(page);
  await openTheWritingView(page);
  await expect.poll(asks, { message: "followed, and asked once as it connects" }).toBe(1);
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(sides(page)).toHaveCount(0);

  // when: another author makes a scene outside the book
  await aLooseScene(page);

  // then: the other side offers it
  await expect(sides(page).getByRole("link", { name: /Not in the book/ })).toContainText("1");
});

test("an opening told for the scene being written here does not stand over it", async ({
  page,
}) => {
  // given: scenes whose news is told by the test
  const stand = await standingInForTheScenesSocket(page);
  // and: a scene being written here
  await aNewProject(page, "OpeningHere");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("Written here");
  await expect(label(page, first)).toContainText("Written here");

  // when: an older opening for it is told along with another scene's title
  stand.tell([
    { id: first, title: "", opening: "Told from afar" },
    { id: second, title: "The second", opening: "" },
  ]);

  // then: the other scene takes its title
  await expect(label(page, second)).toHaveText("The second");
  // and: the scene being written keeps what is written here
  await expect(
    label(page, first),
    "what is written here opens the scene here, and the told opening is older",
  ).toContainText("Written here");
});

test("a title told while my own rename is on its way does not undo it", async ({ page }) => {
  // given: scenes whose news is told by the test
  const stand = await standingInForTheScenesSocket(page);
  // and: a named scene open in the writing view
  await aNewProject(page, "RenameInFlight");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  await titled(page, first, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
  // and: its renaming held on its way to the server
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(new RegExp(`/api/scenes/${first}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    await held;
    await route.fulfill({ response });
  });

  // when: i rename it
  await sceneActions(page, first).click();
  await menuItem(page, "Rename").click();
  await flow(page).getByRole("textbox", { name: "Scene title" }).fill("The loom");
  await page.keyboard.press("Enter");
  // then: the new name shows
  await expect(label(page, first)).toHaveText("The loom");
  // when: its old title is told
  stand.tell([
    { id: first, title: "The arrival", opening: "" },
    { id: second, title: "The second", opening: "" },
  ]);
  // then: news has arrived
  await expect(label(page, second)).toHaveText("The second");

  // and: my rename stands
  await expect(label(page, first), "my rename is newer than what was told").toHaveText("The loom");
  release();
});

test("scenes answered after newer ones were told keep the newer ones and take in the rest", async ({ page }) => {
  // given: scenes whose news is told by the test
  // and: a book of two scenes
  const stand = await standingInForTheScenesSocket(page);
  await aNewProject(page, "ScenesOvertaken");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  // and: the scenes held back while they are asked for
  let release = () => {};
  const asked = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answering = false;
  await page.route(
    (url) => url.pathname === "/api/scenes" && url.searchParams.has("project"),
    async (route) => {
      answering = true;
      await asked;
      await route.fulfill({
        json: {
          scenes: [
            { id: first, project: "", title: "Old", opening: "" },
            { id: second, project: "", title: "Answered", opening: "" },
          ],
          next: null,
        },
      });
    },
  );
  // when: the writing view asks for the scenes
  await openTheWritingView(page);
  await expect.poll(() => answering).toBe(true);

  // and: a newer title is told before the answer comes
  stand.tell([{ id: first, title: "Told", opening: "" }]);
  // then: the told title shows
  await expect(label(page, first)).toHaveText("Told");
  // when: the older answer arrives
  release();
  await page.waitForResponse((response) => new URL(response.url()).pathname === "/api/scenes");
  await page.waitForTimeout(200);

  // then: the told title stands
  await expect(label(page, first), "the answer was asked for before the scene was told").toHaveText(
    "Told",
  );
  // and: the other scene takes its title from the answer
  await expect(label(page, second), "what was not told is taken from the answer").toHaveText(
    "Answered",
  );
});

test("scenes asked for twice close together keep the later answer, whichever comes last", async ({ page }) => {
  // given: scenes whose following the test tells
  const live = await routingLive(page, { withheld: ["scenes"] });
  // and: a book of one scene
  await aNewProject(page, "ScenesAskedTwice");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  // and: the first asking for the scenes answered late, with what it read early
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let asked = 0;
  await page.route(
    (url) => url.pathname === "/api/scenes" && url.searchParams.has("project"),
    async (route) => {
      asked += 1;
      const title = asked === 1 ? "Older" : "Newer";
      if (asked === 1) {
        await held;
      }
      await route.fulfill({
        json: { scenes: [{ id: first, project: "", title, opening: "" }], next: null },
      });
    },
  );

  // when: the writing view asks for the scenes
  await openTheWritingView(page);
  await expect.poll(() => asked).toBe(1);
  // and: asks again once its following begins anew
  live.follow("scenes");
  // then: the later answer shows
  await expect(label(page, first)).toHaveText("Newer");

  // when: the first answer arrives
  release();
  await page.waitForResponse((response) => new URL(response.url()).pathname === "/api/scenes");
  await page.waitForTimeout(200);

  // then: the later answer stands
  await expect(label(page, first), "the first answer was read before the second").toHaveText("Newer");
});

test("a scene written here and then let go takes in the openings it is told again", async ({
  page,
}) => {
  // given: scenes whose news is told by the test
  const stand = await standingInForTheScenesSocket(page);
  // and: a scene written here and let go
  await aNewProject(page, "OpeningLetGo");
  const scenes = (await aBookWithScenes(page, { "Part One": 60 }))["Part One"];
  const [first] = scenes;
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("Written here");
  await expect(saveState(page)).toHaveText("All saved");
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));
  await expect(flow(page).locator(`.scene.waiting[data-scene="${first}"]`)).toHaveCount(1);

  // when: a new opening is told for it
  stand.tell([{ id: first, title: "", opening: "Told from afar" }]);
  // and: it is looked up in the contents
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await rail(page).getByRole("button", { name: "Unfold Part One" }).click();

  // then: the told opening shows
  await expect(
    rail(page).getByRole("button", { name: "Told from afar", exact: true }),
    "a scene let go is no longer written here, so what is told is the latest",
  ).toBeVisible();
});

test("scenes asked for again while my own rename is on its way do not undo it", async ({
  page,
}) => {
  // given: a scenes feed that can be dropped
  const live = await routingLive(page);
  const drop = () => live.end("scenes", "behind");
  // and: a named scene open in the writing view
  await aNewProject(page, "RenameAskedAgain");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await titled(page, first, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
  // and: its renaming held on its way to the server
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(new RegExp(`/api/scenes/${first}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      await route.continue();
      return;
    }
    await held;
    await route.continue();
  });
  const askedAgain = page.waitForResponse(
    (response) => new URL(response.url()).pathname === "/api/scenes",
  );

  // when: i rename it
  await sceneActions(page, first).click();
  await menuItem(page, "Rename").click();
  await flow(page).getByRole("textbox", { name: "Scene title" }).fill("The loom");
  await page.keyboard.press("Enter");
  // then: the new name shows
  await expect(label(page, first)).toHaveText("The loom");
  // when: the scenes are asked for again
  drop();
  await askedAgain;
  await page.waitForTimeout(200);

  // then: my rename stands
  await expect(
    label(page, first),
    "the server still has the old title, and my rename has not reached it yet",
  ).toHaveText("The loom");
  release();
});

test("a scene another author renames is renamed in the outline without asking again", async ({
  page,
}) => {
  // given: a named scene in the book, open in the outline
  await aNewProject(page, "OutlineSceneLive");
  const { outline, ids } = await aBookShaped(page, [{ title: "Part One" }]);
  const arrival = await aSceneMade(page);
  await titled(page, arrival, "The arrival");
  const attached = await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: { attachment: { kind: "scene", id: arrival }, section: ids["Part One"], after: null },
  });
  expect(attached.ok()).toBe(true);
  await sceneCatalogued(page, arrival, "The arrival");
  // and: how often the scenes are asked for kept
  const asks = scenesAskedFor(page);
  await openTheOutline(page);
  await expect(page.locator(".leaf .name")).toHaveText("The arrival");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  // when: another author renames it
  await titled(page, arrival, "The loom");

  // then: the outline shows the new name
  await expect(page.locator(".leaf .name")).toHaveText("The loom");
  // and: the scenes were not asked for again
  expect(asks()).toBe(1);
});

test("a scene another author makes waits in the outline's tally", async ({ page }) => {
  // given: a project open in the outline
  await aNewProject(page, "OutlineTallyLive");
  // and: how often the scenes are asked for kept
  const asks = scenesAskedFor(page);
  await openTheOutline(page);
  await expect.poll(asks, { message: "followed, and asked once as it connects" }).toBe(1);
  const waiting = page.getByRole("list", { name: "Scenes not in the book" });

  // when: another author makes a scene outside the book
  await aLooseScene(page);

  // then: it waits in the outline's tally
  await expect(waiting.getByRole("listitem")).toHaveCount(1);
});

test("a scene another author renames is renamed in the inspector of an idea it draws on", async ({
  page,
}) => {
  // given: an idea drawn on by a named scene
  await aNewProject(page, "InspectorSceneLive");
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: projectOf(page), title: "A girl in a wood" },
  });
  const idea = (await captured.json()).id as string;
  const clearing = await aSceneMade(page);
  await titled(page, clearing, "The clearing");
  const linked = await page.request.post(`${API}/scenes/${clearing}/ideas`, { data: { idea } });
  expect(linked.ok()).toBe(true);
  await sceneCatalogued(page, clearing, "The clearing");
  // and: the idea open with the scene among its appearances
  const appearances = page.getByRole("list", { name: "Appears in" });
  const segment = new URL(page.url()).pathname.split("/")[2];
  await expect(async () => {
    await page.goto(`/projects/${segment}/ideas/${idea}`);
    await expect(appearances.getByRole("link", { name: "The clearing" })).toBeVisible({
      timeout: 1_000,
    });
  }).toPass();

  // when: another author renames the scene
  await titled(page, clearing, "The glade");

  // then: the inspector shows the new name
  await expect(appearances.getByRole("link", { name: "The glade" })).toBeVisible();
});

