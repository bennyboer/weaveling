import { test, expect } from "@playwright/test";

import {
  boardOnServer,
  boardSaves,
  bar,
  cardNamed,
  corkboard,
  dragBy,
  panBy,
  pinHeld,
  seenAt,
  select,
  surface,
  zooming,
} from "./support/board";
import {
  anOpenProject,
  captureIdea,
  onTheBoard,
  openTheBoard,
} from "./support/shell";

const spotOf = (page: Page, named: string) =>
  cardNamed(page, named).evaluate((it) => it.getAttribute("style"));

type Page = Parameters<Parameters<typeof test>[1]>[0]["page"];

test("dragging the bare board carries the cards with it", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "Panning");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await pinHeld(page);
  const before = await seenAt(page, "The loom remembers");

  // when: the bare board is dragged
  await panBy(page, -150, -70);

  // then: the card is carried with it
  const after = await seenAt(page, "The loom remembers");
  expect(after.x).toBe(before.x - 150);
  expect(after.y).toBe(before.y - 70);
  // and: stays where it is pinned
  await expect(
    cardNamed(page, "The loom remembers"),
    "panning moves the window, not the idea",
  ).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

test("an idea pinned beyond the edge can be panned to", async ({ page }) => {
  // given: a card pinned far below the visible board
  await anOpenProject(page, "FarAway");
  await captureIdea(page, "Far away");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin Far away" }).click();
  await pinHeld(page);

  await page.evaluate(
    async (board) => {
      await fetch(`/api/boards/${board.id}/ideas/${board.ideas[0].idea}`, {
        method: "PATCH",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ spot: { x: 40, y: 900 } }),
      });
    },
    await boardOnServer(page),
  );
  await page.reload();
  await onTheBoard(page);

  const away = await seenAt(page, "Far away");
  expect(away.y, "the idea starts far below the visible board").toBeGreaterThan(
    500,
  );

  // when: the board is panned towards it
  await panBy(page, 0, -400);
  await panBy(page, 0, -400);

  // then: it comes into view
  const reached = await seenAt(page, "Far away");
  expect(reached.y, "panning should bring it into view").toBeLessThan(300);
});

test("a plain wheel leaves the board alone and scrolls the page", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "Wheeling");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const before = await seenAt(page, "The loom remembers");
  const middle = await corkboard(page).boundingBox();

  // when: the wheel is turned over the board
  await page.mouse.move(
    middle!.x + middle!.width / 2,
    middle!.y + middle!.height / 2,
  );
  await page.mouse.wheel(0, 200);
  await page.waitForTimeout(200);

  // then: the board stays where it was
  expect(
    await seenAt(page, "The loom remembers"),
    "the board must not steal the page's scroll",
  ).toEqual(before);
  await expect(surface(page)).toHaveAttribute("style", /translate\(0px, 0px\)/);
});

test("holding ctrl turns the wheel into a zoom", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "Zooming");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const before = await seenAt(page, "The loom remembers");
  const middle = await corkboard(page).boundingBox();

  // when: the wheel is turned with ctrl held
  await page.keyboard.down("Control");
  await page.mouse.move(
    middle!.x + middle!.width / 2,
    middle!.y + middle!.height / 2,
  );
  await page.mouse.wheel(0, -200);
  await page.keyboard.up("Control");

  // then: the card is drawn larger
  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).width)
    .toBeGreaterThan(before.width);
  // and: keeps its size
  await expect(
    cardNamed(page, "The loom remembers"),
    "zooming draws the card larger without resizing it",
  ).toHaveAttribute("style", /width: 168px; height: 84px;/);
});

test("the zoom controls take the board in and out and back to where it started", async ({
  page,
}) => {
  // given: a card on a board at full size
  await anOpenProject(page, "ZoomControls");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("100%");
  const before = await seenAt(page, "The loom remembers");

  // when: it is zoomed in
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  // then: the card is drawn larger
  await expect(reading).toHaveText("110%");
  expect((await seenAt(page, "The loom remembers")).width).toBeGreaterThan(
    before.width,
  );

  // when: it is zoomed out twice
  await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  await zooming(page).getByRole("button", { name: "Zoom out" }).click();

  // then: the zoom reads below full size
  await expect(reading).toHaveText("91%");

  // when: the zoom is reset
  await reading.click();

  // then: the board is as it started
  await expect(reading).toHaveText("100%");
  expect(await seenAt(page, "The loom remembers")).toEqual(before);
});

