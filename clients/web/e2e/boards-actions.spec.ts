import { test, expect } from "@playwright/test";

import {
  bar,
  cardNamed,
  corkboard,
  openTheIdea,
  select,
} from "./support/board";
import {
  anOpenProject,
  captureIdea,
  catalogued,
  onTheBoard,
  openTheBoard,
} from "./support/shell";

test("an opened idea leads back to the board", async ({ page }) => {
  // given: an idea pinned to the board and opened from it
  await anOpenProject(page, "BackToBoard");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await openTheIdea(page, "The loom remembers");
  await expect(
    page.getByRole("heading", { name: "The loom remembers" }),
  ).toBeVisible();

  // when: the board is opened again
  await openTheBoard(page);

  // then: the idea is on it
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
});

test("a single click selects a card, and the bare board deselects it", async ({
  page,
}) => {
  // given: two cards on the board
  await anOpenProject(page, "Selecting");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const cards = corkboard(page).locator(".pinned");
  await expect(cards).toHaveCount(2);

  // when: the first is clicked
  await cards.first().click();

  // then: it alone is selected
  await expect(cards.first()).toHaveClass(/selected/);
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  // when: the second is clicked
  await cards.nth(1).click();

  // then: it alone is selected
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
  await expect(cards.nth(1)).toHaveClass(/selected/);

  // when: the bare board is clicked
  await corkboard(page).click({ position: { x: 12, y: 320 } });

  // then: no card is selected
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("escape lets go of a selected card", async ({ page }) => {
  // given: a selected card
  await anOpenProject(page, "Escaping");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").click();
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  // when: escape is pressed
  await page.keyboard.press("Escape");

  // then: no card is selected
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("clicking a card's title selects it rather than opening it", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "TitleClick");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  // when: its title is clicked
  await corkboard(page).locator(".name").click();

  // then: the board stays open
  await expect(page).toHaveURL(board);
  // and: the card is selected
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
});

test("selecting a card raises an action bar over it", async ({ page }) => {
  // given: a card on the board, no bar shown
  await anOpenProject(page, "Bar");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(bar(page)).toHaveCount(0);

  // when: it is selected
  await cardNamed(page, "The loom remembers").click();

  // then: a bar of its actions shows
  await expect(bar(page)).toHaveAttribute(
    "aria-label",
    "Actions for The loom remembers",
  );
  for (const deed of ["Rename", "Open", "Unpin"]) {
    await expect(
      bar(page).getByRole("button", { name: `${deed} The loom remembers` }),
    ).toBeVisible();
  }

  // when: escape is pressed
  await page.keyboard.press("Escape");

  // then: the bar goes
  await expect(bar(page)).toHaveCount(0);
});

test("the bar drops below a card that is too near the top edge", async ({
  page,
}) => {
  // given: a card near the top edge
  await anOpenProject(page, "BarFlip");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  // when: it is selected
  await select(page, "The loom remembers");

  // then: its bar drops below it
  await expect(bar(page), "40px is not enough room for a 42px bar").toHaveClass(
    /below/,
  );

  // when: the card is moved down
  await card.focus();
  await page.keyboard.press("Shift+ArrowDown");
  await expect(card).toHaveAttribute("style", /top: 80px/);

  // then: the bar is back above it
  await expect(bar(page)).not.toHaveClass(/below/);
});

test("the bar steps aside while a card is being dragged", async ({ page }) => {
  // given: a selected card
  await anOpenProject(page, "BarDrag");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  // when: it is dragged
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 80, from.y + 60, { steps: 6 });

  // then: the bar steps aside
  await expect(bar(page)).toHaveCount(0);

  // when: it is dropped
  await page.mouse.up();

  // then: the bar is back
  await expect(bar(page)).toHaveCount(1);
});

test("a card can be renamed from its bar, and the new title sticks", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "Renaming");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: it is renamed from its bar
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", {
    name: "Rename The loom remembers",
  });
  // then: the title is offered for editing
  await expect(field).toBeFocused();
  await expect(field).toHaveValue("The loom remembers");

  // when: a new title is entered
  await field.fill("The loom forgets");
  await field.press("Enter");

  // then: the card carries it
  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
  await catalogued(page, ["The loom forgets"]);

  // and: still does after a reload
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("escape abandons a rename and keeps the old title", async ({ page }) => {
  // given: a card being renamed
  await anOpenProject(page, "RenameEscape");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", {
    name: "Rename The loom remembers",
  });

  // when: a new title is typed and escape pressed
  await field.fill("Thrown away");
  await field.press("Escape");

  // then: the card keeps its old title
  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText(
    "The loom remembers",
  );

  // and: still does after a reload
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".name")).toHaveText(
    "The loom remembers",
  );
});

test("arrow keys write into a title being renamed instead of moving the card", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "RenameKeys");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  // when: it is renamed and arrow keys pressed
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowDown");

  // then: the card stays put
  await expect(
    card,
    "the card must stay put while its title is being edited",
  ).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

test("the bar's open button opens the idea", async ({ page }) => {
  // given: a selected card
  await anOpenProject(page, "BarOpen");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: open is pressed on its bar
  await page.getByRole("button", { name: "Open The loom remembers" }).click();

  // then: the idea opens
  await expect(
    page.getByRole("heading", { name: "The loom remembers" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/ideas\/the-loom-remembers-idea_/);
});

test("clicking away from a rename keeps what was typed", async ({ page }) => {
  // given: a card being renamed
  await anOpenProject(page, "RenameAway");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();

  // when: a new title is typed and the bare board clicked
  await page
    .getByRole("textbox", { name: "Rename The loom remembers" })
    .fill("The loom forgets");
  await corkboard(page).click({ position: { x: 12, y: 320 } });

  // then: the card carries the new title
  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
  await catalogued(page, ["The loom forgets"]);

  // and: still does after a reload
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("a rename survives the board changing underneath it", async ({ page }) => {
  // given: a card being renamed, half typed
  await anOpenProject(page, "RenameSturdy");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", {
    name: "Rename The loom remembers",
  });
  await field.fill("Half typed");

  // when: another idea is pinned meanwhile
  await page.evaluate(() => {
    (document.querySelector(".waiting button") as HTMLElement).click();
  });
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  // then: the typing is kept
  await expect(
    field,
    "pinning another idea must not wipe the editor",
  ).toHaveValue("Half typed");
  // and: the rename still goes through
  await field.press("Enter");
  await expect(corkboard(page).locator(".name").first()).toHaveText(
    "Half typed",
  );
});

test("a failure stays on screen until it is dismissed", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "Alarm");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  // and: a move of it failed
  await page.route("**/api/boards/*/ideas/*", (route) => route.abort());
  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("alert")).toBeVisible();
  await page.unroute("**/api/boards/*/ideas/*");

  // when: something else works afterwards
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  // then: the failure stays on screen
  await expect(
    page.getByRole("alert"),
    "something else working does not un-break what broke",
  ).toBeVisible();

  // when: it is dismissed
  await page.getByRole("button", { name: "Dismiss" }).click();

  // then: it goes
  await expect(page.getByRole("alert")).toHaveCount(0);
});
