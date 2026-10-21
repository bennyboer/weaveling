import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { openTheWritingView, saveState, scene, wordCount } from "./support/writing";

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

function counted(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    if (request.method() === "GET" && new URL(request.url()).pathname === "/api/statistics/word-counts") {
      times += 1;
    }
  });

  return () => times;
}

test("a scene written elsewhere, whose text is not here, counts as the server counts it", async ({
  page,
}) => {
  const held = await holdingBackTheTextOfOneScene(page);
  await aNewProject(page, "CountedLive");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [later] = scenes["Part Two"];
  held.scene = later;
  const asks = counted(page);
  await openTheWritingView(page);
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("0 words in the book");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  const elsewhere = await page.context().newPage();
  await elsewhere.goto(page.url());
  await scene(elsewhere, later).click();
  await elsewhere.keyboard.type("one two three");
  await expect(saveState(elsewhere)).toHaveText("All saved");

  await expect(wordCount(page)).toHaveText("3 words in the book", { timeout: 10_000 });
  expect(asks(), "told, not asked for again").toBe(1);
});

test("a count told for the scene being written here does not stand over it", async ({
  page,
}) => {
  const held = await holdingBackTheTextOfOneScene(page);
  let tell: (counts: object[]) => void = () => {};
  await page.routeWebSocket(/\/api\/statistics\/word-counts\/live\?project=/, (socket) => {
    tell = (counts) => socket.send(JSON.stringify({ version: 1_000_000, event: { counts } }));
  });
  await aNewProject(page, "CountedHere");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = scenes["Part One"];
  const [later] = scenes["Part Two"];
  held.scene = later;
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("one two");
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("2 words in the book");

  tell([
    { scene: first, words: 99 },
    { scene: later, words: 5 },
  ]);

  await expect(
    wordCount(page),
    "the scene whose text is not here counts as told, the one written here as written",
  ).toHaveText("7 words in the book");
});

test("counts asked for and answered after newer ones were told are let go", async ({
  page,
}) => {
  const held = await holdingBackTheTextOfOneScene(page);
  let tell: (counts: object[]) => void = () => {};
  await page.routeWebSocket(/\/api\/statistics\/word-counts\/live\?project=/, (socket) => {
    tell = (counts) => socket.send(JSON.stringify({ version: 1_000_000, event: { counts } }));
  });
  await aNewProject(page, "CountedOvertaken");
  const [later] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  held.scene = later;
  let release = () => {};
  const asked = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answering = false;
  await page.route(/\/api\/statistics\/word-counts\?project=/, async (route) => {
    answering = true;
    await asked;
    await route.fulfill({ json: [{ scene: later, words: 1 }] });
  });
  await openTheWritingView(page);
  await wordCount(page).click();
  await expect.poll(() => answering).toBe(true);

  tell([{ scene: later, words: 5 }]);
  await expect(wordCount(page)).toHaveText("5 words in the book");
  release();
  await page.waitForResponse(/\/api\/statistics\/word-counts\?project=/);
  await page.waitForTimeout(200);

  await expect(
    wordCount(page),
    "the answer was asked for before the counts were told",
  ).toHaveText("5 words in the book");
});