test("resetting the zoom keeps the board where it was panned to", async ({
  page,
}) => {
  // given: a panned board, zoomed in
  await anOpenProject(page, "ResetKeepsPan");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await panBy(page, -180, -90);
  await expect(surface(page)).toHaveAttribute(
    "style",
    /translate\(-180px, -90px\)/,
  );
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("121%");

  // when: the zoom is reset
  await reading.click();

  // then: the board is at full size
  await expect(reading).toHaveText("100%");
  // and: still panned
  const home = await surface(page).getAttribute("style");
  expect(home, "resetting the zoom is not the same as going home").not.toMatch(
    /translate\(0px, 0px\)/,
  );
});

test("the zoom buttons hold the middle of the board still", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "ZoomFromMiddle");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = await corkboard(page).boundingBox();
  const middle = { x: board!.width / 2, y: board!.height / 2 };

  const underTheMiddle = async () => {
    const drawn = (await surface(page).getAttribute("style")) ?? "";
    const [x, y] = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/
      .exec(drawn)!
      .slice(1)
      .map(Number);
    const [zoom] = /scale\(([\d.]+)\)/.exec(drawn)!.slice(1).map(Number);

    return {
      x: Math.round((middle.x - x) / zoom),
      y: Math.round((middle.y - y) / zoom),
    };
  };

  // when: the board is zoomed in
  const before = await underTheMiddle();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  // then: the same spot stays under the middle
  const after = await underTheMiddle();
  expect(
    after.x,
    "zooming in should not slide the board towards a corner",
  ).toBeCloseTo(before.x, -1);
  expect(after.y).toBeCloseTo(before.y, -1);
});

test("a card dragged on a zoomed board still lands under the pointer", async ({
  page,
}) => {
  // given: a card on a board zoomed out
  await anOpenProject(page, "ZoomedDrag");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  for (const _ of [1, 2, 3, 4, 5, 6, 7]) {
    await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  }
  await expect(
    zooming(page).getByRole("button", { name: "Reset the zoom" }),
  ).toHaveText("51%");
  const before = await seenAt(page, "The loom remembers");

  // when: the card is dragged
  await dragBy(page, cardNamed(page, "The loom remembers"), 100, 50);

  // then: it follows the pointer
  const after = await seenAt(page, "The loom remembers");
  expect(
    after.x - before.x,
    "the card should follow the pointer, not half of it",
  ).toBeCloseTo(100, -1);
  expect(after.y - before.y).toBeCloseTo(50, -1);
});

test("the action bar keeps its size however far the board is zoomed out", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "BarSize");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const before = await bar(page).boundingBox();

  // when: the board is zoomed out
  for (const _ of [1, 2, 3, 4, 5, 6, 7]) {
    await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  }

  // then: the action bar keeps its size
  const after = await bar(page).boundingBox();
  expect(
    Math.round(after!.height),
    "chrome is read at arm's length, so it does not shrink with the board",
  ).toBe(Math.round(before!.height));
  expect(Math.round(after!.width)).toBe(Math.round(before!.width));
});

test("an idea is pinned where the author is looking, not at the board's origin", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "PinInView");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  // when: the board is panned and another idea pinned
  await panBy(page, -300, -200);
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  // then: the new card lands in view
  const landed = await seenAt(page, "She never returned");
  expect(
    landed.x,
    "a fresh pin must not land off the visible board",
  ).toBeGreaterThan(0);
  expect(landed.y).toBeGreaterThan(0);
  expect(await spotOf(page, "She never returned")).toMatch(
    /left: 340px; top: 240px;/,
  );
});

test("the board itself does not scroll any more", async ({ page }) => {
  // given: a card on the board
  await anOpenProject(page, "NoScroll");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: the board is panned
  await panBy(page, -400, -300);

  // then: the board itself has not scrolled
  const held = await corkboard(page).evaluate((it) => ({
    overflow: getComputedStyle(it).overflowY,
    scrolled: it.scrollTop + it.scrollLeft,
  }));
  expect(held.overflow, "panning replaced the scroll stopgap").toBe("hidden");
  expect(held.scrolled).toBe(0);
  await expect(surface(page)).toHaveAttribute(
    "style",
    /translate\(-400px, -300px\)/,
  );
});

