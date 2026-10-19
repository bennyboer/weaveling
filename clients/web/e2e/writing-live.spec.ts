import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import { aNewProject, API } from "./support/shell";
import { flow, heading, openTheWritingView } from "./support/writing";

async function aBookFollowedLive(page: Page, named: string) {
  await aNewProject(page, named);
  const book = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  let opened = 0;
  page.on("response", (response) => {
    if (
      response.request().method() === "POST" &&
      response.url().endsWith("/api/outlines")
    ) {
      opened += 1;
    }
  });
  await openTheWritingView(page);
  await expect
    .poll(() => opened, {
      message: "the book is loaded, and loaded again once it is followed live",
    })
    .toBeGreaterThanOrEqual(2);

  return { ...book, reloads: () => opened };
}

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "delete",
  path: string,
  data?: object,
) {
  const done = await page.request[method](`${API}${path}`, { data });
  expect(done.ok(), await done.text()).toBe(true);
}

test("a book followed live is not loaded again while nothing changes", async ({
  page,
}) => {
  const { reloads } = await aBookFollowedLive(page, "LiveQuiet");
  const settled = reloads();

  await page.waitForTimeout(1500);

  expect(
    reloads(),
    "a live channel that keeps dropping must not reload the book over and over",
  ).toBe(settled);
});

test("a section another author adds appears without a reload", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveAdded");

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "Part Two",
    "Part Three",
  ]);
});

test("a change already held is not loaded again", async ({ page }) => {
  let toPage: ((frame: string | Buffer) => void) | null = null;
  let last: string | Buffer | null = null;
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      last = frame;
      socket.send(frame);
    });
    toPage = (frame) => socket.send(frame);
  });
  const { outline, ids, reloads } = await aBookFollowedLive(page, "LiveHeld");
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });
  await expect(heading(page, "Part Three")).toBeVisible();
  await page.waitForTimeout(300);
  const settled = reloads();

  toPage!(last!);
  await page.waitForTimeout(500);

  expect(
    reloads(),
    "the same change told twice is already held the second time",
  ).toBe(settled);
});

test("a heading another author renames takes the new name", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveRenamed");

  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );

  await expect(heading(page, "The Beginning")).toBeVisible();
  await expect(heading(page, "Part One")).toHaveCount(0);
});

test("a section another author removes goes", async ({ page }) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveRemoved");

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("what changed while the connection was down shows once it is back", async ({
  page,
}) => {
  let connections = 0;
  let first: { close: () => Promise<void> } | null = null;
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, (socket) => {
    connections += 1;
    if (connections === 1) {
      first = socket;
      return;
    }
    socket.connectToServer();
  });
  const { outline, ids } = await aBookFollowedLive(page, "LiveCaughtUp");

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Written meanwhile",
    under: null,
    after: ids["Part Two"],
  });
  await page.waitForTimeout(500);
  await expect(
    heading(page, "Written meanwhile"),
    "nothing comes through a connection that hears nothing",
  ).toHaveCount(0);
  await first!.close();

  await expect(heading(page, "Written meanwhile")).toBeVisible();
});

test("a channel that keeps dropping just after opening is tried again less and less often", async ({
  page,
}) => {
  let connections = 0;
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, (socket) => {
    connections += 1;
    setTimeout(() => void socket.close(), 50);
  });
  await aNewProject(page, "LiveDropping");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);
  await expect.poll(() => connections).toBeGreaterThan(0);
  const first = connections;

  await page.waitForTimeout(2000);

  expect(
    connections - first,
    "waiting twice as long each time, it reconnects a handful of times in two seconds, not twenty",
  ).toBeLessThanOrEqual(6);
});
