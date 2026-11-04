import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject, API } from "./support/shell";
import { aBookInTheWritingView, addHere, bringingBack, flow, gapBelow, heading, menuItem, newScene, offered, openTheWritingView, projectsOutline, savedTitles, scene, sceneActions, sectionActions, theBook, titled } from "./support/writing";

const gapAtStartOf = (page: Page, named: string) =>
  flow(page)
    .locator(".in-flow")
    .filter({ has: page.getByRole("heading", { name: named }) })
    .locator(".insertion:not(.below-a-scene)");

async function aBookWithLooseScenes(page: Page, titles: string[]) {
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const loose: Record<string, string> = {};

  for (const title of titles) {
    loose[title] = await aLooseScene(page);
    await titled(page, loose[title], title);
  }

  return { placed: scenes["Part One"][0], loose };
}

const bookStart = (page: Page) => flow(page).locator(".insertion.book-start");

const bookEnd = (page: Page) => flow(page).locator(".insertion.book-end");

async function namingTheNewSection(page: Page, title: string) {
  await expect(page.locator(":focus")).toHaveAttribute("role", "heading");
  await page.keyboard.type(title);
  await expect.poll(() => savedTitles(page)).toContain(title);
}

const headingAt = (page: Page, named: string, level: number) =>
  flow(page).getByRole("heading", { name: named, level });

test("a scene taken out of the book can be brought back below another", async ({
  page,
}) => {
  // given: a book of two named scenes open in the writing view
  await aNewProject(page, "BringBack");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await titled(page, first, "The arrival");
  await titled(page, second, "The storm");
  await openTheWritingView(page);

  // and: the first taken out of the book
  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();
  await expect(scene(page, first)).toHaveCount(0);

  // when: bringing a scene back is chosen below the second
  await addHere(gapBelow(page, second)).click();
  await menuItem(page, "Bring back a scene…").click();
  // then: only the scene outside the book is offered
  await expect(
    bringingBack(page).getByRole("button"),
    "only scenes outside the book are offered",
  ).toHaveText(["The arrival", "Cancel"]);
  // when: it is chosen
  await bringingBack(page).getByRole("button", { name: "The arrival" }).click();

  // then: it is back in the book below the second
  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, first)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", [second, first]]]);
});

test("a heading's + brings a scene back to the start of its section", async ({
  page,
}) => {
  // given: a book of one scene and a named scene outside it, in the writing view
  await aNewProject(page, "BringBackFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  const loose = await aLooseScene(page);
  await titled(page, loose, "The prologue");
  await openTheWritingView(page);

  // when: a scene is brought back at the start of the section
  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "Bring back a scene…").click();
  await bringingBack(page)
    .getByRole("button", { name: "The prologue" })
    .click();

  // then: it is back in the book before the other
  await expect(scene(page, loose)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", [loose, placed]]]);
});

test("with every scene in the book there is nothing to bring back", async ({
  page,
}) => {
  // given: every scene in the book
  const scenes = await aBookInTheWritingView(page, "NothingBack", { "Part One": 1 });

  // when: bringing a scene back is chosen
  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "Bring back a scene…").click();
  // then: it says there is nothing to bring back
  await expect(bringingBack(page)).toContainText(
    "Every scene is in the book already.",
  );
  // when: it is cancelled
  await bringingBack(page).getByRole("button", { name: "Cancel" }).click();

  // then: it closes
  await expect(bringingBack(page)).toHaveCount(0);
});

test("a gap's + shows only when the pointer comes near the gap", async ({
  page,
}) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "NearTheGap");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  const plus = addHere(gapBelow(page, first));

  // when: the pointer is on a scene
  await scene(page, first).hover();
  // then: no gap's + shows
  await expect(plus).toHaveCSS("opacity", "0");
  await expect(addHere(gapBelow(page, second))).toHaveCSS("opacity", "0");

  // when: the pointer comes near a gap
  await gapBelow(page, first).locator(".reach").hover();
  // then: its + shows
  await expect(plus).toHaveCSS("opacity", "1");
});

