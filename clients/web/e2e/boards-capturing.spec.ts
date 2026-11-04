import { test, expect } from "@playwright/test";

import { cardNamed, corkboard, waitingIdeas } from "./support/board";
import {
  aNewProject,
  captureIdea,
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

test("double-clicking the bare board captures an idea and pins it", async ({
  page,
}) => {
  // given: a new project's board
  await aNewProject(page, "Capturing");

  // when: the bare board is double-clicked and an idea entered
  await doubleClickBareBoard(page);
  await expect(editor(page)).toBeFocused();
  await page.keyboard.type("The loom remembers");
  await page.keyboard.press("Enter");

  // then: the idea is pinned, with nothing left waiting
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  await expect(editor(page)).toHaveCount(0);
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(0);

  // when: the pool is opened
  await openThePool(page);

  // then: the idea is in it
  await expect(
    page.getByRole("list", { name: "Ideas" }).getByText("The loom remembers"),
  ).toBeVisible();
});

test("a cancelled capture records nothing at all", async ({ page }) => {
  // given: a new project's board
  await aNewProject(page, "Cancelling");

  // when: a capture is begun and escape pressed
  await doubleClickBareBoard(page);
  await page.keyboard.type("Never mind");
  await page.keyboard.press("Escape");

  // then: nothing is pinned
  await expect(editor(page)).toHaveCount(0);
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  // and: the pool holds no idea
  await openThePool(page);

  await expect(page.getByText("Never mind")).toHaveCount(0);
  await expect(
    page.getByText("No ideas yet. Shoot an idea in and see where it goes."),
  ).toBeVisible();
});

test("a capture left empty records nothing", async ({ page }) => {
  // given: a new project's board
  await aNewProject(page, "Blank");

  // when: a capture is entered blank
  await doubleClickBareBoard(page);
  await page.keyboard.type("   ");
  await page.keyboard.press("Enter");

  // then: nothing is pinned
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  // and: the pool holds no idea
  await openThePool(page);

  await expect(
    page.getByText("No ideas yet. Shoot an idea in and see where it goes."),
  ).toBeVisible();
});

test("the fresh card lands where it was asked for", async ({ page }) => {
  // given: a new project's board
  await aNewProject(page, "Placed");
  const board = (await corkboard(page).boundingBox())!;
  const asked = { x: board.x + 420, y: board.y + 260 };

  // when: an idea is captured at a spot
  await page.mouse.dblclick(asked.x, asked.y);
  await page.keyboard.type("Right here");
  await page.keyboard.press("Enter");

  // then: its card is centred on that spot
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
  // given: a card on the board
  await aNewProject(page, "Retitling");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: it is double-clicked
  await cardNamed(page, "The loom remembers").dblclick();

  // then: its title is offered for editing
  const editing = corkboard(page).getByRole("textbox", {
    name: "Rename The loom remembers",
  });
  await expect(editing).toBeFocused();
  // when: a new title is entered
  await page.keyboard.type("A thread comes loose");
  await page.keyboard.press("Enter");

  // then: the card carries it
  await expect(cardNamed(page, "A thread comes loose")).toBeVisible();
  // and: the board stays open
  await expect(page).toHaveURL(
    /\/projects\/project-[a-z0-9-]+-project_[0-9A-Za-z]{22}\/board$/,
  );
});

test("Enter on a focused card edits its title too", async ({ page }) => {
  // given: a card on the board
  await aNewProject(page, "Entering");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: enter is pressed on it
  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("Enter");

  // then: its title is offered for editing
  await expect(
    corkboard(page).getByRole("textbox", { name: "Rename The loom remembers" }),
  ).toBeFocused();
});

test("focusing a card from the keyboard reveals its actions", async ({
  page,
}) => {
  // given: a card on the board
  await aNewProject(page, "Focusing");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: it is focused from the keyboard
  await corkboard(page).locator(".pinned").focus();

  // then: its actions show
  await expect(
    corkboard(page).getByRole("toolbar", {
      name: "Actions for The loom remembers",
    }),
  ).toBeVisible();
});