test("the action bar follows its card when the board is zoomed", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "BarFollowsZoom");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const board = await corkboard(page).boundingBox();
  const before = await bar(page).boundingBox();
  expect(Math.round(before!.x - board!.x)).toBeGreaterThan(0);

  // when: the board is zoomed in
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  // then: the action bar goes where its card goes
  const card = await seenAt(page, "The loom remembers");
  const after = await bar(page).boundingBox();
  expect(
    Math.round(after!.x - board!.x),
    "the bar belongs to the card, so it goes where the card goes",
  ).toBe(card.x);
  expect(after!.y).not.toBe(before!.y);
});

test("pressing the bare board to pan lets go of the selected card", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "PanDeselects");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: the bare board is pressed to pan
  await panBy(page, -120, -60);

  // then: no card is selected
  await expect(
    bar(page),
    "a press on bare board means nothing is chosen",
  ).toHaveCount(0);
});

test("the surface the board is drawn on carries no styling of its own", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "BareSurface");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // then: the surface it is drawn on is no box of its own
  const drawn = await surface(page).evaluate((it) => {
    const held = getComputedStyle(it);
    const box = it.getBoundingClientRect();

    return {
      width: Math.round(box.width),
      height: Math.round(box.height),
      border: held.borderTopWidth,
      padding: held.paddingTop,
      background: held.backgroundColor,
    };
  });

  expect(
    drawn,
    "the surface is an origin to hang cards off, not a box",
  ).toEqual({
    width: 0,
    height: 0,
    border: "0px",
    padding: "0px",
    background: "rgba(0, 0, 0, 0)",
  });
});

test("a card can be dragged past the origin onto negative ground", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "BehindOrigin");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(cardNamed(page, "The loom remembers")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );

  // when: it is dragged past the origin
  await dragBy(page, cardNamed(page, "The loom remembers"), -95, -70);

  // then: it lands behind the origin
  await expect(
    cardNamed(page, "The loom remembers"),
    "the board runs in every direction, so a card may sit behind the origin",
  ).toHaveAttribute("style", /left: -55px; top: -30px;/);

  // and: stays there after a reload
  await saved();
  await page.reload();
  await onTheBoard(page);
  await expect(cardNamed(page, "The loom remembers")).toHaveAttribute(
    "style",
    /left: -55px; top: -30px;/,
  );
});

test("panning the board does not drag a text selection along with it", async ({
  page,
}) => {
  // given: a card on the board
  await anOpenProject(page, "NoSelection");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // when: the board is panned
  await panBy(page, -200, -120);

  // then: no text is selected
  expect(
    await page.evaluate(() => String(window.getSelection())),
    "a pan that grabs text gets cancelled by the browser halfway through",
  ).toBe("");
  // and: the pan went all the way
  await expect(surface(page)).toHaveAttribute(
    "style",
    /translate\(-200px, -120px\)/,
  );
});

test("clicking a selected card never takes its action bar away", async ({
  page,
}) => {
  // given: a selected card
  await anOpenProject(page, "NoFlash");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  // when: it is pressed without becoming a drag
  await page.evaluate(() => {
    const card = document.querySelector(".pinned")!;
    const fire = (type: string, extra: PointerEventInit = {}) =>
      card.dispatchEvent(
        new PointerEvent(type, { bubbles: true, pointerId: 4, ...extra }),
      );

    fire("pointerdown", { clientX: 100, clientY: 100 });
    fire("pointermove", { clientX: 102, clientY: 101 });
  });

  // then: its action bar stays
  await expect(
    bar(page),
    "a press that has not become a drag must leave the bar alone",
  ).toHaveCount(1);

  // when: it is let go
  await page.evaluate(() => {
    document
      .querySelector(".pinned")!
      .dispatchEvent(
        new PointerEvent("pointerup", { bubbles: true, pointerId: 4 }),
      );
  });

  // then: its action bar stays
  await expect(bar(page)).toHaveCount(1);
});
