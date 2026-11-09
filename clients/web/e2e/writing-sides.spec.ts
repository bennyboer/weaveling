import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject } from "./support/shell";
import {
  aBookInTheWritingView,
  asked,
  backToWhereILeftOff,
  backToWhereYouLeftOffButton,
  beginning,
  flow,
  following,
  menuItem,
  offered,
  openTheWritingView,
  saveState,
  scene,
  sceneActions,
  sceneOnServer,
  inspectorTab,
  sectionActions,
  statusNote,
  theBook,
  titled,
  inspectedSceneTitle,
} from "./support/writing";

const sides = (page: Page) =>
  page.getByRole("navigation", { name: "Sides of the work" });

async function openTheLooseSide(page: Page) {
  await sides(page)
    .getByRole("link", { name: /Not in the book/ })
    .click();
  await expect(page).toHaveURL(/\/write\/loose$/);
}

const placingDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Bring into the book" });

async function aSceneBegunHere(page: Page) {
  await beginning(page).click();
  await page.keyboard.type("Begun here.");
  await expect(beginning(page)).toHaveCount(0);
  await expect(saveState(page)).toHaveText("All saved");
  const [begun] = await flow(page)
    .locator(".scene[data-scene]")
    .evaluateAll((all) => all.map((one) => one.getAttribute("data-scene")!));

  return begun;
}

test("the switch shows the book on one side and the scenes outside it on the other", async ({
  page,
}) => {
  // given: a scene in the book and one outside it, the book open in the writing view
  await aNewProject(page, "Sides");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  // then: the book's scene is shown, not the other
  await expect(scene(page, placed)).toBeVisible();
  await expect(scene(page, loose)).toHaveCount(0);
  // and: the switch counts one scene outside the book
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");

  // when: the other side is opened
  await openTheLooseSide(page);

  // then: the scene outside the book is shown, not the book's
  await expect(scene(page, loose)).toBeVisible();
  await expect(scene(page, placed)).toHaveCount(0);
  // and: the switch marks the other side
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toHaveAttribute("aria-current", "page");
  await expect(
    sides(page).getByRole("link", { name: "The book", exact: true }),
  ).not.toHaveAttribute("aria-current", "page");
});

test("a scene outside the book is written in on its side and kept", async ({
  page,
}) => {
  // given: a scene outside the book, its side open
  await aNewProject(page, "LooseWriting");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);

  // when: it is written in
  await scene(page, loose).click();
  await page.keyboard.type("Cut, but kept.");

  // then: what is written is saved
  await expect(saveState(page)).toHaveText("All saved");
  await expect
    .poll(async () => (await (await sceneOnServer(page, loose)).json()).text)
    .toBe("Cut, but kept.");
});

test("the arrow keys walk among the scenes outside the book", async ({
  page,
}) => {
  // given: two scenes outside the book, the cursor in the first
  await aNewProject(page, "LooseWalk");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  const second = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, first).click();
  await page.keyboard.type("First.");

  // when: the cursor is moved down and something typed
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Second.");

  // then: it lands in the second
  await expect(scene(page, second)).toHaveText("Second.");
});

test("each side counts its own words", async ({ page }) => {
  // given: a scene in the book with two words, and a scene outside it
  await aNewProject(page, "LooseCount");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, placed).click();
  await page.keyboard.type("Two words.");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the scene outside the book is written in
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Three more words.");

  // then: its side counts its words
  await expect(statusNote(page).locator(".word-count")).toHaveText(
    "3 words not in the book",
  );
  // when: the book is opened again
  await expect(saveState(page)).toHaveText("All saved");
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();
  // then: the book counts its own
  await expect(statusNote(page).locator(".word-count")).toContainText(
    "2 words",
  );
});

test("the tray names a scene outside the book", async ({ page }) => {
  // given: a named scene outside the book, its side open
  await aNewProject(page, "LooseTray");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);

  // when: the tray is opened on it
  await scene(page, loose).click();
  await inspectorTab(page).click();

  // then: the tray names it
  await expect(inspectedSceneTitle(page)).toHaveValue("The cut scene");
});

