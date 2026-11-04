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
import { heardOn, routingLive } from "./support/live";

async function followingLive(page: Page, { routed = false } = {}) {
  let opened = 0;
  let told = 0;
  heardOn(page, "outlines", () => {
    told += 1;
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
  // given: a book followed live in the writing view
  const { reloads } = await aBookFollowedLive(page, "LiveQuiet");
  const settled = reloads();

  // when: a while passes without changes
  await page.waitForTimeout(1500);

  // then: the book is not loaded again
  expect(
    reloads(),
    "a live channel that keeps dropping must not reload the book over and over",
  ).toBe(settled);
});

test("a section another author adds appears without a reload", async ({
  page,
}) => {
  // given: a book followed live in the writing view
  const { outline, ids } = await aBookFollowedLive(page, "LiveAdded");

  // when: another author adds a section
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  // then: it appears
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "Part Two",
    "Part Three",
  ]);
});

test("a change already held is not loaded again", async ({ page }) => {
  // given: the last change told to the page kept
  let last: string | null = null;
  const live = await routingLive(page, {
    heard: {
      outlines: (told, pass) => {
        last = told;
        pass();
      },
    },
  });
  const toPage = (told: string) => live.again(told);
  // and: a book followed live in the writing view
  const { outline, ids, reloads } = await aBookFollowedLive(page, "LiveHeld");
  // and: a section another author added, already shown
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });
  await expect(heading(page, "Part Three")).toBeVisible();
  await page.waitForTimeout(300);
  const settled = reloads();

  // when: the same change is told again
  toPage(last!);
  await page.waitForTimeout(500);

  // then: the book is not loaded again
  expect(
    reloads(),
    "the same change told twice is already held the second time",
  ).toBe(settled);
});

test("a heading another author renames takes the new name", async ({
  page,
}) => {
  // given: a book followed live in the writing view
  const { outline, ids } = await aBookFollowedLive(page, "LiveRenamed");

  // when: another author renames a section
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );

  // then: its heading takes the new name
  await expect(heading(page, "The Beginning")).toBeVisible();
  await expect(heading(page, "Part One")).toHaveCount(0);
});

test("a section another author removes goes", async ({ page }) => {
  // given: a book followed live in the writing view
  const { outline, ids } = await aBookFollowedLive(page, "LiveRemoved");

  // when: another author removes a section
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: it goes
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("what changed while the connection was down shows once it is back", async ({
  page,
}) => {
  // given: a live connection that hears nothing until it is opened again
  let first: { close: () => Promise<void> } | null = null;
  await routingLive(page, {
    connecting: (socket, nth) => {
      if (nth === 1) {
        first = socket;
        return false;
      }
      return true;
    },
  });
  // and: a book followed through it
  const { outline, ids } = await aBookFollowedLive(page, "LiveCaughtUp", {
    routed: true,
  });

  // when: another author adds a section meanwhile
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Written meanwhile",
    under: null,
    after: ids["Part Two"],
  });
  await page.waitForTimeout(500);
  // then: it does not show
  await expect(
    heading(page, "Written meanwhile"),
    "nothing comes through a connection that hears nothing",
  ).toHaveCount(0);
  // when: the connection is opened again
  await first!.close();

  // then: it shows
  await expect(heading(page, "Written meanwhile")).toBeVisible();
});

test("a channel that keeps dropping just after opening is tried again less and less often", async ({
  page,
}) => {
  // given: a live channel that drops just after each opening
  let connections = 0;
  await routingLive(page, {
    connecting: (socket) => {
      connections += 1;
      setTimeout(() => void socket.close(), 50);
      return false;
    },
  });
  // and: a book open in the writing view
  await aNewProject(page, "LiveDropping");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);
  await expect.poll(() => connections).toBeGreaterThan(0);
  const first = connections;

  // when: two seconds pass
  await page.waitForTimeout(2000);

  // then: it is opened again only a handful of times
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
  // given: a heading i am in, renamed meanwhile by another author
  await renamedWhileIAmInIt(page, "LiveBothRenamed");
  // and: my own renaming of it
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");

  // when: i finish it
  await page.keyboard.press("Enter");

  // then: i am asked which name stays
  await expect(renamedMeanwhile(page)).toContainText(
    "someone else renamed it to “The Beginning”. Yours is “Part One, drafted”.",
  );
});

