import { test, expect, type Page } from "@playwright/test";

import {
  aLooseScene,
  aNewProject,
  captureIdea,
  openTheBoard,
  openTheOutline,
  openThePool,
} from "./support/shell";

const NARROW = { width: 700, height: 800 };

const tray = (page: Page) => page.getByRole("complementary", { name: "Tray" });

const tab = (page: Page, waiting = "Not on the board") =>
  page.getByRole("button", { name: new RegExp(`^${waiting} ·`) });

const placed = (page: Page) =>
  page.evaluate(() => {
    const seen = document.querySelector("#tray")!.getBoundingClientRect();

    return {
      left: Math.round(seen.left),
      right: Math.round(seen.right),
      width: Math.round(seen.width),
      window: innerWidth,
    };
  });

const corkboardWidth = (page: Page) =>
  page.evaluate(() =>
    Math.round(
      document.querySelector(".corkboard")!.getBoundingClientRect().width,
    ),
  );

test("both views keep the tray in the same place", async ({ page }) => {
  await aNewProject(page, "Traying");
  await expect(tray(page)).toBeVisible();

  const onTheBoard = await placed(page);

  await openTheOutline(page);
  await expect(tray(page)).toBeVisible();
  const inTheOutline = await placed(page);

  expect(onTheBoard.right).toBe(onTheBoard.window);
  expect(onTheBoard.width).toBeGreaterThan(0);
  expect(inTheOutline).toEqual(onTheBoard);
});

test("the tab tells what is still waiting, in each view's own words", async ({
  page,
}) => {
  await aNewProject(page, "Tallying");
  await openThePool(page);
  await captureIdea(page, "A first idea");
  await captureIdea(page, "A second idea");

  await openTheBoard(page);
  await expect(tab(page)).toHaveText(/^Not on the board · 2$/);

  await aLooseScene(page);
  await aLooseScene(page);
  await openTheOutline(page);
  await expect(
    tab(page, "Not in the book"),
    "a shut tray's tab is the only thing saying what is waiting, and the two views wait for different things",
  ).toHaveText(/^Not in the book · 2$/);
});

test("on a wide window the tray is open at first and takes its own room beside the board", async ({
  page,
}) => {
  await aNewProject(page, "Wide");

  await expect(tray(page)).toBeVisible();
  const seen = await placed(page);
  expect(await corkboardWidth(page)).toBe(seen.window - seen.width);

  await tray(page).getByRole("button", { name: "Close the tray" }).click();

  await expect(tray(page)).toBeHidden();
  await expect
    .poll(() => corkboardWidth(page), {
      message: "a shut tray gives the board its room back",
    })
    .toBe(seen.window);
});

test("a narrow window starts with the tray shut, and opens it over the board", async ({
  page,
}) => {
  await page.setViewportSize(NARROW);
  await aNewProject(page, "Narrow");

  await expect(tray(page)).toBeHidden();
  await tab(page).click();

  await expect(tray(page)).toBeVisible();
  expect((await placed(page)).right).toBe(NARROW.width);
  expect(await corkboardWidth(page), "the tray lies over the board").toBe(
    NARROW.width,
  );

  await page.keyboard.press("Escape");
  await expect(tray(page)).toBeHidden();
});

test("the tray is left open or shut, and as wide, for the board and the outline alike", async ({
  page,
}) => {
  await aNewProject(page, "Kept");
  await expect(tray(page)).toBeVisible();

  const handle = page.getByRole("separator", { name: "Resize the tray" });
  const at = (await handle.boundingBox())!;
  await page.mouse.move(at.x + at.width / 2, at.y + 200);
  await page.mouse.down();
  await page.mouse.move(at.x + at.width / 2 - 80, at.y + 200, { steps: 6 });
  await page.mouse.up();
  const widened = (await placed(page)).width;
  expect(widened).toBe(320 + 80);

  await openTheOutline(page);
  expect((await placed(page)).width).toBe(widened);

  await tray(page).getByRole("button", { name: "Close the tray" }).click();
  await openTheBoard(page);
  await expect(tab(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();

  await page.reload();
  await expect(tab(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();
});
