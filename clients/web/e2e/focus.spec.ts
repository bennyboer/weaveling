import { test, expect, type Page } from "@playwright/test";

import { aNewProject, openTheBoard, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";
import { flow, scene } from "./support/writing";

const masthead = (page: Page) => page.locator(".masthead");

const focusLine = (page: Page) =>
  page.getByRole("navigation", { name: "Where you are" });

const focusButton = (page: Page) =>
  page.getByRole("button", { name: "Focus mode" });

const opacityOf = (page: Page, selector: string) =>
  page.evaluate(
    (selector) =>
      Number(getComputedStyle(document.querySelector(selector)!).opacity),
    selector,
  );

async function writingIn(page: Page, named: string) {
  const title = await aNewProject(page, named);
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();

  return { title, scenes };
}

test("Ctrl+Shift+F goes into focus mode and out again", async ({ page }) => {
  // given: an author writing in a scene
  const { title, scenes } = await writingIn(page, "Shortcut");
  await scene(page, scenes["Part One"][0]).click();

  // when: the shortcut is pressed
  await page.keyboard.press("Control+Shift+F");

  // then: focus mode is on, a quiet line naming where they are
  await expect(masthead(page)).toBeHidden();
  await expect(focusLine(page)).toHaveText(`${title}›Part One`);
  await expect(focusButton(page)).toHaveAttribute("aria-pressed", "true");
  // and: the cursor stays where it was
  await expect(
    scene(page, scenes["Part One"][0]),
    "the cursor stays where it was writing",
  ).toBeFocused();

  // when: the shortcut is pressed again
  await page.keyboard.press("Control+Shift+F");

  // then: focus mode is off
  await expect(masthead(page)).toBeVisible();
  await expect(focusLine(page)).toHaveCount(0);
  await expect(focusButton(page)).toHaveAttribute("aria-pressed", "false");
});

test("the button in the status bar goes into focus mode and out again", async ({
  page,
}) => {
  // given: a book open in the writing view
  await writingIn(page, "Button");

  // when: the focus button is pressed
  await focusButton(page).click();
  // then: focus mode is on
  await expect(masthead(page)).toBeHidden();

  // when: it is pressed again
  await focusButton(page).click();
  // then: focus mode is off
  await expect(masthead(page)).toBeVisible();
});

test("the quiet line names the section being written in", async ({ page }) => {
  // given: a book in focus mode
  const { title, scenes } = await writingIn(page, "Following");
  await focusButton(page).click();

  // when: the author writes in the second section
  await scene(page, scenes["Part Two"][0]).click();

  // then: the quiet line names it
  await expect(focusLine(page)).toHaveText(`${title}›Part Two`);
});

test("the drawers go while focused, and come back as they were", async ({
  page,
}) => {
  // given: a book with its contents open
  await writingIn(page, "Drawers");
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  const contents = page.getByRole("complementary", { name: "Contents" });
  await expect(contents).toBeVisible();

  // when: focus mode is turned on
  await focusButton(page).click();

  // then: the drawers and their tabs go
  await expect(contents).toBeHidden();
  await expect(page.locator(".drawer-tab")).toHaveCount(2);
  for (const tab of await page.locator(".drawer-tab").all()) {
    await expect(tab).toBeHidden();
  }

  // when: it is turned off
  await focusButton(page).click();
  // then: the contents are open again
  await expect(contents).toBeVisible();
});

test("the status bar fades back until it is hovered", async ({ page }) => {
  // given: a book open in the writing view, its status bar plain
  await writingIn(page, "Fading");
  expect(await opacityOf(page, ".status-bar")).toBe(1);

  // when: focus mode is turned on
  await focusButton(page).click();
  await page.mouse.move(400, 200);

  // then: the status bar fades back
  await expect.poll(() => opacityOf(page, ".status-bar")).toBeLessThan(0.5);
  await expect.poll(() => opacityOf(page, ".status-note")).toBeLessThan(0.5);

  // when: it is hovered
  await page.getByRole("contentinfo", { name: "Status" }).hover();

  // then: it shows plainly
  await expect.poll(() => opacityOf(page, ".status-bar")).toBe(1);
  await expect.poll(() => opacityOf(page, ".status-note")).toBe(1);
});

test("while focused, a margin's menu shows only when its line is hovered", async ({
  page,
}) => {
  // given: a narrow window, the margin menus showing
  await page.setViewportSize({ width: 1000, height: 800 });
  const { scenes } = await writingIn(page, "Margins");
  const menu = flow(page).locator(
    `.scene-row:has([data-scene="${scenes["Part One"][0]}"]) .menu-button[aria-label="Scene actions"]`,
  );
  await page.mouse.move(5, 5);
  expect(
    Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    "on a narrow window the menus show without hovering",
  ).toBe(1);

  // when: focus mode is turned on
  await focusButton(page).click();
  await page.mouse.move(5, 400);

  // then: a margin's menu is hidden
  await expect
    .poll(async () =>
      Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    )
    .toBe(0);
  // when: its line is hovered
  await scene(page, scenes["Part One"][0]).hover();
  // then: the menu shows
  await expect
    .poll(async () =>
      Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    )
    .toBe(1);
});

test("focus mode is not kept: a reload or another view leaves it", async ({
  page,
}) => {
  // given: a book in focus mode
  await writingIn(page, "NotKept");
  await focusButton(page).click();
  await expect(masthead(page)).toBeHidden();

  // when: the page is reloaded
  await page.reload();
  // then: focus mode is off
  await expect(flow(page)).toBeVisible();
  await expect(masthead(page)).toBeVisible();

  // when: focus mode is turned on and another view opened
  await focusButton(page).click();
  await expect(masthead(page)).toBeHidden();
  await page.goBack();
  await openTheBoard(page);
  // then: focus mode is off
  await expect(masthead(page)).toBeVisible();
});

test("focused, the prose is set larger", async ({ page }) => {
  // given: a book open in the writing view
  const { scenes } = await writingIn(page, "Larger");
  const size = () =>
    scene(page, scenes["Part One"][0]).evaluate((prose) =>
      parseFloat(getComputedStyle(prose).fontSize),
    );
  const before = await size();

  // when: focus mode is turned on
  await focusButton(page).click();

  // then: the prose is larger
  expect(await size()).toBeGreaterThan(before);
  // when: it is turned off
  await focusButton(page).click();
  // then: the prose is as it was
  expect(await size()).toBe(before);
});

const shown = (page: Page) =>
  flow(page).evaluate((book) =>
    [...book.querySelectorAll(".ProseMirror > p, .heading")].map((part) =>
      Number(getComputedStyle(part).opacity),
    ),
  );

async function threeParagraphs(page: Page, id: string) {
  await scene(page, id).click();
  await page.keyboard.type("First paragraph.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second paragraph.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Third paragraph.");
}

test("focused, all but the paragraph being written fades back, and the brightness follows the cursor", async ({
  page,
}) => {
  // given: a scene of three paragraphs in focus mode
  const { scenes } = await writingIn(page, "Fading");
  await threeParagraphs(page, scenes["Part One"][0]);
  await focusButton(page).click();
  // when: the last paragraph is written in
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.press("ControlOrMeta+End");

  // then: all else fades back
  await expect
    .poll(() => shown(page), {
      message:
        "headings: Part One, Part Two; paragraphs: three, then the empty scene",
    })
    .toEqual([0.3, 0.3, 0.3, 1, 0.3, 0.3]);

  // when: the cursor moves up a paragraph
  await page.keyboard.press("ArrowUp");

  // then: the brightness follows it
  await expect.poll(() => shown(page)).toEqual([0.3, 0.3, 1, 0.3, 0.3, 0.3]);
});

test("nothing fades while no paragraph is being written", async ({ page }) => {
  // given: a scene of three paragraphs in focus mode
  const { scenes } = await writingIn(page, "NotWriting");
  await threeParagraphs(page, scenes["Part One"][0]);
  await focusButton(page).click();

  // when: a heading is clicked
  await flow(page).getByRole("heading", { name: "Part Two" }).click();

  // then: nothing fades
  await expect.poll(() => shown(page)).toEqual([1, 1, 1, 1, 1, 1]);
});

test("out of focus mode nothing fades", async ({ page }) => {
  // given: a scene of three paragraphs written out of focus mode
  const { scenes } = await writingIn(page, "Unfocused");
  await threeParagraphs(page, scenes["Part One"][0]);

  // then: nothing fades
  await expect.poll(() => shown(page)).toEqual([1, 1, 1, 1, 1, 1]);
});

test("moving into the next scene brightens its paragraph and fades the one left", async ({
  page,
}) => {
  // given: a scene of three paragraphs in focus mode
  const { scenes } = await writingIn(page, "NextScene");
  await threeParagraphs(page, scenes["Part One"][0]);
  await focusButton(page).click();

  // when: the next scene is written in
  await scene(page, scenes["Part Two"][0]).click();

  // then: its paragraph is bright and the one left fades
  await expect.poll(() => shown(page)).toEqual([0.3, 0.3, 0.3, 0.3, 0.3, 1]);
});
