import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject, API, views } from "./support/shell";
import { backToWhereILeftOff, cataloguedOpening, flow, holdingWhatIsWritten, label, openTheWritingView, saveState, scene, wordCount } from "./support/writing";

const sceneCarried = (frame: string | Buffer) => {
  const bytes = Buffer.from(frame);
  const length = bytes[1];

  return bytes.subarray(2, 2 + length).toString();
};

async function aLongBookWrittenAtItsStart(page: Page, named: string) {
  await aNewProject(page, named);
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("She had not touched it since spring.");

  return { first, last: scenes["Part One"][59] };
}

const toTheEnd = (page: Page) =>
  page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));

const nobodyWriting = (page: Page) =>
  page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());

test("a scene whose text has not arrived still counts, by what the server last counted", async ({
  page,
}) => {
  await aNewProject(page, "CountedByTheServer");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [later] = scenes["Part Two"];
  await openTheWritingView(page);
  await scene(page, later).click();
  await page.keyboard.type("one two three");
  await expect(saveState(page)).toHaveText("All saved");
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  await expect
    .poll(
      async () => {
        const counted = await (
          await page.request.get(
            `${API}/statistics/word-counts?project=${project}`,
          )
        ).json();
        return counted.find((held: { scene: string }) => held.scene === later)
          ?.words;
      },
      {
        message: "the server counts what it stores, and statistics keeps it",
        timeout: 10_000,
      },
    )
    .toBe(3);

  await page.routeWebSocket(/\/api\/scenes\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (sceneCarried(frame) !== later) {
        socket.send(frame);
      }
    });
  });
  await page.reload();
  await expect(scene(page, later)).toBeVisible();
  await wordCount(page).click();

  await expect(wordCount(page)).toHaveText("3 words in the book");
});

test("what the server last counted never stands over what is being written", async ({
  page,
}) => {
  await aNewProject(page, "CountedLate");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("one two three");
  await expect(saveState(page)).toHaveText("All saved");
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  await expect
    .poll(
      async () =>
        (
          await (
            await page.request.get(
              `${API}/statistics/word-counts?project=${project}`,
            )
          ).json()
        )[0]?.words,
      { timeout: 10_000 },
    )
    .toBe(3);

  let answered = false;
  await page.route("**/api/statistics/word-counts*", async (route) => {
    const counted = await route.fetch();
    await new Promise((resolve) => setTimeout(resolve, 1_500));
    await route.fulfill({ response: counted });
    answered = true;
  });
  await page.reload();
  await scene(page, only).click();
  await expect(scene(page, only)).toBeFocused();
  await page.keyboard.press("End");
  await page.keyboard.type(" four");
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");

  await expect.poll(() => answered).toBe(true);
  await page.waitForTimeout(300);
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");
});

test("a scene far from the window waits without an editor, and gets one as it comes near", async ({
  page,
}) => {
  const joined = new Set<string>();
  page.on("websocket", (socket) =>
    socket.on("framesent", ({ payload }) => {
      if (Buffer.from(payload)[0] === 0) {
        joined.add(sceneCarried(payload));
      }
    }),
  );
  await aNewProject(page, "FarAndNear");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await expect(scene(page, scenes["Part One"][0])).toBeVisible();

  const waiting = flow(page).locator(`.scene.waiting[data-scene="${last}"]`);
  await expect(waiting).toHaveCount(1);
  await expect(scene(page, last)).toHaveCount(0);
  expect(
    joined.has(last),
    "a scene nobody can see has no editor and is not synced",
  ).toBe(false);
  expect(joined.size).toBeGreaterThan(0);

  await waiting.scrollIntoViewIfNeeded();

  await expect(scene(page, last)).toBeVisible();
  await expect(waiting).toHaveCount(0);
});

test("landing where the author left off reaches a scene far down the book", async ({
  page,
}) => {
  await aNewProject(page, "FarLanding");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, last).click();
  await page.keyboard.type("Far down.");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();

  await backToWhereILeftOff(page);
  await expect(scene(page, last)).toBeFocused();
});

