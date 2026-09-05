import { test, expect } from "@playwright/test";

import { cardNamed, corkboard, waiting } from "./support/board";
import {
  aNewProject,
  capture,
  openThePool,
  openTheBoard,
} from "./support/shell";

const editor = (page: import("@playwright/test").Page) =>
  corkboard(page).getByRole("textbox", { name: "What is the idea?" });

async function doubleClickBareBoard(page: import("@playwright/test").Page) {
  const board = (await corkboard(page).boundingBox())!;

  await page.mouse.dblclick(
    board.x + board.width / 2,
    board.y + board.height / 2,
  );
}

test("double-clicking the bare board captures a piece and pins it", async ({
  page,
}) => {
  await aNewProject(page, "Capturing");

  await doubleClickBareBoard(page);
  await expect(editor(page)).toBeFocused();
  await page.keyboard.type("The loom remembers");
  await page.keyboard.press("Enter");

  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  await expect(editor(page)).toHaveCount(0);
  await expect(waiting(page).getByRole("button")).toHaveCount(0);

  await openThePool(page);

  await expect(
    page.getByRole("list", { name: "Pieces" }).getByText("The loom remembers"),
  ).toBeVisible();
});

test("a cancelled capture records nothing at all", async ({ page }) => {
  await aNewProject(page, "Cancelling");

  await doubleClickBareBoard(page);
  await page.keyboard.type("Never mind");
  await page.keyboard.press("Escape");

  await expect(editor(page)).toHaveCount(0);
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  await openThePool(page);

  await expect(page.getByText("Never mind")).toHaveCount(0);
  await expect(
    page.getByText("No pieces yet. Shoot an idea in and see where it goes."),
  ).toBeVisible();
});

test("a capture left empty records nothing", async ({ page }) => {
  await aNewProject(page, "Blank");

  await doubleClickBareBoard(page);
  await page.keyboard.type("   ");
  await page.keyboard.press("Enter");

  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  await openThePool(page);

  await expect(
    page.getByText("No pieces yet. Shoot an idea in and see where it goes."),
  ).toBeVisible();
});

test("the fresh card lands where it was asked for", async ({ page }) => {
  await aNewProject(page, "Placed");
  const board = (await corkboard(page).boundingBox())!;
  const asked = { x: board.x + 420, y: board.y + 260 };

  await page.mouse.dblclick(asked.x, asked.y);
  await page.keyboard.type("Right here");
  await page.keyboard.press("Enter");

  const landed = (await cardNamed(page, "Right here").boundingBox())!;
  const middle = {
    x: landed.x + landed.width / 2,
    y: landed.y + landed.height / 2,
  };

  expect(Math.abs(middle.x - asked.x)).toBeLessThanOrEqual(5);
  expect(Math.abs(middle.y - asked.y)).toBeLessThanOrEqual(5);
});

test("double-clicking a card edits its title instead of opening it", async ({
  page,
}) => {
  await aNewProject(page, "Retitling");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await cardNamed(page, "The loom remembers").dblclick();

  const editing = corkboard(page).getByRole("textbox", {
    name: "Rename The loom remembers",
  });
  await expect(editing).toBeFocused();
  await page.keyboard.type("A thread comes loose");
  await page.keyboard.press("Enter");

  await expect(cardNamed(page, "A thread comes loose")).toBeVisible();
  await expect(page).toHaveURL(
    /\/projects\/project-[a-z0-9-]+-project_[0-9A-Za-z]{22}$/,
  );
});

test("Enter on a focused card edits its title too", async ({ page }) => {
  await aNewProject(page, "Entering");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("Enter");

  await expect(
    corkboard(page).getByRole("textbox", { name: "Rename The loom remembers" }),
  ).toBeFocused();
});

test("focusing a card from the keyboard reveals its actions", async ({
  page,
}) => {
  await aNewProject(page, "Focusing");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await corkboard(page).locator(".pinned").focus();

  await expect(
    corkboard(page).getByRole("toolbar", {
      name: "Actions for The loom remembers",
    }),
  ).toBeVisible();
});
