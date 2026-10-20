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
  await aNewProject(page, "Sides");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await expect(scene(page, placed)).toBeVisible();
  await expect(scene(page, loose)).toHaveCount(0);
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");

  await openTheLooseSide(page);

  await expect(scene(page, loose)).toBeVisible();
  await expect(scene(page, placed)).toHaveCount(0);
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
  await aNewProject(page, "LooseWriting");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);

  await scene(page, loose).click();
  await page.keyboard.type("Cut, but kept.");

  await expect(saveState(page)).toHaveText("All saved");
  await expect
    .poll(async () => (await (await sceneOnServer(page, loose)).json()).text)
    .toBe("Cut, but kept.");
});

test("the arrow keys walk among the scenes outside the book", async ({
  page,
}) => {
  await aNewProject(page, "LooseWalk");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  const second = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, first).click();
  await page.keyboard.type("First.");

  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Second.");

  await expect(scene(page, second)).toHaveText("Second.");
});

test("each side counts its own words", async ({ page }) => {
  await aNewProject(page, "LooseCount");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, placed).click();
  await page.keyboard.type("Two words.");
  await expect(saveState(page)).toHaveText("All saved");

  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Three more words.");

  await expect(statusNote(page).locator(".word-count")).toHaveText(
    "3 words not in the book",
  );
  await expect(saveState(page)).toHaveText("All saved");
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();
  await expect(statusNote(page).locator(".word-count")).toContainText(
    "2 words",
  );
});

test("the tray names a scene outside the book", async ({ page }) => {
  await aNewProject(page, "LooseTray");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);

  await scene(page, loose).click();
  await inspectorTab(page).click();

  await expect(inspectedSceneTitle(page)).toHaveValue("The cut scene");
});

test("with every scene in the book, the switch stays out of the way and the other side says so", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "NoLoose", { "Part One": 1 });
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(sides(page)).toHaveCount(0);

  await page.goto(page.url().replace(/\/write$/, "/write/loose"));

  await expect(flow(page)).toContainText("Every scene is in the book.");
  await expect(flow(page).locator(".scene")).toHaveCount(0);
  await expect(
    sides(page),
    "on the other side the switch is the only way back",
  ).toBeVisible();
});

test("a scene outside the book is offered its own ways, and Enter twice begins nothing", async ({
  page,
}) => {
  await aNewProject(page, "LooseMenu");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Alone.");

  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Still here.");
  await sceneActions(page, loose).click();

  await expect(
    flow(page).getByRole("menuitem"),
    "taking out of the book means nothing for a scene not in it",
  ).toHaveText(["Rename", "Bring into the book…", "Delete scene"]);
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(
    scene(page, loose).locator("p"),
    "with no next scene to begin, Enter twice is two line breaks like anywhere else",
  ).toHaveText(["Alone.", "", "Still here."]);
});

test("writing outside the book leaves where you left off in the book alone", async ({
  page,
}) => {
  await aNewProject(page, "LooseLeftOff");
  const second = (await aBookWithScenes(page, { "Part One": 14 }))[
    "Part One"
  ][11];
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

  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  await backToWhereILeftOff(page);
  await expect(scene(page, second)).toBeFocused();
});

test("a scene left empty where I left off is offered back as an empty scene", async ({
  page,
}) => {
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
  await openTheLooseSide(page);

  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  await expect(
    backToWhereYouLeftOffButton(page).locator(".where"),
  ).toHaveText("An empty scene");
});

test("a scene outside the book is brought into the section chosen, and the cursor lands in it", async ({
  page,
}) => {
  await aNewProject(page, "BringingIn");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();

  await expect(placingDialog(page)).toContainText(
    "“The cut scene” goes at the end of the section you choose.",
  );
  await expect(
    placingDialog(page)
      .getByRole("list", { name: "Sections of the book" })
      .getByRole("button"),
  ).toHaveText(["Part One", "Part Two"]);
  await placingDialog(page).getByRole("button", { name: "Part Two" }).click();

  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, loose)).toBeFocused();
  await expect
    .poll(() => theBook(page))
    .toEqual([
      ["Part One", scenes["Part One"]],
      ["Part Two", [...scenes["Part Two"], loose]],
    ]);
  await expect(
    sides(page),
    "with the last scene brought in, nothing is left outside the book",
  ).toHaveCount(0);
});

test("a scene outside the book is deleted after asking", async ({ page }) => {
  await aNewProject(page, "LooseDelete");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Delete scene").click();

  await asked(page).getByRole("button", { name: "Delete" }).click();

  await expect(scene(page, loose)).toHaveCount(0);
  await expect(flow(page)).toContainText("Every scene is in the book.");
  await expect(sides(page)).toBeVisible();
  await expect
    .poll(async () => (await sceneOnServer(page, loose)).status())
    .toBe(404);
});

test("Escape in the section dialog leaves the scene outside the book", async ({
  page,
}) => {
  await aNewProject(page, "BringingInEscape");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();
  await expect(placingDialog(page)).toBeVisible();

  await page.keyboard.press("Escape");

  await expect(placingDialog(page)).toBeHidden();
  await expect(scene(page, loose)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", scenes["Part One"]]]);
});

test("a scene outside the book followed into the book's side opens the other side at it", async ({
  page,
}) => {
  await aNewProject(page, "FollowLoose");
  await aBookWithScenes(page, { "Part One": 1 });
  await aLooseScene(page);
  const wanted = await aLooseScene(page);
  await openTheWritingView(page);

  await following(page, "/write", wanted);

  await expect(page).toHaveURL(/\/write\/loose$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("a scene in the book followed into the other side goes back to the book", async ({
  page,
}) => {
  await aNewProject(page, "FollowPlaced");
  const [, wanted] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await aLooseScene(page);
  await openTheWritingView(page);

  await following(page, "/write/loose", wanted);

  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("each side lands where you left off on it", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
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

  await page.reload();
  await backToWhereILeftOff(page);
  await expect(scene(page, loose)).toBeFocused();
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  await backToWhereILeftOff(page);
  await expect(scene(page, placed)).toBeFocused();
});

test("the other side starts in its first scene when nothing was left off there", async ({
  page,
}) => {
  await aNewProject(page, "LooseFirst");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  await aLooseScene(page);
  await openTheWritingView(page);

  await openTheLooseSide(page);

  await expect(scene(page, first)).toBeFocused();
});

test("a scene begun in the view and taken out of the book shows up outside it at once", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "TakenOutAtOnce", { "Part One": 0 });
  const begun = await aSceneBegunHere(page);
  await expect(sides(page)).toHaveCount(0);

  await sceneActions(page, begun).click();
  await menuItem(page, "Take out of the book").click();

  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
    "the view knows of the scene it began, though the listing it opened with does not",
  ).toContainText("1");
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toHaveText("Begun here.");
});

test("the scenes of a section removed from the book show up outside it at once", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "RemovedAtOnce", {
    "Part One": 0,
    "Part Two": 1,
  });
  const begun = await aSceneBegunHere(page);

  await sectionActions(page, "Part One").click();
  await menuItem(page, "Remove section").click();
  await asked(page).getByRole("button", { name: "Remove" }).click();

  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toBeVisible();
});
