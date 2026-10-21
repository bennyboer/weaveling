import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aNewProject, aSceneMade, API, sceneCatalogued } from "./support/shell";
import {
  flow,
  heading,
  menuItem,
  newScene,
  openTheWritingView,
  projectOf,
  projectsOutline,
  savedTitles,
  scene,
  sectionActions,
  shown,
} from "./support/writing";

async function followingLive(page: Page, { routed = false } = {}) {
  let opened = 0;
  let told = 0;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/outlines/live")) {
      socket.on("framereceived", () => {
        told += 1;
      });
    }
  });
  page.on("response", (response) => {
    if (
      response.request().method() === "POST" &&
      response.url().endsWith("/api/outlines")
    ) {
      opened += 1;
    }
  });
  await openTheWritingView(page);
  if (routed) {
    await expect.poll(() => opened).toBe(1);
  } else {
    await expect
      .poll(() => told, { message: "followed live, told first where it stands" })
      .toBe(1);
  }
  expect(opened, "loaded once, and not again as it is followed").toBe(1);

  return { reloads: () => opened, heard: () => told };
}

async function aBookFollowedLive(
  page: Page,
  named: string,
  { routed = false } = {},
) {
  await aNewProject(page, named);
  const book = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  const { reloads, heard } = await followingLive(page, { routed });

  return { ...book, reloads, heard };
}

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "put" | "delete",
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
  const { outline, ids } = await aBookFollowedLive(page, "LiveCaughtUp", {
    routed: true,
  });

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
  const { outline, ids, heard } = await aBookFollowedLive(page, named);
  await heading(page, "Part One").click();
  const before = heard();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);
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

async function placedOnServer(page: Page) {
  const outline = await projectsOutline(page);
  const ids: Record<string, string> = Object.fromEntries(
    outline.sections.map((held: { title: string; section: string }) => [
      held.title,
      held.section,
    ]),
  );

  return { outline: outline.id as string, ids };
}

test("a heading I am typing in keeps my words and my cursor when another author nests it", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveNested");
  await heading(page, "Part Two").click();
  await page.keyboard.press("End");

  await page.keyboard.type(", rewritten");
  await anotherAuthor(
    page,
    "post",
    `/outlines/${outline}/sections/${ids["Part Two"]}/demote`,
  );
  await expect(heading(page, "Part Two, rewritten")).toHaveAttribute(
    "aria-level",
    "3",
  );
  await page.keyboard.type(" again");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, rewritten again"]);
});

test("a scene I am writing in keeps my cursor when another author moves its section", async ({
  page,
}) => {
  await aNewProject(page, "LiveMovedUnder");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [mine] = scenes["Part Two"];
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, mine).click();
  await page.keyboard.type("Hello");

  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part Two"]}/place`,
    { under: null, after: null },
  );
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part Two",
    "Part One",
  ]);
  await page.keyboard.type(" world");

  await expect.poll(() => shown(page, mine)).toBe("Hello world");
});

const quietNote = (page: Page) =>
  page.getByRole("status").filter({ hasText: "rearranged the book" });

test("a quiet note says when someone else rearranges the book around where I write", async ({
  page,
}) => {
  await aNewProject(page, "LiveNoted");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, scenes["Part Two"][0]).click();

  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part Two"]}/place`,
    { under: null, after: null },
  );

  await expect(quietNote(page)).toHaveText(
    "Someone else rearranged the book around “Part Two”.",
  );
  await quietNote(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(quietNote(page)).toHaveCount(0);
});

test("writing in a chapter, the note names the part that was moved", async ({
  page,
}) => {
  await aNewProject(page, "LiveNotedPart");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await followingLive(page);
  await heading(page, "Chapter 1").click();

  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part One"]}/place`,
    { under: null, after: ids["Part Two"] },
  );

  await expect(quietNote(page)).toHaveText(
    "Someone else rearranged the book around “Part One”.",
  );
});

test("nesting a section myself tells me nothing", async ({ page }) => {
  await aBookFollowedLive(page, "LiveNestedMyself");
  await heading(page, "Part Two").click();

  await page.keyboard.press("Tab");

  await expect(heading(page, "Part Two")).toHaveAttribute("aria-level", "3");
  await page.waitForTimeout(1000);
  await expect(quietNote(page)).toHaveCount(0);
});

test("a section added beside mine tells me nothing", async ({ page }) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveAddedBeside");
  await heading(page, "Part Two").click();

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part One and a half",
    under: null,
    after: ids["Part One"],
  });

  await expect(heading(page, "Part One and a half")).toBeVisible();
  await page.waitForTimeout(500);
  await expect(quietNote(page)).toHaveCount(0);
});