test("keeping my name over one given meanwhile keeps mine everywhere", async ({
  page,
}) => {
  // given: my renaming of a heading renamed meanwhile by another author
  await renamedWhileIAmInIt(page, "LiveKeptMine");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");

  // when: i keep mine
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  // then: the heading shows mine
  await expect(heading(page, "Part One, drafted")).toBeVisible();
  // and: mine is saved
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
});

test("taking the name given meanwhile drops mine", async ({ page }) => {
  // given: my renaming of a heading renamed meanwhile by another author
  await renamedWhileIAmInIt(page, "LiveTookTheirs");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");

  // when: i take theirs
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Take theirs" })
    .click();

  // then: the heading shows theirs
  await expect(heading(page, "The Beginning")).toBeVisible();
  await expect(heading(page, "Part One, drafted")).toHaveCount(0);
  // and: theirs stays saved
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("a heading renamed elsewhere while I only sat in it takes the new name without asking", async ({
  page,
}) => {
  // given: a heading i only sit in, renamed meanwhile by another author
  await renamedWhileIAmInIt(page, "LiveSatIn");

  // when: i leave it
  await page.keyboard.press("Enter");

  // then: it takes the new name
  await expect(heading(page, "The Beginning")).toBeVisible();
  // and: i am asked nothing
  await expect(renamedMeanwhile(page)).toHaveCount(0);
});

test("a heading renamed in two goes, saved in between, never asks", async ({
  page,
}) => {
  // given: a heading i renamed, saved
  await aBookFollowedLive(page, "LiveTwoGoes");
  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
  await page.waitForTimeout(300);

  // when: i rename it further
  await page.keyboard.type(" twice");
  await page.keyboard.press("Enter");

  // then: i am asked nothing
  await expect(renamedMeanwhile(page)).toHaveCount(0);
  // and: the new name is saved
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
    // given: a heading renamed by me and by another author, the question answered
    await renamedWhileIAmInIt(page, `LiveFurther${answer.split(" ")[0]}`);
    await page.keyboard.press("End");
    await page.keyboard.type(", drafted");
    await page.keyboard.press("Enter");
    await renamedMeanwhile(page).getByRole("button", { name: answer }).click();
    await expect(renamedMeanwhile(page)).toHaveCount(0);
    await page.waitForTimeout(300);

    // when: i rename it further
    await heading(page, settled.replace(", again", "")).click();
    await page.keyboard.press("End");
    await page.keyboard.type(", again");
    await page.keyboard.press("Enter");

    // then: i am asked nothing
    await expect(renamedMeanwhile(page)).toHaveCount(0);
    // and: the new name is saved
    await expect.poll(() => savedTitles(page)).toEqual([settled, "Part Two"]);
  });
}

test("having kept mine, the server agreeing while I edit on asks nothing", async ({
  page,
}) => {
  // given: my renaming of a heading renamed meanwhile by another author
  await renamedWhileIAmInIt(page, "LiveKeptThenEdited");
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  await page.keyboard.press("Enter");
  // and: the server slow to answer
  const heldBack = async (route: import("@playwright/test").Route) => {
    await new Promise((later) => setTimeout(later, 800));
    await route.continue();
  };
  await page.route("**/api/outlines/*/sections/*", heldBack);
  await page.route("**/api/outlines", heldBack);
  // and: mine kept
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  // when: i rename it further while the server agrees with mine
  await heading(page, "Part One, drafted").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", again");
  await page.waitForTimeout(1500);
  await page.keyboard.press("Enter");

  // then: i am asked nothing
  await expect(renamedMeanwhile(page)).toHaveCount(0);
  // and: the new name is saved
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
  // given: a heading i am typing in, followed live
  const { outline, ids } = await aBookFollowedLive(page, "LiveNested");
  await heading(page, "Part Two").click();
  await page.keyboard.press("End");

  // when: i type on while another author nests it
  await page.keyboard.type(", rewritten");
  await anotherAuthor(
    page,
    "post",
    `/outlines/${outline}/sections/${ids["Part Two"]}/demote`,
  );
  // then: it is nested
  await expect(heading(page, "Part Two, rewritten")).toHaveAttribute(
    "aria-level",
    "3",
  );
  // when: i type on and finish
  await page.keyboard.type(" again");
  await page.keyboard.press("Enter");

  // then: every word is saved
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, rewritten again"]);
});

