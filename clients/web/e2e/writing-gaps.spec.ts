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

async function aNewSectionNamed(page: Page, title: string) {
  await expect(page.locator(":focus")).toHaveAttribute("role", "heading");
  await page.keyboard.type(title);
  await expect.poll(() => savedTitles(page)).toContain(title);
}

const headingAt = (page: Page, named: string, level: number) =>
  flow(page).getByRole("heading", { name: named, level });

test("a scene taken out of the book can be brought back below another", async ({
  page,
}) => {
  await aNewProject(page, "BringBack");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await titled(page, first, "The arrival");
  await titled(page, second, "The storm");
  await openTheWritingView(page);

  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();
  await expect(scene(page, first)).toHaveCount(0);

  await addHere(gapBelow(page, second)).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(
    bringingBack(page).getByRole("button"),
    "only scenes outside the book are offered",
  ).toHaveText(["The arrival", "Cancel"]);
  await bringingBack(page).getByRole("button", { name: "The arrival" }).click();

  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, first)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", [second, first]]]);
});

test("a heading's + brings a scene back to the start of its section", async ({
  page,
}) => {
  await aNewProject(page, "BringBackFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  const loose = await aLooseScene(page);
  await titled(page, loose, "The prologue");
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "Bring back a scene…").click();
  await bringingBack(page)
    .getByRole("button", { name: "The prologue" })
    .click();

  await expect(scene(page, loose)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", [loose, placed]]]);
});

test("with every scene in the book there is nothing to bring back", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "NothingBack", { "Part One": 1 });

  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(bringingBack(page)).toContainText(
    "Every scene is in the book already.",
  );
  await bringingBack(page).getByRole("button", { name: "Cancel" }).click();

  await expect(bringingBack(page)).toHaveCount(0);
});

test("a gap's + shows only when the pointer comes near the gap", async ({
  page,
}) => {
  await aNewProject(page, "NearTheGap");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  const plus = addHere(gapBelow(page, first));

  await scene(page, first).hover();
  await expect(plus).toHaveCSS("opacity", "0");
  await expect(addHere(gapBelow(page, second))).toHaveCSS("opacity", "0");

  await gapBelow(page, first).locator(".reach").hover();
  await expect(plus).toHaveCSS("opacity", "1");
});

test("the gap between two scenes leaves the writing clickable", async ({
  page,
}) => {
  await aNewProject(page, "ClickThrough");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

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
  await aNewProject(page, "FindScene");
  const { placed } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
    "The stranger",
  ]);
  await openTheWritingView(page);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  const finding = bringingBack(page).getByRole("searchbox", {
    name: "Find a scene",
  });
  await expect(finding).toBeFocused();
  await expect(offered(page)).toHaveCount(3);

  await page.keyboard.type("the st");
  await expect(offered(page)).toHaveText(["The storm", "The stranger"]);
  await page.keyboard.type("orm");
  await expect(offered(page)).toHaveText(["The storm"]);
  await finding.fill("nothing like it");
  await expect(bringingBack(page)).toContainText("No scene matches.");
});

test("Enter in the search brings back the first scene that matches", async ({
  page,
}) => {
  await aNewProject(page, "EnterBack");
  const { placed, loose } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
  ]);
  await openTheWritingView(page);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(
    bringingBack(page).getByRole("searchbox", { name: "Find a scene" }),
  ).toBeFocused();
  await page.keyboard.type("letter");
  await page.keyboard.press("Enter");

  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, loose["A letter"])).toBeVisible();
  expect(await theBook(page)).toEqual([
    ["Part One", [placed, loose["A letter"]]],
  ]);
});

test("New scene in the gap between two scenes begins one right there", async ({
  page,
}) => {
  await aNewProject(page, "NewBetween");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Mitten drin.");

  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(3);
  const made = (await theBook(page))[0][1][1];
  expect((await theBook(page))[0][1]).toEqual([first, made, second]);
  await expect(scene(page, made)).toHaveText("Mitten drin.");
});

test("New scene at a section's start begins one before its first scene", async ({
  page,
}) => {
  await aNewProject(page, "NewFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Ganz vorne.");

  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(2);
  const [made, after] = (await theBook(page))[0][1];
  expect(after).toBe(placed);
  await expect(scene(page, made)).toHaveText("Ganz vorne.");
});