test("the gap between two scenes leaves the writing clickable", async ({
  page,
}) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "ClickThrough");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  // expect: each scene's first line belongs to the scene, not to the gap above it
  for (const id of [first, second]) {
    const written = (await scene(page, id).boundingBox())!;
    const hit = await page.evaluate(
      ([x, y]) => document.elementFromPoint(x, y)?.closest(".reach") === null,
      [written.x + 20, written.y + 4],
    );
    expect(hit, "a scene's first line belongs to the scene").toBe(true);
  }
});

test("the scenes to bring back can be narrowed by what they are called", async ({
  page,
}) => {
  // given: a book of one scene and three named scenes outside it, in the writing view
  await aNewProject(page, "FindScene");
  const { placed } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
    "The stranger",
  ]);
  await openTheWritingView(page);

  // when: bringing a scene back is chosen
  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  // then: all three are offered, the search ready
  const finding = bringingBack(page).getByRole("searchbox", {
    name: "Find a scene",
  });
  await expect(finding).toBeFocused();
  await expect(offered(page)).toHaveCount(3);

  // when: part of a name is typed
  await page.keyboard.type("the st");
  // then: only the scenes so called are offered
  await expect(offered(page)).toHaveText(["The storm", "The stranger"]);
  // when: more is typed
  await page.keyboard.type("orm");
  // then: only the one is offered
  await expect(offered(page)).toHaveText(["The storm"]);
  // when: nothing matches
  await finding.fill("nothing like it");
  // then: it says so
  await expect(bringingBack(page)).toContainText("No scene matches.");
});

test("Enter in the search brings back the first scene that matches", async ({
  page,
}) => {
  // given: a book of one scene and two named scenes outside it, in the writing view
  await aNewProject(page, "EnterBack");
  const { placed, loose } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
  ]);
  await openTheWritingView(page);

  // when: bringing a scene back is chosen
  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(
    bringingBack(page).getByRole("searchbox", { name: "Find a scene" }),
  ).toBeFocused();
  // and: a name is searched for and Enter pressed
  await page.keyboard.type("letter");
  await page.keyboard.press("Enter");

  // then: the scene that matches is brought back after the other
  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, loose["A letter"])).toBeVisible();
  expect(await theBook(page)).toEqual([
    ["Part One", [placed, loose["A letter"]]],
  ]);
});

test("New scene in the gap between two scenes begins one right there", async ({
  page,
}) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "NewBetween");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  // when: a new scene is begun in the gap between them and written in
  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Mitten drin.");

  // then: it is made between them
  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(3);
  const made = (await theBook(page))[0][1][1];
  expect((await theBook(page))[0][1]).toEqual([first, made, second]);
  // and: holds what was written
  await expect(scene(page, made)).toHaveText("Mitten drin.");
});

test("New scene at a section's start begins one before its first scene", async ({
  page,
}) => {
  // given: a book of one scene open in the writing view
  await aNewProject(page, "NewFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  await openTheWritingView(page);

  // when: a new scene is begun at the section's start and written in
  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Ganz vorne.");

  // then: it is made before the other
  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(2);
  const [made, after] = (await theBook(page))[0][1];
  expect(after).toBe(placed);
  // and: holds what was written
  await expect(scene(page, made)).toHaveText("Ganz vorne.");
});

test("New scene in an empty section writes into its opening line", async ({
  page,
}) => {
  // given: an empty section open in the writing view
  await aNewProject(page, "NewEmpty");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  // when: a new scene is begun in it
  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();

  // then: its opening line takes the cursor
  await expect(newScene(page)).toHaveCount(1);
  await expect(newScene(page)).toBeFocused();
});

test("a new scene left empty goes away and makes nothing", async ({ page }) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "NewLeftEmpty");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  // when: a new scene is begun between them and left empty
  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await heading(page, "Part One").click();

  // then: it goes
  await expect(newScene(page)).toHaveCount(0);
  // and: the book is as it was
  expect(await theBook(page)).toEqual([["Part One", [first, second]]]);
});

test("New section after a section's last scene starts the next one at its level", async ({
  page,
}) => {
  // given: two sections, the first with a scene, open in the writing view
  const scenes = await aBookInTheWritingView(page, "NewSectionAfter", { "Part One": 1, "Part Two": 0 });

  // when: a new section is begun after the scene and named
  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "New section").click();
  await namingTheNewSection(page, "Interlude");

  // then: it is saved between the two
  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Interlude",
    "Part Two",
  ]);
  // and: at their level
  await expect(
    flow(page).getByRole("heading", { name: "Interlude", level: 2 }),
  ).toBeVisible();
});

