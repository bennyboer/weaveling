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
  await page.routeWebSocket(/\/api\/scenes\/live\?project=/, (socket) => {
    stand.tell = (summaries) => {
      for (const summary of summaries) {
        told += 1;
        socket.send(
          JSON.stringify({
            of: summary.id,
            version: 1_000_000 + told,
            event: { project: "", ...summary },
          }),
        );
      }
    };
  });

  return stand;
}

function listed(page: Page): () => number {
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
  await aNewProject(page, "MarginLive");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await titled(page, first, "The arrival");
  const asks = listed(page);
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  await titled(page, first, "The loom");

  await expect(label(page, first)).toHaveText("The loom");
  expect(asks()).toBe(1);
});

test("the opening of a scene whose text is not here follows what is written elsewhere", async ({
  page,
}) => {
  const held = await holdingBackTheTextOfOneScene(page);
  await aNewProject(page, "OpeningLive");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  held.scene = first;
  await openTheWritingView(page);

  const elsewhere = await page.context().newPage();
  await elsewhere.goto(page.url());
  await scene(elsewhere, first).click();
  await elsewhere.keyboard.type("The loom stood silent");
  await expect(saveState(elsewhere)).toHaveText("All saved");

  await expect(label(page, first)).toContainText("The loom stood silent", { timeout: 10_000 });
});

test("a scene another author makes outside the book is offered on the other side", async ({
  page,
}) => {
  await aNewProject(page, "LooseLive");
  await aBookWithScenes(page, { "Part One": 1 });
  const asks = listed(page);
  await openTheWritingView(page);
  await expect.poll(asks, { message: "followed, and asked once as it connects" }).toBe(1);
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(sides(page)).toHaveCount(0);

  await aLooseScene(page);

  await expect(sides(page).getByRole("link", { name: /Not in the book/ })).toContainText("1");
});

test("an opening told for the scene being written here does not stand over it", async ({
  page,
}) => {
  const stand = await standingInForTheScenesSocket(page);
  await aNewProject(page, "OpeningHere");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("Written here");
  await expect(label(page, first)).toContainText("Written here");

  stand.tell([
    { id: first, title: "", opening: "Told from afar" },
    { id: second, title: "The second", opening: "" },
  ]);

  await expect(label(page, second)).toHaveText("The second");
  await expect(
    label(page, first),
    "what is written here opens the scene here, and the told opening is older",
  ).toContainText("Written here");
});

test("a title told while my own rename is on its way does not undo it", async ({ page }) => {
  const stand = await standingInForTheScenesSocket(page);
  await aNewProject(page, "RenameInFlight");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  await titled(page, first, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
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

  await sceneActions(page, first).click();
  await menuItem(page, "Rename").click();
  await flow(page).getByRole("textbox", { name: "Scene title" }).fill("The loom");
  await page.keyboard.press("Enter");
  await expect(label(page, first)).toHaveText("The loom");
  stand.tell([
    { id: first, title: "The arrival", opening: "" },
    { id: second, title: "The second", opening: "" },
  ]);
  await expect(label(page, second)).toHaveText("The second");

  await expect(label(page, first), "my rename is newer than what was told").toHaveText("The loom");
  release();
});

test("scenes answered after newer ones were told keep the newer ones and take in the rest", async ({ page }) => {
  const stand = await standingInForTheScenesSocket(page);
  await aNewProject(page, "ScenesOvertaken");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
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
  await openTheWritingView(page);
  await expect.poll(() => answering).toBe(true);

  stand.tell([{ id: first, title: "Told", opening: "" }]);
  await expect(label(page, first)).toHaveText("Told");
  release();
  await page.waitForResponse((response) => new URL(response.url()).pathname === "/api/scenes");
  await page.waitForTimeout(200);

  await expect(label(page, first), "the answer was asked for before the scene was told").toHaveText(
    "Told",
  );
  await expect(label(page, second), "what was not told is taken from the answer").toHaveText(
    "Answered",
  );
});

test("a scene written here and then let go takes in the openings it is told again", async ({
  page,
}) => {
  const stand = await standingInForTheScenesSocket(page);
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

  stand.tell([{ id: first, title: "", opening: "Told from afar" }]);
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await rail(page).getByRole("button", { name: "Unfold Part One" }).click();

  await expect(
    rail(page).getByRole("button", { name: "Told from afar", exact: true }),
    "a scene let go is no longer written here, so what is told is the latest",
  ).toBeVisible();
});

test("scenes asked for again while my own rename is on its way do not undo it", async ({
  page,
}) => {
  let drop = () => {};
  await page.routeWebSocket(/\/api\/scenes\/live\?project=/, (socket) => {
    drop = () => void socket.close();
  });
  await aNewProject(page, "RenameAskedAgain");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await titled(page, first, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, first)).toHaveText("The arrival");
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

  await sceneActions(page, first).click();
  await menuItem(page, "Rename").click();
  await flow(page).getByRole("textbox", { name: "Scene title" }).fill("The loom");
  await page.keyboard.press("Enter");
  await expect(label(page, first)).toHaveText("The loom");
  drop();
  await askedAgain;
  await page.waitForTimeout(200);

  await expect(
    label(page, first),
    "the server still has the old title, and my rename has not reached it yet",
  ).toHaveText("The loom");
  release();
});

test("a scene another author renames is renamed in the outline without asking again", async ({
  page,
}) => {
  await aNewProject(page, "OutlineSceneLive");
  const { outline, ids } = await aBookShaped(page, [{ title: "Part One" }]);
  const arrival = await aSceneMade(page);
  await titled(page, arrival, "The arrival");
  const attached = await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: { attachment: { kind: "scene", id: arrival }, section: ids["Part One"], after: null },
  });
  expect(attached.ok()).toBe(true);
  await sceneCatalogued(page, arrival, "The arrival");
  const asks = listed(page);
  await openTheOutline(page);
  await expect(page.locator(".leaf .name")).toHaveText("The arrival");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  await titled(page, arrival, "The loom");

  await expect(page.locator(".leaf .name")).toHaveText("The loom");
  expect(asks()).toBe(1);
});

test("a scene another author makes waits in the outline's tally", async ({ page }) => {
  await aNewProject(page, "OutlineTallyLive");
  const asks = listed(page);
  await openTheOutline(page);
  await expect.poll(asks, { message: "followed, and asked once as it connects" }).toBe(1);
  const waiting = page.getByRole("list", { name: "Scenes not in the book" });

  await aLooseScene(page);

  await expect(waiting.getByRole("listitem")).toHaveCount(1);
});

test("a scene another author renames is renamed in the inspector of an idea it draws on", async ({
  page,
}) => {
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
  const appearances = page.getByRole("list", { name: "Appears in" });
  const segment = new URL(page.url()).pathname.split("/")[2];
  await expect(async () => {
    await page.goto(`/projects/${segment}/ideas/${idea}`);
    await expect(appearances.getByRole("link", { name: "The clearing" })).toBeVisible({
      timeout: 1_000,
    });
  }).toPass();

  await titled(page, clearing, "The glade");

  await expect(appearances.getByRole("link", { name: "The glade" })).toBeVisible();
});