const removedMeanwhile = (page: Page) =>
  page.getByRole("dialog", { name: "Removed meanwhile" });

async function writingInASceneOfPartTwo(page: Page, named: string) {
  await aNewProject(page, named);
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [mine] = scenes["Part Two"];
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, mine).click();
  await page.keyboard.type("Hello");
  await expect.poll(() => shown(page, mine)).toBe("Hello");

  return { mine, outline, ids };
}

test("a section removed while I write in its scene asks, and Show it goes to my scene", async ({
  page,
}) => {
  const { mine, outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedUnder",
  );

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(removedMeanwhile(page)).toContainText(
    "Someone else removed “Part Two”.",
  );
  await removedMeanwhile(page).getByRole("button", { name: "Show it" }).click();
  await expect(page).toHaveURL(/\/write\/loose/);
  await expect.poll(() => shown(page, mine)).toBe("Hello");
});

test("keeping on writing after my section was removed stays in the book", async ({
  page,
}) => {
  const { outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedStay",
  );
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await removedMeanwhile(page)
    .getByRole("button", { name: "Keep writing" })
    .click();

  await expect(removedMeanwhile(page)).toHaveCount(0);
  await expect(page).toHaveURL(/\/write$/);
});

test("a section removed while I am in its heading is told quietly", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveRemovedHeading");
  await heading(page, "Part Two").click();

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveText("Someone else removed “Part Two”.");
  await expect(removedMeanwhile(page)).toHaveCount(0);
});

test("having left my scene for its section's heading, the removal is told quietly", async ({
  page,
}) => {
  const { outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedAfterLeaving",
  );
  await heading(page, "Part Two").click();

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveText("Someone else removed “Part Two”.");
  await expect(removedMeanwhile(page)).toHaveCount(0);
});

test("removing my own section asks nothing more", async ({ page }) => {
  await aBookFollowedLive(page, "LiveRemovedMyself");
  await heading(page, "Part Two").click();

  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();

  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  await page.waitForTimeout(1000);
  await expect(removedMeanwhile(page)).toHaveCount(0);
  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveCount(0);
});

test("the inspector says when the section it shows was removed", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(
    page,
    "LiveRemovedInspected",
  );
  await heading(page, "Part Two").click();
  await page.getByRole("button", { name: "This section", exact: true }).click();

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(
    page.getByRole("complementary", { name: "This section" }),
  ).toContainText("This section was removed.");
});

const placeGone = (page: Page) =>
  page.getByRole("dialog", { name: "Its place is gone" });

async function holdingBack(page: Page, method: string, url: RegExp) {
  await page.route(url, async (route) => {
    if (route.request().method() !== method) {
      return route.continue();
    }
    await new Promise((later) => setTimeout(later, 800));
    await route.continue();
  });
}

async function aNewSceneWhosePlaceGoes(page: Page, named: string) {
  await aNewProject(page, named);
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, scenes["Part Two"][0]).click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(newScene(page)).toBeFocused();
  await holdingBack(page, "POST", /\/api\/scenes$/);

  await page.keyboard.type("Hello");
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );
}

test("a new scene whose place is removed while it begins asks where its words go", async ({
  page,
}) => {
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedScene");

  await expect(placeGone(page)).toContainText(
    "Someone else removed where your new scene was going. It begins “Hello”.",
  );
  await expect(
    placeGone(page).getByRole("button"),
    "the words have to go somewhere, so there is no way back",
  ).toHaveText(["Discard", "Put it at the end of the book"]);
});

