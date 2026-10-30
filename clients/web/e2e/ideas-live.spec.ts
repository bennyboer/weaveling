import { test, expect, type Page } from "@playwright/test";

import { cardNamed, corkboard, waitingIdeas } from "./support/board";
import { aBookShaped, aBookWithScenes } from "./support/book";
import {
  aNewProject,
  API,
  catalogued,
  openTheOutline,
  openThePool,
} from "./support/shell";
import {
  inspector,
  inspectorTab,
  openTheWritingView,
  projectOf,
  scene,
} from "./support/writing";
import { heardOn, routingLive, subscribing } from "./support/live";

const ideas = (page: Page) => page.getByRole("list", { name: "Ideas" });

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "delete",
  path: string,
  data?: object,
) {
  const done = await page.request[method](`${API}${path}`, { data });
  expect(done.ok(), await done.text()).toBe(true);

  return done;
}

async function anIdea(page: Page, title: string) {
  const captured = await anotherAuthor(page, "post", "/ideas", {
    project: projectOf(page),
    title,
  });

  return (await captured.json()).id as string;
}

function asked(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    const fromThePool = new URL(page.url()).pathname.endsWith("/ideas");
    if (
      fromThePool &&
      request.method() === "GET" &&
      /\/api\/ideas\?project=/.test(request.url())
    ) {
      times += 1;
    }
  });

  return () => times;
}

async function aPoolFollowedLive(page: Page) {
  const asks = asked(page);
  let heard = 0;
  heardOn(page, "ideas", () => {
    heard += 1;
  });
  const followed = subscribing(page, "ideas");
  await openThePool(page);
  await followed;
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return { asks, heard: () => heard };
}

test("an idea another author captures joins the pool without the ideas asked for again", async ({
  page,
}) => {
  await aNewProject(page, "PoolLiveCaptured");
  const { asks } = await aPoolFollowedLive(page);

  await anIdea(page, "The loom remembers");

  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  expect(asks()).toBe(1);
});

test("an idea another author renames or discards changes in the pool as it happens", async ({
  page,
}) => {
  await aNewProject(page, "PoolLiveChanged");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const { asks } = await aPoolFollowedLive(page);
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();

  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });
  await expect(ideas(page).getByText("The weft")).toBeVisible();
  await anotherAuthor(page, "delete", `/ideas/${loom}`);

  await expect(ideas(page).getByText("The weft")).toHaveCount(0);
  expect(asks()).toBe(1);
});

test("my own capture told back to me is not shown twice", async ({ page }) => {
  await aNewProject(page, "PoolLiveMine");
  const { heard } = await aPoolFollowedLive(page);

  await page.getByRole("textbox", { name: "What is the idea?" }).fill("The shuttle");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect.poll(heard).toBe(1);

  await expect(ideas(page).getByText("The shuttle")).toHaveCount(1);
});

test("a change missed on the way asks for the ideas again, and the pool comes out right", async ({
  page,
}) => {
  let passed = 0;
  await routingLive(page, {
    heard: {
      ideas: (_told, pass) => {
        passed += 1;
        if (passed !== 1) {
          pass();
        }
      },
    },
  });
  await aNewProject(page, "PoolLiveMissed");
  const asks = asked(page);
  await openThePool(page);
  await expect.poll(asks).toBe(1);

  const shuttle = await anIdea(page, "The shuttle");
  await anotherAuthor(page, "patch", `/ideas/${shuttle}`, { title: "The heddle" });

  await expect.poll(() => passed).toBe(2);
  await expect(
    ideas(page).getByText("The heddle"),
    "the capture that was missed is there too, so the ideas were asked for again",
  ).toBeVisible();
  expect(asks()).toBeGreaterThan(1);
});

test("a change told while the ideas are still on their way asks for them again", async ({
  page,
}) => {
  await aNewProject(page, "PoolLiveInFlight");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  let heard = 0;
  heardOn(page, "ideas", () => {
    heard += 1;
  });
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answered = false;
  await page.route(/\/api\/ideas\?project=/, async (route) => {
    const response = await route.fetch();
    if (!answered) {
      answered = true;
      await held;
    }
    await route.fulfill({ response });
  });
  await openThePool(page);
  await expect.poll(() => answered).toBe(true);

  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });
  await expect.poll(() => heard).toBe(1);
  release();

  await expect(
    ideas(page).getByText("The weft"),
    "the first answer was read before the rename, and the rename was told before it arrived",
  ).toBeVisible();
});

