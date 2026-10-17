import { test, expect, type Page } from "@playwright/test";

import { aNewProject, aLooseScene, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

const scene = (page: Page, id: string) =>
  flow(page).locator(`[data-scene="${id}"] .ProseMirror`);

const chosen = (page: Page) =>
  flow(page)
    .locator(".scene-slot.chosen")
    .evaluateAll((slots) =>
      slots.map((slot) => slot.getAttribute("data-place-of")),
    );

async function aBook(page: Page, named: string, scenes: number) {
  await aNewProject(page, named);
  const made = await aBookWithScenes(page, { "Part One": scenes });
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();

  return made["Part One"];
}

async function written(page: Page, id: string, text: string) {
  await scene(page, id).click();
  await page.keyboard.type(text);
}

test("Shift+Down at a scene's end chooses the scene, then takes in the next, and Shift+Up gives it back", async ({
  page,
}) => {
  const [first, second, third] = await aBook(page, "Down", 3);
  await written(page, first, "The first scene.");

  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);
  await expect(
    scene(page, first),
    "the scene is chosen whole, not edited",
  ).not.toBeFocused();

  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second]);
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second, third]);

  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => chosen(page)).toEqual([first, second]);
});

test("Shift+Up at a scene's top chooses the scene and takes in those above", async ({
  page,
}) => {
  const [first, second, third] = await aBook(page, "Up", 3);
  await written(page, third, "The third scene.");
  await page.keyboard.press("ControlOrMeta+Home");

  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => chosen(page)).toEqual([third]);
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");

  await expect.poll(() => chosen(page)).toEqual([first, second, third]);
});

test("inside a scene, Shift+Down still selects text as it always did", async ({
  page,
}) => {
  const [first] = await aBook(page, "Inside", 2);
  await written(page, first, "One line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Another line.");
  await page.keyboard.press("ControlOrMeta+Home");

  await page.keyboard.press("Shift+ArrowDown");

  await expect.poll(() => chosen(page)).toEqual([]);
  await expect(scene(page, first)).toBeFocused();
  expect(await page.evaluate(() => getSelection()!.toString())).toContain(
    "One line.",
  );
});

test("Ctrl+A chooses the scene's text, and pressed again the whole book", async ({
  page,
}) => {
  const scenes = await aBook(page, "Everything", 3);
  await written(page, scenes[1], "The middle scene.");

  await page.keyboard.press("ControlOrMeta+A");
  await expect.poll(() => chosen(page)).toEqual([]);
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "The middle scene.",
  );

  await page.keyboard.press("ControlOrMeta+A");

  await expect.poll(() => chosen(page)).toEqual(scenes);
});

test("Escape lets the scenes go and puts the cursor back where it was", async ({
  page,
}) => {
  const [first, second] = await aBook(page, "Escaping", 2);
  await written(page, first, "Back here");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second]);

  await page.keyboard.press("Escape");

  await expect.poll(() => chosen(page)).toEqual([]);
  await expect(
    scene(page, second),
    "the cursor goes back into the scene the choosing ended in",
  ).toBeFocused();
});