test("New section after the scenes of a section with subsections starts its first subsection", async ({
  page,
}) => {
  // given: a section with a scene and a subsection, open in the writing view
  await aNewProject(page, "NewSubsection");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  const placed = await aLooseScene(page);
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: placed },
      section: ids["Part One"],
      after: null,
    },
  });
  await openTheWritingView(page);

  // when: a new section is begun after the scene and named
  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New section").click();
  await namingTheNewSection(page, "Prelude");

  // then: it is saved before the subsection
  expect(await savedTitles(page)).toEqual(["Part One", "Prelude", "Chapter 1"]);
  // and: at its level
  await expect(
    flow(page).getByRole("heading", { name: "Prelude", level: 3 }),
    "it sits where the gap was, so it nests beside the subsections",
  ).toBeVisible();
});

test("New section is offered only where nothing has to move", async ({
  page,
}) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "OnlyAtTheEnd");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  // when: something is to be added between them
  await addHere(gapBelow(page, first)).click();
  // then: a new scene is offered, but no new section
  await expect(menuItem(page, "New scene")).toBeVisible();
  await expect(menuItem(page, "New section")).toHaveCount(0);
  // when: something is to be added at the section's start
  await page.keyboard.press("Escape");

  await addHere(gapAtStartOf(page, "Part One")).click();
  // then: no new section is offered
  await expect(menuItem(page, "New section")).toHaveCount(0);
  // when: something is to be added after the last scene
  await page.keyboard.press("Escape");

  await addHere(gapBelow(page, second)).click();
  // then: a new section is offered
  await expect(menuItem(page, "New section")).toBeVisible();
});

test("before the first section, New section begins the book", async ({
  page,
}) => {
  // given: a book of a section and its subsection, open in the writing view
  await aNewProject(page, "NewFirstSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: something is to be added before the first section
  await addHere(bookStart(page)).click();
  // then: no new scene is offered
  await expect(menuItem(page, "New scene")).toHaveCount(0);
  // when: a new section is begun there and named
  await menuItem(page, "New section").click();
  await namingTheNewSection(page, "Prologue");

  // then: it begins the book
  expect(await savedTitles(page)).toEqual([
    "Prologue",
    "Part One",
    "Chapter 1",
  ]);
});

test("after everything, New section ends the book at the top level", async ({
  page,
}) => {
  // given: a book of a section and its subsection, open in the writing view
  await aNewProject(page, "NewLastSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: a new section is begun after everything and named
  await addHere(bookEnd(page)).click();
  await menuItem(page, "New section").click();
  await namingTheNewSection(page, "Epilogue");

  // then: it ends the book
  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  // and: at the top level
  await expect(
    flow(page).getByRole("heading", { name: "Epilogue", level: 2 }),
  ).toBeVisible();
});

test("a book ending in a scene has one gap at its end, not two", async ({
  page,
}) => {
  // given: a book ending in a scene, open in the writing view
  const scenes = await aBookInTheWritingView(page, "OneGapAtTheEnd", { "Part One": 1 });
  await expect(scene(page, scenes["Part One"][0])).toBeVisible();

  // then: there is no gap after the book's end
  await expect(bookEnd(page)).toHaveCount(0);
  // when: something is to be added after the last scene
  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  // then: a new section is offered
  await expect(menuItem(page, "New section")).toBeVisible();
  // and: no new top-level section
  await expect(
    menuItem(page, "New top-level section"),
    "a section at its level already is one",
  ).toHaveCount(0);
});

test("the last gap of a book ending in a nested section can still end the book at the top level", async ({
  page,
}) => {
  // given: a book ending in a scene of a subsection, open in the writing view
  await aNewProject(page, "NestedEnd");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  const placed = await aLooseScene(page);
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: placed },
      section: ids["Chapter 1"],
      after: null,
    },
  });
  await openTheWritingView(page);
  await expect(scene(page, placed)).toBeVisible();
  await expect(bookEnd(page)).toHaveCount(0);

  // when: a new top-level section is begun after the scene and named
  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New top-level section").click();
  await namingTheNewSection(page, "Epilogue");

  // then: it ends the book
  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  // and: at the top level
  await expect(headingAt(page, "Epilogue", 2)).toBeVisible();
});

