import { test, expect, type Page } from "@playwright/test";

import { aNewProject, openTheBoard, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

const masthead = (page: Page) => page.locator(".masthead");

const focusLine = (page: Page) =>
  page.getByRole("navigation", { name: "Where you are" });

const focusButton = (page: Page) =>
  page.getByRole("button", { name: "Focus mode" });

const scene = (page: Page, id: string) =>
  flow(page).locator(`[data-scene="${id}"] .ProseMirror`);

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
  const { title, scenes } = await writingIn(page, "Shortcut");
  await scene(page, scenes["Part One"][0]).click();

  await page.keyboard.press("Control+Shift+F");

  await expect(masthead(page)).toBeHidden();
  await expect(focusLine(page)).toHaveText(`${title}›Part One`);
  await expect(focusButton(page)).toHaveAttribute("aria-pressed", "true");
  await expect(
    scene(page, scenes["Part One"][0]),
    "the cursor stays where it was writing",
  ).toBeFocused();

  await page.keyboard.press("Control+Shift+F");

  await expect(masthead(page)).toBeVisible();
  await expect(focusLine(page)).toHaveCount(0);
  await expect(focusButton(page)).toHaveAttribute("aria-pressed", "false");
});

test("the button in the status bar goes into focus mode and out again", async ({
  page,
}) => {
  await writingIn(page, "Button");

  await focusButton(page).click();
  await expect(masthead(page)).toBeHidden();

  await focusButton(page).click();
  await expect(masthead(page)).toBeVisible();
});

test("the quiet line names the section being written in", async ({ page }) => {
  const { title, scenes } = await writingIn(page, "Following");
  await focusButton(page).click();

  await scene(page, scenes["Part Two"][0]).click();

  await expect(focusLine(page)).toHaveText(`${title}›Part Two`);
});

test("the drawers go while focused, and come back as they were", async ({
  page,
}) => {
  await writingIn(page, "Drawers");
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  const contents = page.getByRole("complementary", { name: "Contents" });
  await expect(contents).toBeVisible();

  await focusButton(page).click();

  await expect(contents).toBeHidden();
  await expect(page.locator(".drawer-tab")).toHaveCount(2);
  for (const tab of await page.locator(".drawer-tab").all()) {
    await expect(tab).toBeHidden();
  }

  await focusButton(page).click();
  await expect(contents).toBeVisible();
});

test("the status bar fades back until it is hovered", async ({ page }) => {
  await writingIn(page, "Fading");
  expect(await opacityOf(page, ".status-bar")).toBe(1);

  await focusButton(page).click();
  await page.mouse.move(400, 200);

  await expect.poll(() => opacityOf(page, ".status-bar")).toBeLessThan(0.5);
  await expect.poll(() => opacityOf(page, ".status-note")).toBeLessThan(0.5);

  await page.getByRole("contentinfo", { name: "Status" }).hover();

  await expect.poll(() => opacityOf(page, ".status-bar")).toBe(1);
  await expect.poll(() => opacityOf(page, ".status-note")).toBe(1);
});

test("while focused, a margin's menu shows only when its line is hovered", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1000, height: 800 });
  const { scenes } = await writingIn(page, "Margins");
  const menu = flow(page).locator(
    `.scene-row:has([data-scene="${scenes["Part One"][0]}"]) .menu-button`,
  );
  await page.mouse.move(5, 5);
  expect(
    Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    "on a narrow window the menus show without hovering",
  ).toBe(1);

  await focusButton(page).click();
  await page.mouse.move(5, 400);

  await expect
    .poll(async () =>
      Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    )
    .toBe(0);
  await scene(page, scenes["Part One"][0]).hover();
  await expect
    .poll(async () =>
      Number(await menu.evaluate((button) => getComputedStyle(button).opacity)),
    )
    .toBe(1);
});

test("focus mode is not kept: a reload or another view leaves it", async ({
  page,
}) => {
  await writingIn(page, "NotKept");
  await focusButton(page).click();
  await expect(masthead(page)).toBeHidden();

  await page.reload();
  await expect(flow(page)).toBeVisible();
  await expect(masthead(page)).toBeVisible();

  await focusButton(page).click();
  await expect(masthead(page)).toBeHidden();
  await page.goBack();
  await openTheBoard(page);
  await expect(masthead(page)).toBeVisible();
});