test("a scene coming near is named in the margin before its text arrives", async ({
  page,
}) => {
  await aNewProject(page, "FarNamed");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first] = scenes["Part One"];
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, last).click();
  await page.keyboard.type("The far scene speaks.");
  await expect(saveState(page)).toHaveText("All saved");
  await expect
    .poll(() => cataloguedOpening(page, last), {
      message:
        "an opening is told when the live scene counts, at most every 2 seconds, so it trails what was saved",
      timeout: 15_000,
    })
    .toBe("The far scene speaks.");
  await page.evaluate(() => window.scrollTo(0, 0));
  await scene(page, first).click();
  await page.keyboard.type("A");
  await expect(saveState(page)).toHaveText("All saved");

  await page.routeWebSocket(/\/api\/scenes\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (sceneCarried(frame) !== last) {
        socket.send(frame);
      }
    });
  });
  await page.reload();
  await scene(page, first).click();
  await expect(scene(page, first)).toBeFocused();
  await expect(
    flow(page).locator(`.scene-slot.waiting[data-place-of="${last}"]`),
  ).toHaveCount(1);
  await expect(
    flow(page).locator(`[data-place-of="${last}"] *`),
    "a scene far away draws nothing but its place",
  ).toHaveCount(1);

  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();

  await expect(scene(page, last)).toHaveText("");
  await expect(label(page, last)).toHaveText("The far scene speaks.");
});

test("another author's cursor shows in the scene they are writing in", async ({
  page,
  context,
}) => {
  await aNewProject(page, "TwoAuthors");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  const other = await context.newPage();
  await other.goto(page.url());
  await expect(scene(other, second)).toBeVisible();

  await scene(other, second).click();
  await other.keyboard.type("Written elsewhere.");

  await expect(scene(page, second)).toContainText("Written elsewhere.");
  await expect(
    flow(page).locator(`[data-scene="${second}"] .ProseMirror-yjs-cursor`),
  ).toHaveCount(1);
  await expect(
    flow(page).locator(`[data-scene="${first}"] .ProseMirror-yjs-cursor`),
  ).toHaveCount(0);
  await expect(
    flow(page).locator(
      `[data-scene="${second}"] .ProseMirror-yjs-cursor > div`,
    ),
    "the name above another author's cursor floats over the text rather than pushing it apart",
  ).toHaveCSS("position", "absolute");
  await other.close();
});

test("a dropped connection comes back, and what was written meanwhile is saved", async ({
  page,
}) => {
  let down = false;
  let live: { close: () => Promise<void> } | null = null;
  await page.routeWebSocket(/\/api\/scenes\/sync$/, (socket) => {
    if (down) {
      socket.close();
      return;
    }
    socket.connectToServer();
    live = socket;
  });
  await aNewProject(page, "Dropped");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("Before.");
  await expect(saveState(page)).toHaveText("All saved");

  down = true;
  await live!.close();
  await page.keyboard.type(" During.");
  await expect(saveState(page)).toHaveText("Saving…");
  await page.waitForTimeout(1_000);
  await expect(
    saveState(page),
    "nothing can be stored while the connection is down",
  ).toHaveText("Saving…");

  down = false;
  await expect(saveState(page)).toHaveText("All saved", { timeout: 10_000 });
  expect(
    (await (await page.request.get(`${API}/scenes/${only}`)).json()).text,
  ).toBe("Before. During.");
});

test("a scene scrolled far away is let go, its height kept, and back again as it comes near", async ({
  page,
}) => {
  const left: string[] = [];
  page.on("websocket", (socket) =>
    socket.on("framesent", ({ payload }) => {
      if (Buffer.from(payload)[0] === 1) {
        left.push(sceneCarried(payload));
      }
    }),
  );
  const { first } = await aLongBookWrittenAtItsStart(page, "LetGo");
  await expect(saveState(page)).toHaveText("All saved");
  const height = (await flow(page)
    .locator(`[data-place-of="${first}"]`)
    .boundingBox())!.height;
  await nobodyWriting(page);

  await toTheEnd(page);

  const waiting = flow(page).locator(`.scene.waiting[data-scene="${first}"]`);
  await expect(waiting).toHaveCount(1);
  expect(
    Math.abs((await waiting.boundingBox())!.height - height),
    "the page keeps its length, so nothing below jumps",
  ).toBeLessThan(1);
  expect(left, "a scene let go leaves the connection").toContain(first);

  await page.evaluate(() => window.scrollTo(0, 0));

  await expect(scene(page, first)).toContainText(
    "She had not touched it since spring.",
  );
});

