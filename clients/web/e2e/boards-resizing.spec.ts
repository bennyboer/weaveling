import { test, expect } from "@playwright/test";

import {
  boardSaves,
  bar,
  boxOf,
  corkboard,
  dragBy,
  pullBy,
  select,
} from "./support/board";
import {
  anOpenProject,
  captureIdea,
  onTheBoard,
  openTheBoard,
} from "./support/shell";

test("a very long title scrolls inside its card instead of stretching it", async ({
  page,
}) => {
  // given: an idea with a very long title
  await anOpenProject(page, "LongTitle");
  await captureIdea(
    page,
    "Ich habe eine sehr lange Idee. Von einem Buch. Allerdings ist es noch nicht so das ich mir sicher bin ob das eine gute Idee ist.",
  );
  // when: it is pinned
  await openTheBoard(page);
  await page.getByRole("button", { name: /^Pin Ich habe/ }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveCount(1);

  // then: its card keeps its size, the title scrolling inside it
  const shape = await card.evaluate((it) => {
    const name = it.querySelector(".name")!;

    return {
      cardHeight: (it as HTMLElement).offsetHeight,
      nameScroll: name.scrollHeight,
      nameClient: name.clientHeight,
    };
  });

  expect(shape.cardHeight, "the card keeps its size").toBeLessThan(120);
  expect(
    shape.nameScroll,
    "the whole title is still there to scroll to",
  ).toBeGreaterThan(shape.nameClient);
});

test("a freshly pinned card is drawn at the size the board was told", async ({
  page,
}) => {
  // given: an idea waiting beside the board
  await anOpenProject(page, "Sized");
  await captureIdea(page, "The loom remembers");
  // when: it is pinned
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // then: its card is drawn at the size given
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 168px; height: 84px;/,
  );
});

test("dragging the right edge widens a card without moving it", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "Widen");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: its right edge is dragged out
  await pullBy(page, "right", "The loom remembers", 62, 0);

  // then: it widens in place
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 230px; height: 84px;/,
  );

  // and: stays so after a reload
  await saved();
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 230px; height: 84px;/,
  );
});

test("dragging the bottom edge makes a card taller", async ({ page }) => {
  // given: a selected card
  await anOpenProject(page, "Taller");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: its bottom edge is dragged down
  await pullBy(page, "bottom", "The loom remembers", 0, 41);

  // then: it grows taller in place
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 168px; height: 125px;/,
  );
});

test("dragging the top-left corner moves and resizes in one gesture", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "Corner");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: its top-left corner is dragged out
  await pullBy(page, "top-left", "The loom remembers", -23, -18);

  // then: it moves and grows, its far corner staying put
  await expect(
    corkboard(page).locator(".pinned"),
    "40-23 snaps to 15 and 40-18 snaps to 20, so the far corner stays put",
  ).toHaveAttribute(
    "style",
    /left: 15px; top: 20px; width: 193px; height: 104px;/,
  );

  // and: stays so after a reload
  await saved();
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 15px; top: 20px; width: 193px; height: 104px;/,
  );
});

test("a card cannot be dragged smaller than it is allowed to be", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "Smallest");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: it is dragged far smaller
  await pullBy(page, "right", "The loom remembers", -400, 0);

  // then: it stops at its smallest size
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 80px; height: 84px;/,
  );
  // and: nothing is reported failed
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a resize lands on the grid like everything else", async ({ page }) => {
  // given: a selected card
  await anOpenProject(page, "ResizeSnap");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: it is resized off the grid
  await pullBy(page, "right", "The loom remembers", 43, 0);

  // then: its size lands on the grid
  const drawn = await boxOf(page);
  const [width] = /width: (\d+)px/.exec(drawn!)!.slice(1).map(Number);
  expect(width % 5, `a card ${width}px wide is off the grid`).toBe(0);
});

test("a resized card keeps its size while it is dragged around", async ({
  page,
}) => {
  // given: a widened card
  await anOpenProject(page, "KeepsSize");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await pullBy(page, "right", "The loom remembers", 62, 0);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /width: 230px/,
  );

  // when: it is dragged
  await dragBy(page, corkboard(page).locator(".pinned"), 100, 50);

  // then: it keeps its size
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 140px; top: 90px; width: 230px; height: 84px;/,
  );
});

test("the action bar follows the bottom of a card that has grown", async ({
  page,
}) => {
  // given: a selected card with its action bar below
  await anOpenProject(page, "BarFollows");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await expect(bar(page)).toHaveClass(/below/);
  const first = await bar(page).boundingBox();

  // when: it is made taller
  await pullBy(page, "bottom", "The loom remembers", 0, 80);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /height: 165px/,
  );

  // then: the bar follows its bottom
  const after = await bar(page).boundingBox();
  expect(
    after!.y - first!.y,
    "the bar sat under an 84px card and should now sit under a 165px one",
  ).toBeCloseTo(81, 0);
});