test("a scene I am writing in keeps my cursor when another author moves its section", async ({
  page,
}) => {
  // given: a scene i am writing in, followed live
  await aNewProject(page, "LiveMovedUnder");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [mine] = scenes["Part Two"];
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, mine).click();
  await page.keyboard.type("Hello");

  // when: another author moves its section
  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part Two"]}/place`,
    { under: null, after: null },
  );
  // then: the section is moved
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part Two",
    "Part One",
  ]);
  // and: my typing goes on in the scene
  await page.keyboard.type(" world");

  await expect.poll(() => shown(page, mine)).toBe("Hello world");
});

const quietNote = (page: Page) =>
  page.getByRole("status").filter({ hasText: "rearranged the book" });

test("a quiet note says when someone else rearranges the book around where I write", async ({
  page,
}) => {
  // given: the cursor in a scene of the second section, followed live
  await aNewProject(page, "LiveNoted");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  await scene(page, scenes["Part Two"][0]).click();

  // when: another author moves the section to the start
  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part Two"]}/place`,
    { under: null, after: null },
  );

  // then: a quiet note says the book was rearranged around it
  await expect(quietNote(page)).toHaveText(
    "Someone else rearranged the book around “Part Two”.",
  );
  // when: the note is dismissed
  await quietNote(page).getByRole("button", { name: "Dismiss" }).click();
  // then: it goes
  await expect(quietNote(page)).toHaveCount(0);
});

test("writing in a chapter, the note names the part that was moved", async ({
  page,
}) => {
  // given: the cursor in a chapter of the first part, followed live
  await aNewProject(page, "LiveNotedPart");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await followingLive(page);
  await heading(page, "Chapter 1").click();

  // when: another author moves the part
  await anotherAuthor(
    page,
    "put",
    `/outlines/${outline}/sections/${ids["Part One"]}/place`,
    { under: null, after: ids["Part Two"] },
  );

  // then: the note names the part
  await expect(quietNote(page)).toHaveText(
    "Someone else rearranged the book around “Part One”.",
  );
});

test("nesting a section myself tells me nothing", async ({ page }) => {
  // given: the cursor in a heading, followed live
  await aBookFollowedLive(page, "LiveNestedMyself");
  await heading(page, "Part Two").click();

  // when: i nest the section myself
  await page.keyboard.press("Tab");

  // then: it is nested
  await expect(heading(page, "Part Two")).toHaveAttribute("aria-level", "3");
  // and: no note comes
  await page.waitForTimeout(1000);
  await expect(quietNote(page)).toHaveCount(0);
});

test("a section added beside mine tells me nothing", async ({ page }) => {
  // given: the cursor in a heading, followed live
  const { outline, ids } = await aBookFollowedLive(page, "LiveAddedBeside");
  await heading(page, "Part Two").click();

  // when: another author adds a section beside it
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part One and a half",
    under: null,
    after: ids["Part One"],
  });

  // then: it appears
  await expect(heading(page, "Part One and a half")).toBeVisible();
  // and: no note comes
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
  // given: a scene of the second section being written in, followed live
  const { mine, outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedUnder",
  );

  // when: another author removes the section
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: i am asked about it
  await expect(removedMeanwhile(page)).toContainText(
    "Someone else removed “Part Two”.",
  );
  // when: i choose to see the scene
  await removedMeanwhile(page).getByRole("button", { name: "Show it" }).click();
  // then: it is shown outside the book with my words
  await expect(page).toHaveURL(/\/write\/loose/);
  await expect.poll(() => shown(page, mine)).toBe("Hello");
});

test("keeping on writing after my section was removed stays in the book", async ({
  page,
}) => {
  // given: a scene of the second section being written in, followed live
  const { outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedStay",
  );
  // and: its section removed by another author
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // when: i choose to keep writing
  await removedMeanwhile(page)
    .getByRole("button", { name: "Keep writing" })
    .click();

  // then: the question goes
  await expect(removedMeanwhile(page)).toHaveCount(0);
  // and: i stay in the book
  await expect(page).toHaveURL(/\/write$/);
});