test("the scene the cursor is in stays however far the window scrolls", async ({
  page,
}) => {
  const { first } = await aLongBookWrittenAtItsStart(page, "KeptByTheCursor");

  await toTheEnd(page);
  await page.waitForTimeout(500);

  await expect(scene(page, first)).toBeFocused();
  await page.keyboard.type(" Still here.");
  await expect(scene(page, first)).toContainText("since spring. Still here.");
});

test("a scene still saving stays until it is saved", async ({ page }) => {
  const sync = await holdingWhatIsWritten(page);
  const { first } = await aLongBookWrittenAtItsStart(page, "KeptUntilSaved");
  await expect(saveState(page)).toHaveText("All saved");
  sync.hold();
  await page.keyboard.type(" More.");
  await expect(saveState(page)).toHaveText("Saving…");
  await nobodyWriting(page);

  await toTheEnd(page);
  await page.waitForTimeout(500);

  await expect(
    scene(page, first),
    "letting go of the only copy of an unsaved edit would lose it",
  ).toHaveCount(1);
  sync.release();
  await expect(saveState(page)).toHaveText("All saved");
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${first}"]`),
  ).toHaveCount(1);
});

test("the arrow keys carry the cursor into a scene let go while the window was elsewhere", async ({
  page,
}) => {
  await aNewProject(page, "WalkingIntoTheFar");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first, second, third] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Second.");
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await expect(saveState(page)).toHaveText("All saved");

  await toTheEnd(page);
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${second}"]`),
  ).toHaveCount(1);
  await page.keyboard.press("ArrowDown");

  await expect(scene(page, second)).toBeFocused();
  await page.keyboard.type("X");
  await expect(scene(page, second)).toHaveText("XSecond.");

  await scene(page, third).click();
  await toTheEnd(page);
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${second}"]`),
  ).toHaveCount(1);
  await page.keyboard.press("ArrowUp");

  await expect(scene(page, second)).toBeFocused();
  await page.keyboard.type("Y");
  await expect(
    scene(page, second),
    "coming from below, the cursor lands after the scene's last word",
  ).toHaveText("XSecond.Y");
});

test("two tabs writing in one scene converge", async ({ page, context }) => {
  await aNewProject(page, "Converge");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  const second = await context.newPage();
  await second.goto(page.url());
  await expect(scene(second, only)).toBeVisible();

  await scene(page, only).click();
  await page.keyboard.type("She threaded the shuttle.");
  await expect(scene(second, only)).toContainText("She threaded the shuttle.");
  await scene(second, only).click();
  await second.keyboard.press("Control+End");
  await second.keyboard.type(" The loom answered.");

  const written = (tab: Page) =>
    scene(tab, only).evaluate((editor) => {
      const copy = editor.cloneNode(true) as HTMLElement;
      copy
        .querySelectorAll(".ProseMirror-yjs-cursor")
        .forEach((cursor) => cursor.remove());

      return copy.textContent;
    });
  const woven = "She threaded the shuttle. The loom answered.";
  await expect.poll(() => written(page)).toBe(woven);
  await expect.poll(() => written(second)).toBe(woven);
  await second.close();
});

test("leaving the writing view closes its connection", async ({ page }) => {
  await aNewProject(page, "TearDown");
  const sockets: { isClosed(): boolean }[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().endsWith("/api/scenes/sync")) {
      sockets.push(socket);
    }
  });
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(1);
  expect(sockets).toHaveLength(1);

  await views(page).getByRole("link", { name: "Board", exact: true }).click();

  await expect.poll(() => sockets[0].isClosed()).toBe(true);
  await expect(flow(page)).toHaveCount(0);
});