test("New scene in an empty section writes into its opening line", async ({
  page,
}) => {
  await aNewProject(page, "NewEmpty");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();

  await expect(newScene(page)).toHaveCount(1);
  await expect(newScene(page)).toBeFocused();
});

test("a new scene left empty goes away and makes nothing", async ({ page }) => {
  await aNewProject(page, "NewLeftEmpty");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await heading(page, "Part One").click();

  await expect(newScene(page)).toHaveCount(0);
  expect(await theBook(page)).toEqual([["Part One", [first, second]]]);
});

test("New section after a section's last scene starts the next one at its level", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "NewSectionAfter", { "Part One": 1, "Part Two": 0 });

  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Interlude");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Interlude",
    "Part Two",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Interlude", level: 2 }),
  ).toBeVisible();
});

test("New section after the scenes of a section with subsections starts its first subsection", async ({
  page,
}) => {
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

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Prelude");

  expect(await savedTitles(page)).toEqual(["Part One", "Prelude", "Chapter 1"]);
  await expect(
    flow(page).getByRole("heading", { name: "Prelude", level: 3 }),
    "it sits where the gap was, so it nests beside the subsections",
  ).toBeVisible();
});

test("New section is offered only where nothing has to move", async ({
  page,
}) => {
  await aNewProject(page, "OnlyAtTheEnd");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await expect(menuItem(page, "New scene")).toBeVisible();
  await expect(menuItem(page, "New section")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await addHere(gapAtStartOf(page, "Part One")).click();
  await expect(menuItem(page, "New section")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await addHere(gapBelow(page, second)).click();
  await expect(menuItem(page, "New section")).toBeVisible();
});

test("before the first section, New section begins the book", async ({
  page,
}) => {
  await aNewProject(page, "NewFirstSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await addHere(bookStart(page)).click();
  await expect(menuItem(page, "New scene")).toHaveCount(0);
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Prologue");

  expect(await savedTitles(page)).toEqual([
    "Prologue",
    "Part One",
    "Chapter 1",
  ]);
});

test("after everything, New section ends the book at the top level", async ({
  page,
}) => {
  await aNewProject(page, "NewLastSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await addHere(bookEnd(page)).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Epilogue");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Epilogue", level: 2 }),
  ).toBeVisible();
});

test("a book ending in a scene has one gap at its end, not two", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "OneGapAtTheEnd", { "Part One": 1 });
  await expect(scene(page, scenes["Part One"][0])).toBeVisible();

  await expect(bookEnd(page)).toHaveCount(0);
  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await expect(menuItem(page, "New section")).toBeVisible();
  await expect(
    menuItem(page, "New top-level section"),
    "a section at its level already is one",
  ).toHaveCount(0);
});

test("the last gap of a book ending in a nested section can still end the book at the top level", async ({
  page,
}) => {
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

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New top-level section").click();
  await aNewSectionNamed(page, "Epilogue");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  await expect(headingAt(page, "Epilogue", 2)).toBeVisible();
});

test("Tab in a heading nests its section under the one before, and the cursor stays", async ({
  page,
}) => {
  await aNewProject(page, "TabNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Part Two").click();
  await page.keyboard.press("Tab");

  await expect(headingAt(page, "Part Two", 3)).toBeVisible();
  await expect(headingAt(page, "Part Two", 3)).toBeFocused();
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
  await aNewProject(page, "ShiftTabLifts");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Chapter 1").click();
  await page.keyboard.press("Shift+Tab");

  await expect(headingAt(page, "Chapter 1", 2)).toBeVisible();
  await expect(headingAt(page, "Chapter 1", 2)).toBeFocused();
});

test("a heading's title being typed survives the heading being nested", async ({
  page,
}) => {
  await aNewProject(page, "TypedThenTab");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Part Two").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" and more");
  await page.keyboard.press("Tab");

  await expect(headingAt(page, "Part Two and more", 3)).toBeVisible();
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two and more"]);
});

test("a section's menu nests and lifts it, offering only what is possible", async ({
  page,
}) => {
  await aNewProject(page, "MenuNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await sectionActions(page, "Part One").click();
  await expect(menuItem(page, "Remove section")).toBeVisible();
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  await expect(menuItem(page, "Lift out a level")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Nest under the previous section").click();
  await expect(headingAt(page, "Part Two", 3)).toBeVisible();

  await sectionActions(page, "Part Two").click();
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  await menuItem(page, "Lift out a level").click();
  await expect(headingAt(page, "Part Two", 2)).toBeVisible();
});