test("an arrow without Shift lets the scenes go too", async ({ page }) => {
  const [first] = await aBook(page, "Arrow", 2);
  await written(page, first, "Here.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  await page.keyboard.press("ArrowDown");

  await expect.poll(() => chosen(page)).toEqual([]);
  await expect(scene(page, first)).toBeFocused();
  await page.keyboard.type(" More.");
  await expect(scene(page, first)).toHaveText("Here. More.");
});

test("clicking into a scene lets the chosen ones go", async ({ page }) => {
  const [first, second] = await aBook(page, "Clicking", 2);
  await written(page, first, "Chosen.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  await scene(page, second).click();

  await expect.poll(() => chosen(page)).toEqual([]);
});

test("typing while scenes are chosen changes nothing in them", async ({
  page,
}) => {
  const [first] = await aBook(page, "Typing", 2);
  await written(page, first, "Untouched.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  await page.keyboard.type("xyz");

  await expect(scene(page, first)).toHaveText("Untouched.");
});

test("taking in scenes reaches down the book, bringing each into sight", async ({
  page,
}) => {
  const scenes = await aBook(page, "FarDown", 14);
  await written(page, scenes[0], "The top.");

  for (let step = 0; step < 14; step++) {
    await page.keyboard.press("Shift+ArrowDown");
  }

  await expect.poll(() => chosen(page)).toEqual(scenes);
  await expect(
    flow(page).locator(`[data-place-of="${scenes[13]}"]`),
  ).toBeInViewport();
});

test("scenes outside the book are chosen the same way", async ({ page }) => {
  await aNewProject(page, "Outside");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  const second = await aLooseScene(page);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));
  await expect(scene(page, first)).toBeVisible();
  await written(page, first, "Loose.");

  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  await expect.poll(() => chosen(page)).toEqual([first, second]);
});

const spannedBySelection = (page: Page) =>
  page.evaluate(() => {
    const shown = getSelection()!;
    if (shown.rangeCount === 0) {
      return [];
    }
    const range = shown.getRangeAt(0);

    return [...document.querySelectorAll(".scene[data-scene]")]
      .filter((scene) => range.intersectsNode(scene))
      .map((scene) => scene.getAttribute("data-scene"));
  });

test("chosen scenes are shown as the browser's own selection, from the first to the last", async ({
  page,
}) => {
  const [first, second, third] = await aBook(page, "Shown", 3);
  await written(page, first, "Some words");
  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowLeft");

  await page.keyboard.press("End");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  await expect.poll(() => spannedBySelection(page)).toEqual([first, second]);
  await expect(
    flow(page),
    "the keys stay with the book while scenes are chosen",
  ).toBeFocused();

  await page.keyboard.press("Shift+ArrowDown");
  await expect
    .poll(() => spannedBySelection(page))
    .toEqual([first, second, third]);
});

const headingsChosen = (page: Page) =>
  page.evaluate(() => {
    const shown = getSelection()!;
    if (shown.rangeCount === 0) {
      return [];
    }
    const range = shown.getRangeAt(0);

    return [...document.querySelectorAll(".writing .heading")]
      .filter((heading) => range.intersectsNode(heading))
      .map((heading) => heading.textContent);
  });

test("a heading is a step of its own when choosing across sections", async ({
  page,
}) => {
  await aNewProject(page, "Headings");
  const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = made["Part One"];
  const [second] = made["Part Two"];
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await written(page, second, "In part two.");
  await page.keyboard.press("ControlOrMeta+Home");

  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => spannedBySelection(page)).toEqual([second]);
  await expect.poll(() => headingsChosen(page)).toEqual([]);

  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => headingsChosen(page)).toEqual(["Part Two"]);
  await expect.poll(() => spannedBySelection(page)).toEqual([second]);

  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => spannedBySelection(page)).toEqual([first, second]);
  await expect.poll(() => headingsChosen(page)).toEqual(["Part Two"]);
});

test("a scene already selected whole as text takes in the next one at the first press beyond it", async ({
  page,
}) => {
  const [first, second, third] = await aBook(page, "Whole", 3);
  await written(page, third, "The last scene.");

  await page.keyboard.press("Control+Shift+ArrowUp");
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "The last scene.",
  );

  await page.keyboard.press("Control+Shift+ArrowUp");

  await expect.poll(() => chosen(page)).toEqual([second, third]);
  await page.keyboard.press("Control+Shift+ArrowUp");
  await expect.poll(() => chosen(page)).toEqual([first, second, third]);
});

test("ending a choosing on a heading puts the cursor in that heading", async ({
  page,
}) => {
  await aNewProject(page, "EndOnHeading");
  const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [second] = made["Part Two"];
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await written(page, second, "Text.");
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");

  await page.keyboard.press("Escape");

  await expect(
    flow(page).getByRole("heading", { name: "Part Two" }),
  ).toBeFocused();
});

test("in an empty scene, Shift+Down chooses only that scene", async ({
  page,
}) => {
  const [first, second] = await aBook(page, "EmptyScene", 3);
  await scene(page, first).click();

  await page.keyboard.press("Shift+ArrowDown");

  await expect.poll(() => chosen(page)).toEqual([first]);
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second]);
});