test("with every scene in the book, the switch stays out of the way and the other side says so", async ({
  page,
}) => {
  // given: every scene in the book, open in the writing view
  await aBookInTheWritingView(page, "NoLoose", { "Part One": 1 });
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  // then: no switch is shown
  await expect(sides(page)).toHaveCount(0);

  // when: the other side is opened
  await expect(page).toHaveURL(/\/write$/);
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));

  // then: it says every scene is in the book
  await expect(flow(page)).toContainText("Every scene is in the book.");
  await expect(flow(page).locator(".scene")).toHaveCount(0);
  // and: the switch leads back
  await expect(
    sides(page),
    "on the other side the switch is the only way back",
  ).toBeVisible();
});

test("a scene outside the book is offered its own ways, and Enter twice begins nothing", async ({
  page,
}) => {
  // given: a scene outside the book, written in on its side
  await aNewProject(page, "LooseMenu");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Alone.");

  // when: Enter is pressed twice and more typed
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Still here.");
  // and: its menu is opened
  await sceneActions(page, loose).click();

  // then: it offers what suits a scene outside the book
  await expect(
    flow(page).getByRole("menuitem"),
    "taking out of the book means nothing for a scene not in it",
  ).toHaveText(["Rename", "Bring into the book…", "Delete scene"]);
  // and: no scene was begun
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  // and: the scene has two line breaks
  await expect(
    scene(page, loose).locator("p"),
    "with no next scene to begin, Enter twice is two line breaks like anywhere else",
  ).toHaveText(["Alone.", "", "Still here."]);
});

test("writing outside the book leaves where you left off in the book alone", async ({
  page,
}) => {
  // given: a scene far down the book written in last there
  await aNewProject(page, "LooseLeftOff");
  const second = (await aBookWithScenes(page, { "Part One": 14 }))[
    "Part One"
  ][11];
  // and: a scene outside the book written in after it
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${second}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, second).click();
  await page.keyboard.type("Here.");
  await expect(saveState(page)).toHaveText("All saved");
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Elsewhere.");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the book is opened again
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is in the book's scene
  await expect(scene(page, second)).toBeFocused();
});

test("a scene left empty where I left off is offered back as an empty scene", async ({
  page,
}) => {
  // given: a scene far down the book left empty where the author left off
  await aNewProject(page, "LeftEmpty");
  const second = (await aBookWithScenes(page, { "Part One": 14 }))[
    "Part One"
  ][11];
  await aLooseScene(page);
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${second}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, second).click();
  await page.keyboard.type("x");
  await page.keyboard.press("Backspace");
  await expect(saveState(page)).toHaveText("All saved");
  // and: the other side opened
  await openTheLooseSide(page);

  // when: the book is opened again
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  // then: the way back names it as an empty scene
  await expect(
    backToWhereYouLeftOffButton(page).locator(".where"),
  ).toHaveText("An empty scene");
});

test("a scene outside the book is brought into the section chosen, and the cursor lands in it", async ({
  page,
}) => {
  // given: a named scene outside the book, with two sections in it
  await aNewProject(page, "BringingIn");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);
  // when: bringing it into the book is chosen
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();

  // then: it asks which section
  await expect(placingDialog(page)).toContainText(
    "“The cut scene” goes at the end of the section you choose.",
  );
  // and: offers each
  await expect(
    placingDialog(page)
      .getByRole("list", { name: "Sections of the book" })
      .getByRole("button"),
  ).toHaveText(["Part One", "Part Two"]);
  // when: the second section is chosen
  await placingDialog(page).getByRole("button", { name: "Part Two" }).click();

  // then: the book is opened with the cursor in the scene
  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, loose)).toBeFocused();
  // and: the scene is saved at the end of the section
  await expect
    .poll(() => theBook(page))
    .toEqual([
      ["Part One", scenes["Part One"]],
      ["Part Two", [...scenes["Part Two"], loose]],
    ]);
  // and: the switch goes, with nothing left outside the book
  await expect(
    sides(page),
    "with the last scene brought in, nothing is left outside the book",
  ).toHaveCount(0);
});

test("a scene outside the book is deleted after asking", async ({ page }) => {
  // given: deleting a scene outside the book asked
  await aNewProject(page, "LooseDelete");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Delete scene").click();

  // when: it is deleted
  await asked(page).getByRole("button", { name: "Delete" }).click();

  // then: it is gone
  await expect(scene(page, loose)).toHaveCount(0);
  // and: the side says every scene is in the book
  await expect(flow(page)).toContainText("Every scene is in the book.");
  // and: the switch still leads back
  await expect(sides(page)).toBeVisible();
  // and: the scene is gone for good
  await expect
    .poll(async () => (await sceneOnServer(page, loose)).status())
    .toBe(404);
});

