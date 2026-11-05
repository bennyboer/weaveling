import { test, expect } from "@playwright/test";

import { boardSaves, corkboard, dragBy, pinHeld } from "./support/board";
import {
  anOpenProject,
  captureIdea,
  onTheBoard,
  openTheBoard,
} from "./support/shell";

test("a card can be nudged with the arrow keys", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "Nudging");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  // when: it is nudged down with an arrow key
  await card.focus();
  await page.keyboard.press("ArrowDown");

  // then: it moves one grid cell
  await expect(card, "one arrow press moves one grid cell").toHaveAttribute(
    "style",
    /top: 45px/,
  );

  // when: it is nudged with shift held
  await page.keyboard.press("Shift+ArrowDown");

  // then: it leaps eight cells
  await expect(card, "shift leaps eight cells").toHaveAttribute(
    "style",
    /top: 85px/,
  );
});

test("a card nudged by a held key ends where the last nudge put it, though the server hears them out of turn", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "NudgingHeld");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await pinHeld(page);
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);
  // and: each move reaching the server later the earlier it was made
  let moves = 0;
  await page.route(
    (url) => /\/api\/boards\/[^/]+\/ideas\/[^/]+$/.test(url.pathname),
    async (route) => {
      if (route.request().method() !== "PATCH") {
        return route.fallback();
      }
      moves += 1;
      await new Promise((resolve) => setTimeout(resolve, Math.max(0, 400 - moves * 20)));
      await route.fallback();
    },
  );

  // when: it is nudged down twenty times in a row
  await card.focus();
  for (let nudge = 0; nudge < 20; nudge += 1) {
    await page.keyboard.press("ArrowDown");
  }

  // then: it shows where the last nudge put it
  await expect(card).toHaveAttribute("style", /top: 140px/);
  // and: fewer moves were sent than were made
  await saved();
  expect(moves, "nudges made while one is on its way are sent as one").toBeLessThan(20);
  // when: the board is reloaded
  await page.reload();
  await onTheBoard(page);
  // then: the card is still where the last nudge put it
  await expect(
    corkboard(page).locator(".pinned"),
    "moves heard out of turn would leave it where an earlier nudge put it",
  ).toHaveAttribute("style", /top: 140px/);
});

test("a nudge survives a reload", async ({ page }) => {
  // given: a card nudged on the board
  await anOpenProject(page, "NudgeLasts");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await pinHeld(page);
  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("ArrowRight");
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 45px/,
  );

  // when: the board is reloaded
  await saved();
  await page.reload();
  await onTheBoard(page);

  // then: the card is where it was nudged to
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 45px/,
  );
});

test("a card dragged anywhere on its body lands where it was dropped and stays there", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "Dragging");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: it is dragged by its body
  await dragBy(page, corkboard(page).locator(".pinned"), 120, 90);

  // then: it lands where it was dropped
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
  // and: stays there after a reload
  await saved();
  await page.reload();
  await onTheBoard(page);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
});

test("a cancelled drag puts the card back where it was", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "Cancelled");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  // when: a drag of it is begun and cancelled
  const carried = await page.evaluate(async () => {
    const card = document.querySelector(".pinned")!;
    const fire = (type: string, extra: PointerEventInit = {}) =>
      card.dispatchEvent(
        new PointerEvent(type, { bubbles: true, pointerId: 7, ...extra }),
      );
    const style = () =>
      document.querySelector(".pinned")!.getAttribute("style");

    fire("pointerdown", { clientX: 100, clientY: 100 });
    fire("pointermove", { clientX: 130, clientY: 120 });
    await new Promise((it) => setTimeout(it, 50));
    const held = style();

    fire("pointercancel");
    await new Promise((it) => setTimeout(it, 50));
    fire("pointermove", { clientX: 230, clientY: 220 });
    await new Promise((it) => setTimeout(it, 50));

    return { held, afterwards: style() };
  });

  // then: it is carried while held and put back afterwards
  expect(carried.held).toMatch(/left: 70px; top: 60px;/);
  expect(carried.afterwards).toMatch(/left: 40px; top: 40px;/);
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