test("an idea another author captures waits beside the board, and their rename reaches its card", async ({
  page,
}) => {
  await aNewProject(page, "BoardIdeasLive");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const opened = await anotherAuthor(page, "post", "/boards", {
    project: projectOf(page),
  });
  const board = (await opened.json()).id as string;
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, {
    idea: loom,
    spot: { x: 400, y: 200 },
    size: { width: 168, height: 84 },
  });
  await page.reload();
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();

  await anIdea(page, "The shuttle");
  await expect(waitingIdeas(page).getByText("The shuttle")).toBeVisible();
  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });

  await expect(cardNamed(page, "The weft")).toBeVisible();
});

test("an idea noted in the outline is renamed there when another author renames it", async ({
  page,
}) => {
  await aNewProject(page, "OutlineIdeasLive");
  const { outline, ids } = await aBookShaped(page, [{ title: "Part One" }]);
  const loom = await anIdea(page, "The loom remembers");
  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "idea", id: loom },
    section: ids["Part One"],
    after: null,
  });
  await openTheOutline(page);
  await expect(page.locator(".leaf .name")).toHaveText("The loom remembers");

  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });

  await expect(page.locator(".leaf .name")).toHaveText("The weft");
});

test("an idea a scene draws on is renamed in the tray when another author renames it", async ({
  page,
}) => {
  await aNewProject(page, "TrayIdeasLive");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdea(page, "The loom remembers");
  await anotherAuthor(page, "post", `/scenes/${first}/ideas`, { idea: loom });
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();
  const drawnFrom = inspector(page).getByRole("list", { name: "Drawn from" });
  await expect(drawnFrom.getByRole("link")).toHaveText(["The loom remembers"]);

  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });

  await expect(drawnFrom.getByRole("link")).toHaveText(["The weft"]);
});

test("an idea captured on the board, told back before its answer, waits only once when unpinned", async ({
  page,
}) => {
  await aNewProject(page, "BoardIdeasOnce");
  let heard = 0;
  heardOn(page, "ideas", () => {
    heard += 1;
  });
  await page.route(/\/api\/ideas$/, async (route) => {
    const response = await route.fetch();
    await expect.poll(() => heard).toBe(1);
    await route.fulfill({ response });
  });
  await page.reload();
  const board = (await corkboard(page).boundingBox())!;
  await page.mouse.dblclick(board.x + board.width / 2, board.y + board.height / 2);
  await page.keyboard.type("The loom remembers");
  await page.keyboard.press("Enter");
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();

  await cardNamed(page, "The loom remembers").click();
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();

  await expect(
    waitingIdeas(page).getByText("The loom remembers"),
    "the frame of my own capture and the answer to it are one idea",
  ).toHaveCount(1);
});

test("a rename on the board answered after a newer one was told leaves the newer title", async ({
  page,
}) => {
  await aNewProject(page, "BoardIdeasNewer");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const opened = await anotherAuthor(page, "post", "/boards", {
    project: projectOf(page),
  });
  const board = (await opened.json()).id as string;
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, {
    idea: loom,
    spot: { x: 400, y: 200 },
    size: { width: 168, height: 84 },
  });
  let heard = 0;
  heardOn(page, "ideas", () => {
    heard += 1;
  });
  await page.route(new RegExp(`/api/ideas/${loom}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The heddle" });
    await expect.poll(() => heard).toBe(2);
    await route.fulfill({ response });
  });
  await page.reload();
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();

  await cardNamed(page, "The loom remembers").dblclick();
  await page.keyboard.type("The weft");
  await page.keyboard.press("Enter");

  await expect(cardNamed(page, "The heddle")).toBeVisible();
  await page.waitForTimeout(300);
  await expect(
    cardNamed(page, "The weft"),
    "my rename's answer is older than theirs, told before it came",
  ).toHaveCount(0);
});
