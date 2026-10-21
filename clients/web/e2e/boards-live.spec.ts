import { test, expect, type Page } from "@playwright/test";

import { cardNamed, corkboard, seenAt, waitingIdeas } from "./support/board";
import { aNewProject, API, catalogued } from "./support/shell";
import { projectOf } from "./support/writing";

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

async function theBoard(page: Page) {
  const opened = await anotherAuthor(page, "post", "/boards", {
    project: projectOf(page),
  });

  return (await opened.json()).id as string;
}

const pinning = (idea: string, x: number, y: number) => ({
  idea,
  spot: { x, y },
  size: { width: 168, height: 84 },
});

async function aBoardFollowedLive(page: Page, named: string) {
  await aNewProject(page, named);
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const board = await theBoard(page);
  await page.reload();
  await expect(waitingIdeas(page).getByText("The loom remembers")).toBeVisible();
  const followed = page.waitForEvent("websocket", (socket) =>
    socket.url().includes(`/api/boards/live?board=${board}`),
  );
  await page.reload();
  await followed;

  return { board, loom };
}

test("an idea another author pins appears on the board without a reload", async ({
  page,
}) => {
  const { board, loom } = await aBoardFollowedLive(page, "BoardLivePinned");

  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));

  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  await expect(waitingIdeas(page).getByText("The loom remembers")).toHaveCount(0);
});

test("a card another author moves goes where they put it", async ({ page }) => {
  const { board, loom } = await aBoardFollowedLive(page, "BoardLiveMoved");
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  const before = await seenAt(page, "The loom remembers");

  await anotherAuthor(page, "patch", `/boards/${board}/ideas/${loom}`, {
    spot: { x: 600, y: 300 },
  });

  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    })
    .toEqual({ x: 200, y: 100 });
});

test("a card another author unpins goes back beside the board", async ({
  page,
}) => {
  const { board, loom } = await aBoardFollowedLive(page, "BoardLiveUnpinned");
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();

  await anotherAuthor(page, "delete", `/boards/${board}/ideas/${loom}`);

  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  await expect(waitingIdeas(page).getByText("The loom remembers")).toBeVisible();
});

test("an idea captured and pinned elsewhere shows on the board by its name", async ({
  page,
}) => {
  await page.routeWebSocket(/\/api\/ideas\/live\?project=/, () => {});
  const { board } = await aBoardFollowedLive(page, "BoardLiveUnknown");
  const winter = await anIdea(page, "Winter comes early");
  await catalogued(page, ["The loom remembers", "Winter comes early"]);

  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(winter, 400, 200));

  await expect(
    cardNamed(page, "Winter comes early"),
    "the board hears of the idea only through the pin, so it asks for the ideas again",
  ).toBeVisible();
});

test("an idea the list of ideas does not hold yet is asked for once per change, not in a loop", async ({
  page,
}) => {
  await page.routeWebSocket(/\/api\/ideas\/live\?project=/, () => {});
  const { board } = await aBoardFollowedLive(page, "BoardLiveLagging");
  const winter = await anIdea(page, "Winter comes early");
  let asked = 0;
  await page.route(/\/api\/ideas\?project=/, async (route) => {
    asked += 1;
    const answer = await route.fetch();
    const listed = (await answer.json()).filter(
      (idea: { id: string }) => idea.id !== winter,
    );
    await route.fulfill({ response: answer, json: listed });
  });

  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(winter, 400, 200));
  await expect.poll(() => asked).toBeGreaterThan(0);
  await page.waitForTimeout(1500);

  expect(
    asked,
    "asked again once for the change that brought it, not over and over while the list lags",
  ).toBeLessThanOrEqual(2);
});

const openings = (page: Page) => {
  let opened = 0;
  page.on("request", (request) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/boards") {
      opened += 1;
    }
  });

  return () => opened;
};

test("a change that follows on the board as held is taken in, without opening the board again", async ({
  page,
}) => {
  const { board, loom } = await aBoardFollowedLive(page, "BoardLiveTakenIn");
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  const opened = openings(page);
  const before = await seenAt(page, "The loom remembers");

  await anotherAuthor(page, "patch", `/boards/${board}/ideas/${loom}`, {
    spot: { x: 600, y: 300 },
  });

  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).x - before.x)
    .toBe(200);
  await page.waitForTimeout(500);
  expect(opened(), "the move was taken in as it came").toBe(0);
});

test("a change missed on the way opens the board again, and the board comes out right", async ({
  page,
}) => {
  await aNewProject(page, "BoardLiveMissed");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const board = await theBoard(page);
  let passed = 0;
  await page.routeWebSocket(/\/api\/boards\/live\?board=/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      passed += 1;
      if (passed !== 3) {
        socket.send(frame);
      }
    });
  });
  await page.reload();
  await expect(waitingIdeas(page).getByText("The loom remembers")).toBeVisible();
  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  const before = await seenAt(page, "The loom remembers");
  const opened = openings(page);

  await anotherAuthor(page, "patch", `/boards/${board}/ideas/${loom}`, {
    spot: { x: 600, y: 300 },
  });
  await anotherAuthor(page, "patch", `/boards/${board}/ideas/${loom}`, {
    size: { width: 240, height: 120 },
  });

  await expect.poll(() => passed).toBeGreaterThanOrEqual(4);
  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, width: now.width };
    }, { message: "the move that was missed is there too, so the board was opened again" })
    .toEqual({ x: 200, width: Math.round(before.width * 240 / 168) });
  expect(opened()).toBeGreaterThan(0);
});

test("a board is opened once, and a change made before it is followed is caught up as it connects", async ({
  page,
}) => {
  await aNewProject(page, "BoardLiveHello");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const board = await theBoard(page);
  let connect = () => {};
  await page.routeWebSocket(/\/api\/boards\/live\?board=/, (socket) => {
    connect = () => void socket.connectToServer();
  });
  const opened = openings(page);
  await page.reload();
  await expect(waitingIdeas(page).getByText("The loom remembers")).toBeVisible();
  await expect.poll(opened, { message: "opened once, as the board is shown" }).toBe(1);

  await anotherAuthor(page, "post", `/boards/${board}/ideas`, pinning(loom, 400, 200));
  connect();

  await expect(
    cardNamed(page, "The loom remembers"),
    "told where the board stands, a newer version than held, the board is opened again",
  ).toBeVisible();
  expect(opened()).toBe(2);
});