test("a new scene put at the end of the book keeps its words", async ({
  page,
}) => {
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedSceneKept");

  await placeGone(page)
    .getByRole("button", { name: "Put it at the end of the book" })
    .click();

  await expect(flow(page).locator(".ProseMirror").last()).toHaveText("Hello");
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("a new scene discarded leaves nothing behind", async ({ page }) => {
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedSceneGone");

  await placeGone(page).getByRole("button", { name: "Discard" }).click();

  await expect(placeGone(page)).toHaveCount(0);
  await expect
    .poll(
      async () => {
        const listed = await page.request.get(
          `${API}/scenes?project=${projectOf(page)}`,
        );
        return (await listed.json()).scenes.length;
      },
      { message: "the empty scene made for the words is deleted again" },
    )
    .toBe(2);
});

test("a new section whose neighbour is removed while it begins can go to the end of the book", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveUnplacedSection");
  await heading(page, "Part Two").click();
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  await holdingBack(page, "POST", /\/api\/outlines\/[^/]+\/sections$/);

  await page.keyboard.type("Part Three");
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  await expect(placeGone(page)).toContainText(
    "Someone else removed what your new section “Part Three” was going after.",
  );
  await placeGone(page)
    .getByRole("button", { name: "Put it at the end of the book" })
    .click();
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Three"]);
});

test("where I am reading holds while sections come and go above it", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await aNewProject(page, "LiveReading");
  await aBookWithScenes(page, {
    "Part One": 3,
    "Part Two": 3,
    "Part Three": 3,
  });
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  const reading = heading(page, "Part Three");
  await reading.evaluate((shown) => shown.scrollIntoView({ block: "start" }));
  await page.mouse.wheel(0, -60);
  await page.waitForTimeout(300);
  const at = async () => (await reading.boundingBox())!.y;
  const before = await at();
  expect(before, "the book is long enough to scroll").toBeGreaterThan(0);

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "A prologue",
    under: null,
    after: null,
  });
  await expect(heading(page, "A prologue")).toBeAttached();
  await page.waitForTimeout(300);
  expect(Math.abs((await at()) - before)).toBeLessThanOrEqual(2);

  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
  );
  await expect(heading(page, "Part One")).toHaveCount(0);
  await page.waitForTimeout(300);
  expect(Math.abs((await at()) - before)).toBeLessThanOrEqual(2);
});

test("a change made from a view that is behind is refused gently, and the view catches up", async ({
  page,
}) => {
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, () => {});
  const { outline, ids } = await aBookFollowedLive(page, "LiveOvertaken", {
    routed: true,
  });
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );
  await page.waitForTimeout(300);
  await expect(
    heading(page, "Part Two"),
    "this view hears nothing",
  ).toBeVisible();
  await heading(page, "Part Two").click();

  await page.keyboard.press("Tab");

  await expect(
    page.getByRole("status").filter({ hasText: "meanwhile" }),
  ).toHaveText(
    "Someone else changed the book meanwhile, so that could not be done. It shows the book as it is now.",
  );
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a section another author adds is taken into the book, without opening it again", async ({
  page,
}) => {
  const { outline, ids, reloads, heard } = await aBookFollowedLive(page, "LiveTakenIn");
  const before = reloads();
  const told = heard();

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  await expect(heading(page, "Part Three")).toBeVisible();
  await expect.poll(() => heard()).toBeGreaterThan(told);
  await page.waitForTimeout(500);
  expect(reloads(), "the section was taken in as it came").toBe(before);
});

test("a change missed on the way opens the book again, and the book comes out right", async ({
  page,
}) => {
  let passed = 0;
  let connected = false;
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, (socket) => {
    connected = true;
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      passed += 1;
      if (passed !== 2) {
        socket.send(frame);
      }
    });
  });
  await aNewProject(page, "LiveMissed");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);
  await expect.poll(() => connected).toBe(true);
  await page.waitForTimeout(300);

  for (const [section, title] of [
    ["Part One", "The Beginning"],
    ["Part One", "The Very Beginning"],
    ["Part Two", "The Middle"],
  ]) {
    await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids[section]}`, {
      title,
    });
  }

  await expect.poll(() => passed).toBeGreaterThanOrEqual(3);
  await expect(heading(page, "The Middle")).toBeVisible();
  await expect(
    heading(page, "The Very Beginning"),
    "the rename that was missed is there too, so the book was opened again",
  ).toBeVisible();
});


test("a change another author makes while mine is on its way is caught up once mine is through", async ({
  page,
}) => {
  const { outline, ids } = await aBookFollowedLive(page, "LiveWhileMine");
  const theirs = await aSceneMade(page);
  await sceneCatalogued(page, theirs);
  let released: () => void = () => {};
  const mineThrough = new Promise<void>((through) => {
    released = through;
  });
  await page.route("**/api/outlines/*/sections/*", async (route) => {
    if (route.request().method() === "PATCH") {
      await mineThrough;
    }
    await route.continue();
  });
  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");
  await page.waitForTimeout(300);

  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: theirs },
    section: ids["Part Two"],
    after: null,
  });
  await page.waitForTimeout(500);
  released();

  await expect(
    flow(page).locator(`[data-scene="${theirs}"]`),
    "the scene they placed while my rename was on its way",
  ).toHaveCount(1);
});
