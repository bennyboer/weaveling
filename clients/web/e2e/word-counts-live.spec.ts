import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { openTheWritingView, saveState, scene, wordCount } from "./support/writing";
import { routingLive } from "./support/live";

const sceneCarried = (frame: string | Buffer) => {
  const bytes = Buffer.from(frame);
  const length = bytes[1];

  return bytes.subarray(2, 2 + length).toString();
};

async function holdingBackTheTextOfOneScene(page: Page) {
  const held = { scenes: [] as string[] };
  await page.routeWebSocket(/\/api\/scenes\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (!held.scenes.includes(sceneCarried(frame))) {
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
  held.scenes = [later];
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
  let told = 0;
  const live = await routingLive(page, { withheld: ["word-counts"] });
  const tell = (counts: object[]) => {
    for (const count of counts as { scene: string }[]) {
      told += 1;
      live.tell("word-counts", { of: count.scene, version: 1_000_000 + told, event: count });
    }
  };
  await aNewProject(page, "CountedHere");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = scenes["Part One"];
  const [later] = scenes["Part Two"];
  held.scenes = [later];
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

test("counts answered after newer ones were told keep the newer ones and take in the rest", async ({
  page,
}) => {
  const held = await holdingBackTheTextOfOneScene(page);
  let told = 0;
  const live = await routingLive(page, { withheld: ["word-counts"] });
  const tell = (counts: object[]) => {
    for (const count of counts as { scene: string }[]) {
      told += 1;
      live.tell("word-counts", { of: count.scene, version: 1_000_000 + told, event: count });
    }
  };
  await aNewProject(page, "CountedOvertaken");
  const [later, other] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  held.scenes = [later, other];
  let release = () => {};
  const asked = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answering = false;
  await page.route(/\/api\/statistics\/word-counts\?project=/, async (route) => {
    answering = true;
    await asked;
    await route.fulfill({
      json: [
        { scene: later, words: 1 },
        { scene: other, words: 4 },
      ],
    });
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
    "the other scene counts as answered, the told one as told, since the answer was asked for before",
  ).toHaveText("9 words in the book");
});