test("Tab in a heading nests its section under the one before, and the cursor stays", async ({
  page,
}) => {
  // given: two sections open in the writing view
  await aNewProject(page, "TabNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: Tab is pressed in the second's heading
  await heading(page, "Part Two").click();
  await page.keyboard.press("Tab");

  // then: it is nested under the first
  await expect(headingAt(page, "Part Two", 3)).toBeVisible();
  // and: keeps the cursor
  await expect(headingAt(page, "Part Two", 3)).toBeFocused();
  // and: it is saved so
  await expect
    .poll(async () =>
      (await projectsOutline(page)).sections.map(
        (section: { title: string; parent: string | null }) => [
          section.title,
          section.parent !== null,
        ],
      ),
    )
    .toEqual([
      ["Part One", false],
      ["Part Two", true],
    ]);
});

test("Shift+Tab in a heading lifts its section out a level", async ({
  page,
}) => {
  // given: a section and its subsection, open in the writing view
  await aNewProject(page, "ShiftTabLifts");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: Shift+Tab is pressed in the subsection's heading
  await heading(page, "Chapter 1").click();
  await page.keyboard.press("Shift+Tab");

  // then: it is lifted out a level
  await expect(headingAt(page, "Chapter 1", 2)).toBeVisible();
  // and: keeps the cursor
  await expect(headingAt(page, "Chapter 1", 2)).toBeFocused();
});

test("a heading's title being typed survives the heading being nested", async ({
  page,
}) => {
  // given: two sections open in the writing view
  await aNewProject(page, "TypedThenTab");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: the second's heading is typed into and Tab pressed
  await heading(page, "Part Two").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" and more");
  await page.keyboard.press("Tab");

  // then: it is nested with the new title
  await expect(headingAt(page, "Part Two and more", 3)).toBeVisible();
  // and: the title is saved
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two and more"]);
});

test("a section's menu nests and lifts it, offering only what is possible", async ({
  page,
}) => {
  // given: two sections open in the writing view
  await aNewProject(page, "MenuNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: the first section's menu is opened
  await sectionActions(page, "Part One").click();
  // then: it can be removed but neither nested nor lifted
  await expect(menuItem(page, "Remove section")).toBeVisible();
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  await expect(menuItem(page, "Lift out a level")).toHaveCount(0);
  await page.keyboard.press("Escape");

  // when: the second is nested from its menu
  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Nest under the previous section").click();
  // then: it is nested
  await expect(headingAt(page, "Part Two", 3)).toBeVisible();

  // when: its menu is opened again
  await sectionActions(page, "Part Two").click();
  // then: it cannot be nested further
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  // when: it is lifted from its menu
  await menuItem(page, "Lift out a level").click();
  // then: it is lifted
  await expect(headingAt(page, "Part Two", 2)).toBeVisible();
});

test("nesting and lifting in a section's menu show their keys, faded beside them", async ({
  page,
}) => {
  // given: two sections open in the writing view
  await aNewProject(page, "MenuShowsKeys");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  // when: the second section's menu is opened
  await sectionActions(page, "Part Two").click();
  // then: nesting shows its key
  const nesting = flow(page).getByRole("menuitem", {
    name: "Nest under the previous section",
    exact: true,
  });
  await expect(nesting).toHaveAttribute("aria-keyshortcuts", "Tab");
  await expect(nesting.locator("kbd")).toHaveText("Tab");
  // and: an item without a key shows none
  await expect(
    menuItem(page, "Remove section").locator("kbd"),
    "an item without a key shows none",
  ).toHaveCount(0);
  // when: it is nested and its menu opened again
  await nesting.click();

  await sectionActions(page, "Part Two").click();
  // then: lifting shows its key
  const lifting = flow(page).getByRole("menuitem", {
    name: "Lift out a level",
    exact: true,
  });
  await expect(lifting).toHaveAttribute("aria-keyshortcuts", "Shift+Tab");
  await expect(lifting.locator("kbd")).toHaveText("Shift+Tab");
});
