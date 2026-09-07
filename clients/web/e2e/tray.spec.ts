import { test, expect, type Page } from "@playwright/test";

import {
  aNewProject,
  capture,
  openTheBoard,
  openTheOutline,
  openThePool,
} from "./support/shell";

const NARROW = { width: 700, height: 800 };

const tray = (page: Page) => page.locator(".tray");

const toggle = (page: Page) =>
  page.getByRole("button", { name: /^Pieces ·/ });

const placed = (page: Page) =>
  page.evaluate(() => {
    const seen = document.querySelector(".tray")!.getBoundingClientRect();

    return {
      left: Math.round(seen.left),
      right: Math.round(seen.right),
      width: Math.round(seen.width),
      window: innerWidth,
    };
  });

const awayFromView = (page: Page) =>
  expect
    .poll(async () => (await placed(page)).left)
    .toBeGreaterThanOrEqual(NARROW.width);

test("both views keep the tray in the same place", async ({ page }) => {
  await aNewProject(page, "Traying");

  const onTheBoard = await placed(page);

  await openTheOutline(page);
  const inTheOutline = await placed(page);

  expect(onTheBoard.right).toBe(onTheBoard.window);
  expect(onTheBoard.width).toBeGreaterThan(0);
  expect(inTheOutline).toEqual(onTheBoard);
});

test("the tally counts what is still waiting", async ({ page }) => {
  await aNewProject(page, "Tallying");
  await openThePool(page);
  await capture(page, "A first idea");
  await capture(page, "A second idea");

  await openTheBoard(page);
  await expect(tray(page).getByText("Not on the board · 2")).toBeVisible();

  await openTheOutline(page);
  await expect(tray(page).getByText("Not in the book · 2")).toBeVisible();
});

test("a narrow window keeps the tray in a drawer", async ({ page }) => {
  await aNewProject(page, "Drawering");
  await page.setViewportSize(NARROW);

  await expect(toggle(page)).toBeVisible();
  await awayFromView(page);

  await toggle(page).click();
  await expect
    .poll(async () => (await placed(page)).right)
    .toBe(NARROW.width);
  expect((await placed(page)).left).toBeLessThan(NARROW.width);

  await tray(page).getByRole("button", { name: "Hide the pieces" }).click();
  await awayFromView(page);
});

test("the drawer becomes the rail again when the window widens", async ({
  page,
}) => {
  await aNewProject(page, "Widening");
  await page.setViewportSize(NARROW);
  await toggle(page).click();

  await page.setViewportSize({ width: 1280, height: 800 });

  await expect(toggle(page)).toBeHidden();
  await expect
    .poll(async () => (await placed(page)).right)
    .toBe(1280);
  expect((await placed(page)).left).toBeLessThan(1280);
});
