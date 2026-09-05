import { test, expect } from "@playwright/test";

import {
  anOpenProject,
  bar,
  capture,
  cardNamed,
  corkboard,
  dragBy,
  openTheBoard,
  panBy,
  seenAt,
  select,
  surface,
  zooming,
} from "./support/board";

const spotOf = (page: Page, named: string) =>
  cardNamed(page, named).evaluate((it) => it.getAttribute("style"));

type Page = Parameters<Parameters<typeof test>[1]>[0]["page"];

test("dragging the bare board carries the cards with it", async ({ page }) => {
  await anOpenProject(page, "Panning");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const before = await seenAt(page, "The loom remembers");

  await panBy(page, -150, -70);

  const after = await seenAt(page, "The loom remembers");
  expect(after.x).toBe(before.x - 150);
  expect(after.y).toBe(before.y - 70);
  await expect(
    cardNamed(page, "The loom remembers"),
    "panning moves the window, not the piece",
  ).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

test("a piece pinned beyond the edge can be panned to", async ({ page }) => {
  await anOpenProject(page, "FarAway");
  await capture(page, "Far away");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin Far away" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const board = await fetch("/api/boards", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project }),
    }).then((it) => it.json());

    await fetch(`/api/boards/${board.id}/pieces/${board.pieces[0].piece}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ spot: { x: 40, y: 900 } }),
    });
  });
  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  const away = await seenAt(page, "Far away");
  expect(away.y, "the piece starts far below the visible board").toBeGreaterThan(500);

  await panBy(page, 0, -400);
  await panBy(page, 0, -400);

  const reached = await seenAt(page, "Far away");
  expect(reached.y, "panning should bring it into view").toBeLessThan(300);
});

test("a plain wheel leaves the board alone and scrolls the page", async ({ page }) => {
  await anOpenProject(page, "Wheeling");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const before = await seenAt(page, "The loom remembers");
  const middle = await corkboard(page).boundingBox();

  await page.mouse.move(middle!.x + middle!.width / 2, middle!.y + middle!.height / 2);
  await page.mouse.wheel(0, 200);
  await page.waitForTimeout(200);

  expect(
    await seenAt(page, "The loom remembers"),
    "the board must not steal the page's scroll",
  ).toEqual(before);
  await expect(surface(page)).toHaveAttribute("style", /translate\(0px, 0px\)/);
});

test("holding ctrl turns the wheel into a zoom", async ({ page }) => {
  await anOpenProject(page, "Zooming");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const before = await seenAt(page, "The loom remembers");
  const middle = await corkboard(page).boundingBox();

  await page.keyboard.down("Control");
  await page.mouse.move(middle!.x + middle!.width / 2, middle!.y + middle!.height / 2);
  await page.mouse.wheel(0, -200);
  await page.keyboard.up("Control");

  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).width)
    .toBeGreaterThan(before.width);
  await expect(
    cardNamed(page, "The loom remembers"),
    "zooming draws the card larger without resizing it",
  ).toHaveAttribute("style", /width: 168px; height: 84px;/);
});

test("the zoom controls take the board in and out and back to where it started", async ({
  page,
}) => {
  await anOpenProject(page, "ZoomControls");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("100%");
  const before = await seenAt(page, "The loom remembers");

  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  await expect(reading).toHaveText("110%");
  expect((await seenAt(page, "The loom remembers")).width).toBeGreaterThan(before.width);

  await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  await zooming(page).getByRole("button", { name: "Zoom out" }).click();

  await expect(reading).toHaveText("91%");

  await reading.click();

  await expect(reading).toHaveText("100%");
  expect(await seenAt(page, "The loom remembers")).toEqual(before);
});

test("resetting the zoom keeps the board where it was panned to", async ({ page }) => {
  await anOpenProject(page, "ResetKeepsPan");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await panBy(page, -180, -90);
  await expect(surface(page)).toHaveAttribute("style", /translate\(-180px, -90px\)/);
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("121%");

  await reading.click();

  await expect(reading).toHaveText("100%");
  const home = await surface(page).getAttribute("style");
  expect(home, "resetting the zoom is not the same as going home").not.toMatch(
    /translate\(0px, 0px\)/,
  );
});

test("the zoom buttons hold the middle of the board still", async ({ page }) => {
  await anOpenProject(page, "ZoomFromMiddle");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = await corkboard(page).boundingBox();
  const middle = { x: board!.width / 2, y: board!.height / 2 };

  const underTheMiddle = async () => {
    const drawn = (await surface(page).getAttribute("style")) ?? "";
    const [x, y] = /translate\((-?[\d.]+)px, (-?[\d.]+)px\)/.exec(drawn)!.slice(1).map(Number);
    const [zoom] = /scale\(([\d.]+)\)/.exec(drawn)!.slice(1).map(Number);

    return {
      x: Math.round((middle.x - x) / zoom),
      y: Math.round((middle.y - y) / zoom),
    };
  };

  const before = await underTheMiddle();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  const after = await underTheMiddle();
  expect(after.x, "zooming in should not slide the board towards a corner").toBeCloseTo(
    before.x,
    -1,
  );
  expect(after.y).toBeCloseTo(before.y, -1);
});

test("a card dragged on a zoomed board still lands under the pointer", async ({ page }) => {
  await anOpenProject(page, "ZoomedDrag");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  for (const _ of [1, 2, 3, 4, 5, 6, 7]) {
    await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  }
  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).toHaveText("51%");
  const before = await seenAt(page, "The loom remembers");

  await dragBy(page, cardNamed(page, "The loom remembers"), 100, 50);

  const after = await seenAt(page, "The loom remembers");
  expect(after.x - before.x, "the card should follow the pointer, not half of it").toBeCloseTo(
    100,
    -1,
  );
  expect(after.y - before.y).toBeCloseTo(50, -1);
});

test("the action bar keeps its size however far the board is zoomed out", async ({ page }) => {
  await anOpenProject(page, "BarSize");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const before = await bar(page).boundingBox();

  for (const _ of [1, 2, 3, 4, 5, 6, 7]) {
    await zooming(page).getByRole("button", { name: "Zoom out" }).click();
  }

  const after = await bar(page).boundingBox();
  expect(
    Math.round(after!.height),
    "chrome is read at arm's length, so it does not shrink with the board",
  ).toBe(Math.round(before!.height));
  expect(Math.round(after!.width)).toBe(Math.round(before!.width));
});

test("a piece is pinned where the author is looking, not at the board's origin", async ({
  page,
}) => {
  await anOpenProject(page, "PinInView");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  await panBy(page, -300, -200);
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  const landed = await seenAt(page, "She never returned");
  expect(landed.x, "a fresh pin must not land off the visible board").toBeGreaterThan(0);
  expect(landed.y).toBeGreaterThan(0);
  expect(await spotOf(page, "She never returned")).toMatch(/left: 340px; top: 240px;/);
});

test("the board itself does not scroll any more", async ({ page }) => {
  await anOpenProject(page, "NoScroll");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await panBy(page, -400, -300);

  const held = await corkboard(page).evaluate((it) => ({
    overflow: getComputedStyle(it).overflowY,
    scrolled: it.scrollTop + it.scrollLeft,
  }));
  expect(held.overflow, "panning replaced the scroll stopgap").toBe("hidden");
  expect(held.scrolled).toBe(0);
  await expect(surface(page)).toHaveAttribute("style", /translate\(-400px, -300px\)/);
});

test("the action bar follows its card when the board is zoomed", async ({ page }) => {
  await anOpenProject(page, "BarFollowsZoom");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const board = await corkboard(page).boundingBox();
  const before = await bar(page).boundingBox();
  expect(Math.round(before!.x - board!.x)).toBeGreaterThan(0);

  await zooming(page).getByRole("button", { name: "Zoom in" }).click();
  await zooming(page).getByRole("button", { name: "Zoom in" }).click();

  const card = await seenAt(page, "The loom remembers");
  const after = await bar(page).boundingBox();
  expect(
    Math.round(after!.x - board!.x),
    "the bar belongs to the card, so it goes where the card goes",
  ).toBe(card.x);
  expect(after!.y).not.toBe(before!.y);
});

test("pressing the bare board to pan lets go of the selected card", async ({ page }) => {
  await anOpenProject(page, "PanDeselects");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await panBy(page, -120, -60);

  await expect(bar(page), "a press on bare board means nothing is chosen").toHaveCount(0);
});

test("the surface the board is drawn on carries no styling of its own", async ({ page }) => {
  await anOpenProject(page, "BareSurface");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

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

  expect(drawn, "the surface is an origin to hang cards off, not a box").toEqual({
    width: 0,
    height: 0,
    border: "0px",
    padding: "0px",
    background: "rgba(0, 0, 0, 0)",
  });
});

test("a card can be dragged past the origin onto negative ground", async ({ page }) => {
  await anOpenProject(page, "BehindOrigin");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(cardNamed(page, "The loom remembers")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );

  await dragBy(page, cardNamed(page, "The loom remembers"), -95, -70);

  await expect(
    cardNamed(page, "The loom remembers"),
    "the board runs in every direction, so a card may sit behind the origin",
  ).toHaveAttribute("style", /left: -55px; top: -30px;/);

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(cardNamed(page, "The loom remembers")).toHaveAttribute(
    "style",
    /left: -55px; top: -30px;/,
  );
});

test("panning the board does not drag a text selection along with it", async ({ page }) => {
  await anOpenProject(page, "NoSelection");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await panBy(page, -200, -120);

  expect(
    await page.evaluate(() => String(window.getSelection())),
    "a pan that grabs text gets cancelled by the browser halfway through",
  ).toBe("");
  await expect(surface(page)).toHaveAttribute("style", /translate\(-200px, -120px\)/);
});

test("clicking a selected card never takes its action bar away", async ({ page }) => {
  await anOpenProject(page, "NoFlash");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await page.evaluate(() => {
    const card = document.querySelector(".pinned")!;
    const fire = (type: string, extra: PointerEventInit = {}) =>
      card.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerId: 4, ...extra }));

    fire("pointerdown");
    fire("pointermove", { movementX: 2, movementY: 1 });
  });

  await expect(bar(page), "a press that has not become a drag must leave the bar alone").toHaveCount(
    1,
  );

  await page.evaluate(() => {
    document
      .querySelector(".pinned")!
      .dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerId: 4 }));
  });

  await expect(bar(page)).toHaveCount(1);
});
