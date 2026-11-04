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
  // given: a project open on its board
  await aNewProject(page, "Traying");
  await expect(tray(page)).toBeVisible();

  // when: the tray is measured on the board and in the outline
  const onTheBoard = await placed(page);

  await openTheOutline(page);
  await expect(tray(page)).toBeVisible();
  const inTheOutline = await placed(page);

  // then: it runs along the right edge in both alike
  expect(onTheBoard.right).toBe(onTheBoard.window);
  expect(onTheBoard.width).toBeGreaterThan(0);
  expect(inTheOutline).toEqual(onTheBoard);
});

test("the tab tells what is still waiting, in each view's own words", async ({
  page,
}) => {
  // given: a project with two ideas off the board
  await aNewProject(page, "Tallying");
  await openThePool(page);
  await captureIdea(page, "A first idea");
  await captureIdea(page, "A second idea");

  // when: the board is opened
  await openTheBoard(page);
  // then: the tab tells the ideas not on the board
  await expect(tab(page)).toHaveText(/^Not on the board · 2$/);

  // given: two scenes outside the book
  await aLooseScene(page);
  await aLooseScene(page);
  // when: the outline is opened
  await openTheOutline(page);
  // then: the tab tells the scenes not in the book
  await expect(
    tab(page, "Not in the book"),
    "a shut tray's tab is the only thing saying what is waiting, and the two views wait for different things",
  ).toHaveText(/^Not in the book · 2$/);
});

test("on a wide window the tray is open at first and takes its own room beside the board", async ({
  page,
}) => {
  // given: a project open on a wide window
  await aNewProject(page, "Wide");

  // then: the tray is open beside the board
  await expect(tray(page)).toBeVisible();
  const seen = await placed(page);
  expect(await corkboardWidth(page)).toBe(seen.window - seen.width);

  // when: the tray is closed
  await tray(page).getByRole("button", { name: "Close the tray" }).click();

  // then: it is hidden
  await expect(tray(page)).toBeHidden();
  // and: the board takes its room back
  await expect
    .poll(() => corkboardWidth(page), {
      message: "a shut tray gives the board its room back",
    })
    .toBe(seen.window);
});

test("a narrow window starts with the tray shut, and opens it over the board", async ({
  page,
}) => {
  // given: a project open on a narrow window
  await page.setViewportSize(NARROW);
  await aNewProject(page, "Narrow");

  // then: the tray is shut
  await expect(tray(page)).toBeHidden();
  // when: the tab is clicked
  await tab(page).click();

  // then: the tray opens over the board
  await expect(tray(page)).toBeVisible();
  expect((await placed(page)).right).toBe(NARROW.width);
  expect(await corkboardWidth(page), "the tray lies over the board").toBe(
    NARROW.width,
  );

  // when: Escape is pressed
  await page.keyboard.press("Escape");
  // then: the tray is shut
  await expect(tray(page)).toBeHidden();
});

test("the tray is left open or shut, and as wide, for the board and the outline alike", async ({
  page,
}) => {
  // given: a project open on its board
  await aNewProject(page, "Kept");
  await expect(tray(page)).toBeVisible();

  // when: the tray is widened
  const handle = page.getByRole("separator", { name: "Resize the tray" });
  const at = (await handle.boundingBox())!;
  await page.mouse.move(at.x + at.width / 2, at.y + 200);
  await page.mouse.down();
  await page.mouse.move(at.x + at.width / 2 - 80, at.y + 200, { steps: 6 });
  await page.mouse.up();
  // then: it is wider
  const widened = (await placed(page)).width;
  expect(widened).toBe(320 + 80);

  // when: the outline is opened
  await openTheOutline(page);
  // then: the tray is as wide there
  expect((await placed(page)).width).toBe(widened);

  // when: the tray is closed and the board opened
  await tray(page).getByRole("button", { name: "Close the tray" }).click();
  await openTheBoard(page);
  // then: the tray is shut there too
  await expect(tab(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();

  // when: the page is reloaded
  await page.reload();
  // then: the tray stays shut
  await expect(tab(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();
});
