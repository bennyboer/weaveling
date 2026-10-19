import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import { aNewProject, API } from "./support/shell";
import {
  flow,
  heading,
  openTheWritingView,
  savedTitles,
} from "./support/writing";

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

const renamedMeanwhile = (page: Page) =>
  page.getByRole("dialog", { name: "Renamed meanwhile" });

async function renamedWhileIAmInIt(page: Page, named: string) {
  const { outline, ids, reloads } = await aBookFollowedLive(page, named);
  await heading(page, "Part One").click();
  const before = reloads();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => reloads()).toBeGreaterThan(before);
}

test("a heading renamed elsewhere while I rename it asks which name stays", async ({
  page,
}) => {
  await renamedWhileIAmInIt(page, "LiveBothRenamed");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");

  await page.keyboard.press("Enter");

  await expect(renamedMeanwhile(page)).toContainText(
    "someone else renamed it to “The Beginning”. Yours is “Part One, drafted”.",
  );
});

test("keeping my name over one given meanwhile keeps mine everywhere", async ({
  page,
}) => {
  await renamedWhileIAmInIt(page, "LiveKeptMine");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");

  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  await expect(heading(page, "Part One, drafted")).toBeVisible();
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
});

test("taking the name given meanwhile drops mine", async ({ page }) => {
  await renamedWhileIAmInIt(page, "LiveTookTheirs");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");

  await renamedMeanwhile(page)
    .getByRole("button", { name: "Take theirs" })
    .click();

  await expect(heading(page, "The Beginning")).toBeVisible();
  await expect(heading(page, "Part One, drafted")).toHaveCount(0);
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("a heading renamed elsewhere while I only sat in it takes the new name without asking", async ({
  page,
}) => {
  await renamedWhileIAmInIt(page, "LiveSatIn");

  await page.keyboard.press("Enter");

  await expect(heading(page, "The Beginning")).toBeVisible();
  await expect(renamedMeanwhile(page)).toHaveCount(0);
});

test("a heading renamed in two goes, saved in between, never asks", async ({
  page,
}) => {
  await aBookFollowedLive(page, "LiveTwoGoes");
  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
  await page.waitForTimeout(300);

  await page.keyboard.type(" twice");
  await page.keyboard.press("Enter");

  await expect(renamedMeanwhile(page)).toHaveCount(0);
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted twice", "Part Two"]);
});

for (const [answer, settled] of [
  ["Keep mine", "Part One, drafted, again"],
  ["Take theirs", "The Beginning, again"],
] as const) {
  test(`having answered “${answer}”, renaming further asks nothing more`, async ({
    page,
  }) => {
    await renamedWhileIAmInIt(page, `LiveFurther${answer.split(" ")[0]}`);
    await page.keyboard.press("End");
    await page.keyboard.type(", drafted");
    await page.keyboard.press("Enter");
    await renamedMeanwhile(page).getByRole("button", { name: answer }).click();
    await expect(renamedMeanwhile(page)).toHaveCount(0);
    await page.waitForTimeout(300);

    await heading(page, settled.replace(", again", "")).click();
    await page.keyboard.press("End");
    await page.keyboard.type(", again");
    await page.keyboard.press("Enter");

    await expect(renamedMeanwhile(page)).toHaveCount(0);
    await expect.poll(() => savedTitles(page)).toEqual([settled, "Part Two"]);
  });
}

test("having kept mine, the server agreeing while I edit on asks nothing", async ({
  page,
}) => {
  await renamedWhileIAmInIt(page, "LiveKeptThenEdited");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");
  const heldBack = async (route: import("@playwright/test").Route) => {
    await new Promise((later) => setTimeout(later, 800));
    await route.continue();
  };
  await page.route("**/api/outlines/*/sections/*", heldBack);
  await page.route("**/api/outlines", heldBack);
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  await heading(page, "Part One, drafted").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", again");
  await page.waitForTimeout(1500);
  await page.keyboard.press("Enter");

  await expect(renamedMeanwhile(page)).toHaveCount(0);
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted, again", "Part Two"]);
});