test("a section removed while I am in its heading is told quietly", async ({
  page,
}) => {
  // given: the cursor in a heading, followed live
  const { outline, ids } = await aBookFollowedLive(page, "LiveRemovedHeading");
  await heading(page, "Part Two").click();

  // when: another author removes the section
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: it is told quietly
  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveText("Someone else removed “Part Two”.");
  // and: i am asked nothing
  await expect(removedMeanwhile(page)).toHaveCount(0);
});

test("having left my scene for its section's heading, the removal is told quietly", async ({
  page,
}) => {
  // given: a scene of the second section written in, the cursor then in its heading
  const { outline, ids } = await writingInASceneOfPartTwo(
    page,
    "LiveRemovedAfterLeaving",
  );
  await heading(page, "Part Two").click();

  // when: another author removes the section
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: it is told quietly
  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveText("Someone else removed “Part Two”.");
  // and: i am asked nothing
  await expect(removedMeanwhile(page)).toHaveCount(0);
});

test("removing my own section asks nothing more", async ({ page }) => {
  // given: the cursor in a heading, followed live
  await aBookFollowedLive(page, "LiveRemovedMyself");
  await heading(page, "Part Two").click();

  // when: i remove the section myself
  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();

  // then: it goes
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  // and: i am neither asked nor told
  await page.waitForTimeout(1000);
  await expect(removedMeanwhile(page)).toHaveCount(0);
  await expect(
    page.getByRole("status").filter({ hasText: "removed" }),
  ).toHaveCount(0);
});

test("the inspector says when the section it shows was removed", async ({
  page,
}) => {
  // given: the tray open on a section, followed live
  const { outline, ids } = await aBookFollowedLive(
    page,
    "LiveRemovedInspected",
  );
  await heading(page, "Part Two").click();
  await page.getByRole("button", { name: "This section", exact: true }).click();

  // when: another author removes the section
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: the tray says it was removed
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
  // given: a new scene begun, whose section another author removes while it is being made
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedScene");

  // then: i am asked where its words go
  await expect(placeGone(page)).toContainText(
    "Someone else removed where your new scene was going. It begins “Hello”.",
  );
  // and: only discarding or putting it at the end is offered
  await expect(
    placeGone(page).getByRole("button"),
    "the words have to go somewhere, so there is no way back",
  ).toHaveText(["Discard", "Put it at the end of the book"]);
});

test("a new scene put at the end of the book keeps its words", async ({
  page,
}) => {
  // given: a new scene begun, whose section another author removes while it is being made
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedSceneKept");

  // when: i put it at the end of the book
  await placeGone(page)
    .getByRole("button", { name: "Put it at the end of the book" })
    .click();

  // then: it is there with its words
  await expect(flow(page).locator(".ProseMirror").last()).toHaveText("Hello");
  // and: the section is gone
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("a new scene discarded leaves nothing behind", async ({ page }) => {
  // given: a new scene begun, whose section another author removes while it is being made
  await aNewSceneWhosePlaceGoes(page, "LiveUnplacedSceneGone");

  // when: i discard it
  await placeGone(page).getByRole("button", { name: "Discard" }).click();

  // then: the question goes
  await expect(placeGone(page)).toHaveCount(0);
  // and: no scene is left of it
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
  // given: a new section begun after another, followed live
  const { outline, ids } = await aBookFollowedLive(page, "LiveUnplacedSection");
  await heading(page, "Part Two").click();
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  // and: the new section slow to be made
  await holdingBack(page, "POST", /\/api\/outlines\/[^/]+\/sections$/);

  // when: it is named while another author removes the one before it
  await page.keyboard.type("Part Three");
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );

  // then: i am asked where it goes
  await expect(placeGone(page)).toContainText(
    "Someone else removed what your new section “Part Three” was going after.",
  );
  // when: i put it at the end of the book
  await placeGone(page)
    .getByRole("button", { name: "Put it at the end of the book" })
    .click();
  // then: it is saved there
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Three"]);
});

test("where I am reading holds while sections come and go above it", async ({
  page,
}) => {
  // given: a long book followed live on a short window
  await page.setViewportSize({ width: 1280, height: 420 });
  await aNewProject(page, "LiveReading");
  await aBookWithScenes(page, {
    "Part One": 3,
    "Part Two": 3,
    "Part Three": 3,
  });
  await followingLive(page);
  const { outline, ids } = await placedOnServer(page);
  // and: a heading near the top of the window
  const reading = heading(page, "Part Three");
  await reading.evaluate((shown) => shown.scrollIntoView({ block: "start" }));
  await page.mouse.wheel(0, -60);
  await page.waitForTimeout(300);
  const at = async () => (await reading.boundingBox())!.y;
  const before = await at();
  expect(before, "the book is long enough to scroll").toBeGreaterThan(0);

  // when: another author adds a section above it
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "A prologue",
    under: null,
    after: null,
  });
  // then: the heading stays where it was
  await expect(heading(page, "A prologue")).toBeAttached();
  await page.waitForTimeout(300);
  expect(Math.abs((await at()) - before)).toBeLessThanOrEqual(2);

  // when: another author removes a section above it
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
  );
  // then: the heading stays where it was
  await expect(heading(page, "Part One")).toHaveCount(0);
  await page.waitForTimeout(300);
  expect(Math.abs((await at()) - before)).toBeLessThanOrEqual(2);
});