test("Escape in the section dialog leaves the scene outside the book", async ({
  page,
}) => {
  // given: bringing a scene outside the book into it asked
  await aNewProject(page, "BringingInEscape");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();
  await expect(placingDialog(page)).toBeVisible();

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the question goes
  await expect(placingDialog(page)).toBeHidden();
  // and: the scene is still outside the book
  await expect(scene(page, loose)).toBeVisible();
  // and: the book is unchanged
  expect(await theBook(page)).toEqual([["Part One", scenes["Part One"]]]);
});

test("a scene outside the book followed into the book's side opens the other side at it", async ({
  page,
}) => {
  // given: two scenes outside the book, the book open in the writing view
  await aNewProject(page, "FollowLoose");
  await aBookWithScenes(page, { "Part One": 1 });
  await aLooseScene(page);
  const wanted = await aLooseScene(page);
  await openTheWritingView(page);

  // when: one of them is followed
  await following(page, "/write", wanted);

  // then: the other side opens with the cursor in it
  await expect(page).toHaveURL(/\/write\/loose$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("a scene in the book followed into the other side goes back to the book", async ({
  page,
}) => {
  // given: a scene in the book and one outside it, the book open in the writing view
  await aNewProject(page, "FollowPlaced");
  const [, wanted] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await aLooseScene(page);
  await openTheWritingView(page);

  // when: the book's scene is followed on the other side
  await following(page, "/write/loose", wanted);

  // then: the book opens with the cursor in it
  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("each side lands where you left off on it", async ({ page }) => {
  // given: a short window
  await page.setViewportSize({ width: 1280, height: 420 });
  // and: a long book and many scenes outside it, each side written in far down
  await aNewProject(page, "LeftOffSides");
  const placed = (await aBookWithScenes(page, { "Part One": 14 }))[
    "Part One"
  ][11];
  let loose = "";
  for (let made = 0; made < 10; made++) {
    loose = await aLooseScene(page);
  }
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${placed}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, placed).click();
  await page.keyboard.type("In the book.");
  await openTheLooseSide(page);
  await flow(page)
    .locator(`[data-place-of="${loose}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, loose).click();
  await page.keyboard.type("Outside.");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the page is reloaded
  await page.reload();
  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is where she left off outside the book
  await expect(scene(page, loose)).toBeFocused();
  // when: the book is opened again
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is where she left off in the book
  await expect(scene(page, placed)).toBeFocused();
});

test("the other side starts in its first scene when nothing was left off there", async ({
  page,
}) => {
  // given: two scenes outside the book, nothing written on its side
  await aNewProject(page, "LooseFirst");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  await aLooseScene(page);
  await openTheWritingView(page);

  // when: the other side is opened
  await openTheLooseSide(page);

  // then: its first scene has the cursor
  await expect(scene(page, first)).toBeFocused();
});

test("a scene begun in the view and taken out of the book shows up outside it at once", async ({
  page,
}) => {
  // given: a scene begun in the view, all scenes in the book
  await aBookInTheWritingView(page, "TakenOutAtOnce", { "Part One": 0 });
  const begun = await aSceneBegunHere(page);
  await expect(sides(page)).toHaveCount(0);

  // when: it is taken out of the book
  await sceneActions(page, begun).click();
  await menuItem(page, "Take out of the book").click();

  // then: the switch counts it at once
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
    "the view knows of the scene it began, though the listing it opened with does not",
  ).toContainText("1");
  // and: it is outside the book with its words
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toHaveText("Begun here.");
});

test("the scenes of a section removed from the book show up outside it at once", async ({
  page,
}) => {
  // given: a scene begun in the view in a section of its own
  await aBookInTheWritingView(page, "RemovedAtOnce", {
    "Part One": 0,
    "Part Two": 1,
  });
  const begun = await aSceneBegunHere(page);

  // when: the section is removed
  await sectionActions(page, "Part One").click();
  await menuItem(page, "Remove section").click();
  await asked(page).getByRole("button", { name: "Remove" }).click();

  // then: the switch counts the scene at once
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");
  // and: it is outside the book
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toBeVisible();
});
