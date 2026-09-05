import { test, expect } from "@playwright/test";

import { anOpenProject, capture, corkboard, dragBy, openTheBoard } from "./support/board";

test("a card can be nudged with the arrow keys", async ({ page }) => {
  await anOpenProject(page, "Nudging");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  await card.focus();
  await page.keyboard.press("ArrowDown");

  await expect(card, "one arrow press moves one grid cell").toHaveAttribute("style", /top: 45px/);

  await page.keyboard.press("Shift+ArrowDown");

  await expect(card, "shift leaps eight cells").toHaveAttribute("style", /top: 85px/);
});

test("a nudge survives a reload", async ({ page }) => {
  await anOpenProject(page, "NudgeLasts");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("ArrowRight");
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /left: 45px/);

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /left: 45px/);
});

test("a card dragged anywhere on its body lands where it was dropped and stays there", async ({
  page,
}) => {
  await anOpenProject(page, "Dragging");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 120, 90);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
});

test("a cancelled drag puts the card back where it was", async ({ page }) => {
  await anOpenProject(page, "Cancelled");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  const carried = await page.evaluate(async () => {
    const card = document.querySelector(".pinned")!;
    const fire = (type: string, extra: PointerEventInit = {}) =>
      card.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerId: 7, ...extra }));
    const style = () => document.querySelector(".pinned")!.getAttribute("style");

    fire("pointerdown");
    fire("pointermove", { movementX: 30, movementY: 20 });
    await new Promise((it) => setTimeout(it, 50));
    const held = style();

    fire("pointercancel");
    await new Promise((it) => setTimeout(it, 50));
    fire("pointermove", { movementX: 100, movementY: 100 });
    await new Promise((it) => setTimeout(it, 50));

    return { held, afterwards: style() };
  });

  expect(carried.held).toMatch(/left: 70px; top: 60px;/);
  expect(carried.afterwards).toMatch(/left: 40px; top: 40px;/);
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

test("a card lands on the grid, however sloppily it is dropped", async ({ page }) => {
  await anOpenProject(page, "Snapping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 43, 27);

  await expect(
    corkboard(page).locator(".pinned"),
    "40+43 snaps to 85 and 40+27 snaps to 65",
  ).toHaveAttribute("style", /left: 85px; top: 65px;/);
});

test("a slip too small to be a drag leaves the card where it was", async ({ page }) => {
  await anOpenProject(page, "Slipping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 2, 1);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );
});

test("a card stays selected all the way through a drag", async ({ page }) => {
  await anOpenProject(page, "StaysLit");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 60, from.y + 45, { steps: 6 });

  await expect(card, "the outline must not blink off mid-drag").toHaveClass(/selected/);

  await page.mouse.up();

  await expect(card).toHaveClass(/selected/);
});

test("a dropped card does not flash back to where it came from", async ({ page }) => {
  await anOpenProject(page, "NoFlash");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 50, { steps: 6 });
  await page.mouse.up();

  const rightAway = await card.getAttribute("style");
  expect(rightAway, "the drop must not wait on the server").toMatch(/left: 140px; top: 90px;/);
  await expect(card).toHaveAttribute("style", /left: 140px; top: 90px;/);
});

test("a card can be dragged by its title", async ({ page }) => {
  await anOpenProject(page, "TitleDrag");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  await dragBy(page, corkboard(page).locator(".name"), 60, 40);

  await expect(page, "dragging the title must not follow the link").toHaveURL(board);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 100px; top: 80px;/,
  );
});

test("a card being dragged rides above the ones pinned after it", async ({ page }) => {
  await anOpenProject(page, "OnTop");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const first = corkboard(page).locator(".pinned").first();
  const box = await first.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 5, { steps: 6 });

  await expect(first).toHaveClass(/carried/);
  const onTop = await page.evaluate(() => {
    const carried = document.querySelector(".pinned.carried")!;
    const box = carried.getBoundingClientRect();
    const middle = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);

    return carried.contains(middle);
  });
  expect(onTop, "the card under the cursor must be the one being dragged").toBe(true);

  await page.mouse.up();
  await expect(first).not.toHaveClass(/carried/);
});