test("a card lands on the grid, however sloppily it is dropped", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "Snapping");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: it is dropped off the grid
  await dragBy(page, corkboard(page).locator(".pinned"), 43, 27);

  // then: it lands on the grid
  await expect(
    corkboard(page).locator(".pinned"),
    "40+43 snaps to 85 and 40+27 snaps to 65",
  ).toHaveAttribute("style", /left: 85px; top: 65px;/);
});

test("a slip too small to be a drag leaves the card where it was", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "Slipping");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: it slips a pixel or two
  await dragBy(page, corkboard(page).locator(".pinned"), 2, 1);

  // then: it stays where it was
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );
});

test("a card stays selected all the way through a drag", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "StaysLit");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  // when: it is dragged
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 60, from.y + 45, { steps: 6 });

  // then: it is selected mid-drag
  await expect(card, "the outline must not blink off mid-drag").toHaveClass(
    /selected/,
  );

  // when: it is dropped
  await page.mouse.up();

  // then: it is still selected
  await expect(card).toHaveClass(/selected/);
});

test("a dropped card does not flash back to where it came from", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "NoFlash");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  // when: it is dragged and dropped
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 50, { steps: 6 });
  await page.mouse.up();

  // then: it is where it was dropped at once
  const rightAway = await card.getAttribute("style");
  expect(rightAway, "the drop must not wait on the server").toMatch(
    /left: 140px; top: 90px;/,
  );
  await expect(card).toHaveAttribute("style", /left: 140px; top: 90px;/);
});

test("a card can be dragged by its title", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "TitleDrag");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  // when: it is dragged by its title
  await dragBy(page, corkboard(page).locator(".name"), 60, 40);

  // then: the board stays open
  await expect(page, "dragging the title must not follow the link").toHaveURL(
    board,
  );
  // and: the card moves
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 100px; top: 80px;/,
  );
});

test("a card being dragged rides above the ones pinned after it", async ({
  page,
}) => {
  // given: two cards, one pinned after the other
  await anOpenProject(page, "OnTop");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const first = corkboard(page).locator(".pinned").first();
  const box = await first.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  // when: the first is dragged
  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 5, { steps: 6 });

  // then: it rides on top
  await expect(first).toHaveClass(/carried/);
  const onTop = await page.evaluate(() => {
    const carried = document.querySelector(".pinned.carried")!;
    const box = carried.getBoundingClientRect();
    const middle = document.elementFromPoint(
      box.left + box.width / 2,
      box.top + box.height / 2,
    );

    return carried.contains(middle);
  });
  expect(onTop, "the card under the cursor must be the one being dragged").toBe(
    true,
  );

  // when: it is dropped
  await page.mouse.up();
  // then: it is no longer carried
  await expect(first).not.toHaveClass(/carried/);
});

test("a dragged card stays in front after it is dropped", async ({ page }) => {
  // given: two cards, one pinned after the other
  await anOpenProject(page, "StaysInFront");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const cards = corkboard(page).locator(".pinned");
  await expect(cards).toHaveCount(2);
  const under = cards.first();

  // when: the first is dragged
  await dragBy(page, under, 160, 0);

  // then: it is in front
  const onTop = await page.evaluate(() => {
    const drawn = [...document.querySelectorAll(".pinned")];
    const last = drawn[drawn.length - 1];

    return last.querySelector(".name")!.textContent;
  });
  expect(
    onTop,
    "the card you just moved should be drawn last, so it sits in front",
  ).toBe("The loom remembers");

  // and: still is after a reload
  await saved();
  await page.reload();
  await onTheBoard(page);
  await expect(cards).toHaveCount(2);
  const afterwards = await page.evaluate(() => {
    const drawn = [...document.querySelectorAll(".pinned")];

    return drawn[drawn.length - 1].querySelector(".name")!.textContent;
  });
  expect(afterwards, "the stacking is the board's, not the browser's").toBe(
    "The loom remembers",
  );
});
