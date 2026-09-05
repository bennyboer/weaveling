import { test, expect } from "@playwright/test";

import {
  anOpenProject,
  bar,
  capture,
  cardNamed,
  corkboard,
  openTheBoard,
  select,
} from "./support/board";

test("a pinned piece opens for writing from the board", async ({ page }) => {
  await anOpenProject(page, "Writing");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await corkboard(page).locator(".pinned").dblclick();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
});

test("the writing view leads back to the board", async ({ page }) => {
  await anOpenProject(page, "BackToBoard");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").dblclick();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();

  await page.getByRole("link", { name: "Back to the board" }).click();

  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
});

test("a single click selects a card, and the bare board deselects it", async ({ page }) => {
  await anOpenProject(page, "Selecting");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const cards = corkboard(page).locator(".pinned");
  await expect(cards).toHaveCount(2);

  await cards.first().click();

  await expect(cards.first()).toHaveClass(/selected/);
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  await cards.nth(1).click();

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
  await expect(cards.nth(1)).toHaveClass(/selected/);

  await corkboard(page).click({ position: { x: 12, y: 320 } });

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("escape lets go of a selected card", async ({ page }) => {
  await anOpenProject(page, "Escaping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").click();
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  await page.keyboard.press("Escape");

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("clicking a card's title selects it rather than opening it", async ({ page }) => {
  await anOpenProject(page, "TitleClick");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  await corkboard(page).locator(".name").click();

  await expect(page).toHaveURL(board);
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
});

test("selecting a card raises an action bar over it", async ({ page }) => {
  await anOpenProject(page, "Bar");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(bar(page)).toHaveCount(0);

  await cardNamed(page, "The loom remembers").click();

  await expect(bar(page)).toHaveAttribute("aria-label", "Actions for The loom remembers");
  for (const deed of ["Rename", "Open", "Unpin"]) {
    await expect(bar(page).getByRole("button", { name: `${deed} The loom remembers` })).toBeVisible();
  }

  await page.keyboard.press("Escape");

  await expect(bar(page)).toHaveCount(0);
});

test("the bar drops below a card that is too near the top edge", async ({ page }) => {
  await anOpenProject(page, "BarFlip");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  await select(page, "The loom remembers");

  await expect(bar(page), "40px is not enough room for a 42px bar").toHaveClass(/below/);

  await card.focus();
  await page.keyboard.press("Shift+ArrowDown");
  await expect(card).toHaveAttribute("style", /top: 80px/);

  await expect(bar(page)).not.toHaveClass(/below/);
});

test("the bar steps aside while a card is being dragged", async ({ page }) => {
  await anOpenProject(page, "BarDrag");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 80, from.y + 60, { steps: 6 });

  await expect(bar(page)).toHaveCount(0);

  await page.mouse.up();

  await expect(bar(page)).toHaveCount(1);
});

test("a card can be renamed from its bar, and the new title sticks", async ({ page }) => {
  await anOpenProject(page, "Renaming");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });
  await expect(field).toBeFocused();
  await expect(field).toHaveValue("The loom remembers");

  await field.fill("The loom forgets");
  await field.press("Enter");

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("escape abandons a rename and keeps the old title", async ({ page }) => {
  await anOpenProject(page, "RenameEscape");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });

  await field.fill("Thrown away");
  await field.press("Escape");

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom remembers");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom remembers");
});

test("arrow keys write into a title being renamed instead of moving the card", async ({ page }) => {
  await anOpenProject(page, "RenameKeys");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowDown");

  await expect(card, "the card must stay put while its title is being edited").toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );
});

test("the bar's open button opens the piece for writing", async ({ page }) => {
  await anOpenProject(page, "BarOpen");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await page.getByRole("button", { name: "Open The loom remembers" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
});

test("clicking away from a rename keeps what was typed", async ({ page }) => {
  await anOpenProject(page, "RenameAway");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();

  await page.getByRole("textbox", { name: "Rename The loom remembers" }).fill("The loom forgets");
  await corkboard(page).click({ position: { x: 12, y: 320 } });

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("a rename survives the board changing underneath it", async ({ page }) => {
  await anOpenProject(page, "RenameSturdy");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });
  await field.fill("Half typed");

  await page.evaluate(() => {
    (document.querySelector(".waiting button") as HTMLElement).click();
  });
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  await expect(field, "pinning another piece must not wipe the editor").toHaveValue("Half typed");
  await field.press("Enter");
  await expect(corkboard(page).locator(".name").first()).toHaveText("Half typed");
});
