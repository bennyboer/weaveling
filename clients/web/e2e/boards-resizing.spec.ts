import { test, expect } from "@playwright/test";

import {
  anOpenProject,
  bar,
  boxOf,
  capture,
  corkboard,
  dragBy,
  openTheBoard,
  pullBy,
  select,
} from "./support/board";

test("a very long title scrolls inside its card instead of stretching it", async ({ page }) => {
  await anOpenProject(page, "LongTitle");
  await capture(
    page,
    "Ich habe eine sehr lange Idee. Von einem Buch. Allerdings ist es noch nicht so das ich mir sicher bin ob das eine gute Idee ist.",
  );
  await openTheBoard(page);
  await page.getByRole("button", { name: /^Pin Ich habe/ }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveCount(1);

  const shape = await card.evaluate((it) => {
    const name = it.querySelector(".name")!;

    return {
      cardHeight: (it as HTMLElement).offsetHeight,
      nameScroll: name.scrollHeight,
      nameClient: name.clientHeight,
    };
  });

  expect(shape.cardHeight, "the card keeps its size").toBeLessThan(120);
  expect(shape.nameScroll, "the whole title is still there to scroll to").toBeGreaterThan(
    shape.nameClient,
  );
});

test("a freshly pinned card is drawn at the size the board was told", async ({ page }) => {
  await anOpenProject(page, "Sized");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 168px; height: 84px;/,
  );
});

test("dragging the right edge widens a card without moving it", async ({ page }) => {
  await anOpenProject(page, "Widen");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await pullBy(page, "right", "The loom remembers", 62, 0);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 230px; height: 84px;/,
  );

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 230px; height: 84px;/,
  );
});

test("dragging the bottom edge makes a card taller", async ({ page }) => {
  await anOpenProject(page, "Taller");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await pullBy(page, "bottom", "The loom remembers", 0, 41);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 168px; height: 125px;/,
  );
});

test("dragging the top-left corner moves and resizes in one gesture", async ({ page }) => {
  await anOpenProject(page, "Corner");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await pullBy(page, "top-left", "The loom remembers", -23, -18);

  await expect(
    corkboard(page).locator(".pinned"),
    "40-23 snaps to 15 and 40-18 snaps to 20, so the far corner stays put",
  ).toHaveAttribute("style", /left: 15px; top: 20px; width: 193px; height: 104px;/);

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 15px; top: 20px; width: 193px; height: 104px;/,
  );
});

test("a card cannot be dragged smaller than it is allowed to be", async ({ page }) => {
  await anOpenProject(page, "Smallest");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await pullBy(page, "right", "The loom remembers", -400, 0);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px; width: 80px; height: 84px;/,
  );
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a resize lands on the grid like everything else", async ({ page }) => {
  await anOpenProject(page, "ResizeSnap");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await pullBy(page, "right", "The loom remembers", 43, 0);

  const drawn = await boxOf(page);
  const [width] = /width: (\d+)px/.exec(drawn!)!.slice(1).map(Number);
  expect(width % 5, `a card ${width}px wide is off the grid`).toBe(0);
});

test("a resized card keeps its size while it is dragged around", async ({ page }) => {
  await anOpenProject(page, "KeepsSize");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await pullBy(page, "right", "The loom remembers", 62, 0);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /width: 230px/);

  await dragBy(page, corkboard(page).locator(".pinned"), 100, 50);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 140px; top: 90px; width: 230px; height: 84px;/,
  );
});

test("the action bar follows the bottom of a card that has grown", async ({ page }) => {
  await anOpenProject(page, "BarFollows");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await expect(bar(page)).toHaveClass(/below/);
  const first = await bar(page).boundingBox();

  await pullBy(page, "bottom", "The loom remembers", 0, 80);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /height: 165px/);

  const after = await bar(page).boundingBox();
  expect(
    after!.y - first!.y,
    "the bar sat under an 84px card and should now sit under a 165px one",
  ).toBeCloseTo(81, 0);
});