test("a change made from a view that is behind is refused gently, and the view catches up", async ({
  page,
}) => {
  // given: a book open in the writing view that hears no news
  await routingLive(page, { withheld: ["outlines"] });
  const { outline, ids } = await aBookFollowedLive(page, "LiveOvertaken", {
    routed: true,
  });
  // and: a section another author removed meanwhile
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
  // when: i nest it
  await heading(page, "Part Two").click();

  await page.keyboard.press("Tab");

  // then: i am told gently the book had changed
  await expect(
    page.getByRole("status").filter({ hasText: "meanwhile" }),
  ).toHaveText(
    "Someone else changed the book meanwhile, so that could not be done. It shows the book as it is now.",
  );
  // and: the book shows how it is now
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  // and: no alarm is raised
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a section another author adds is taken into the book, without opening it again", async ({
  page,
}) => {
  // given: a book followed live in the writing view
  const { outline, ids, reloads, heard } = await aBookFollowedLive(page, "LiveTakenIn");
  const before = reloads();
  const told = heard();

  // when: another author adds a section
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  // then: it appears
  await expect(heading(page, "Part Three")).toBeVisible();
  // and: it was told
  await expect.poll(() => heard()).toBeGreaterThan(told);
  // and: the book was not opened again
  await page.waitForTimeout(500);
  expect(reloads(), "the section was taken in as it came").toBe(before);
});

test("a change missed on the way opens the book again, and the book comes out right", async ({
  page,
}) => {
  // given: a live channel that misses the second change told
  let passed = 0;
  let connected = false;
  await routingLive(page, {
    connecting: () => {
      connected = true;
      return true;
    },
    heard: {
      outlines: (_told, pass) => {
        passed += 1;
        if (passed !== 2) {
          pass();
        }
      },
    },
  });
  // and: a book open in the writing view
  await aNewProject(page, "LiveMissed");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);
  await expect.poll(() => connected).toBe(true);
  await page.waitForTimeout(300);

  // when: another author makes three changes
  for (const [section, title] of [
    ["Part One", "The Beginning"],
    ["Part One", "The Very Beginning"],
    ["Part Two", "The Middle"],
  ]) {
    await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids[section]}`, {
      title,
    });
  }

  // then: the last shows
  await expect.poll(() => passed).toBeGreaterThanOrEqual(3);
  await expect(heading(page, "The Middle")).toBeVisible();
  // and: so does the missed one
  await expect(
    heading(page, "The Very Beginning"),
    "the rename that was missed is there too, so the book was opened again",
  ).toBeVisible();
});


test("a change another author makes while mine is on its way is caught up once mine is through", async ({
  page,
}) => {
  // given: a book followed live in the writing view
  const { outline, ids } = await aBookFollowedLive(page, "LiveWhileMine");
  // and: a scene outside it
  const theirs = await aSceneMade(page);
  await sceneCatalogued(page, theirs);
  // and: my renaming of a heading held on its way
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

  // when: another author places the scene meanwhile
  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: theirs },
    section: ids["Part Two"],
    after: null,
  });
  await page.waitForTimeout(500);
  // and: my renaming goes through
  released();

  // then: their scene is in the book
  await expect(
    flow(page).locator(`[data-scene="${theirs}"]`),
    "the scene they placed while my rename was on its way",
  ).toHaveCount(1);
});
