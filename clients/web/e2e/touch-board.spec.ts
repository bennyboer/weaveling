import { test, expect, type BrowserContext, type Locator, type Page } from "@playwright/test";

import { boardOnServer, cardNamed, corkboard, seenAt, zooming } from "./support/board";
import { aBookWithScenes } from "./support/book";
import { anOpenProject, captureIdea, openTheBoard } from "./support/shell";
import { openTheWritingView, saveState, scene } from "./support/writing";

test.use({
  hasTouch: true,
  isMobile: true,
  viewport: { width: 390, height: 844 },
});

type Point = { x: number; y: number; id?: number };

async function fingers(page: Page, context: BrowserContext) {
  const touch = await context.newCDPSession(page);
  const send = (type: string, points: Point[]) =>
    touch.send("Input.dispatchTouchEvent", {
      type,
      touchPoints: points.map((point, nth) => ({ ...point, id: point.id ?? nth })),
    });

  return {
    async drag(from: Point, to: Point) {
      await send("touchStart", [from]);
      for (let step = 1; step <= 10; step++) {
        await send("touchMove", [
          { x: from.x + ((to.x - from.x) * step) / 10, y: from.y + ((to.y - from.y) * step) / 10 },
        ]);
      }
      await send("touchEnd", []);
    },
    async pinch(around: Point, fromGap: number, toGap: number) {
      const apart = (gap: number) => [
        { x: around.x - gap / 2, y: around.y },
        { x: around.x + gap / 2, y: around.y },
      ];
      await send("touchStart", apart(fromGap));
      for (let step = 1; step <= 10; step++) {
        await send("touchMove", apart(fromGap + ((toGap - fromGap) * step) / 10));
      }
      await send("touchEnd", []);
    },
    async press(points: Point[]) {
      await send("touchStart", points);
    },
    async moveTo(points: Point[]) {
      await send("touchMove", points);
    },
    async lift(points: Point[]) {
      await send("touchEnd", points);
    },
    async doubleTap(at: Point) {
      for (let tap = 0; tap < 2; tap++) {
        await send("touchStart", [at]);
        await send("touchEnd", []);
      }
    },
  };
}

const middleOf = (box: { x: number; y: number; width: number; height: number }) => ({
  x: box.x + box.width / 2,
  y: box.y + box.height / 2,
});

async function aBoardWithACard(page: Page, named: string) {
  await anOpenProject(page, named);
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  const drawer = page.getByRole("button", { name: /Not on the board/ });
  await drawer.tap();
  await page.getByRole("button", { name: "Pin The loom remembers" }).tap();
  await expect(cardNamed(page, "The loom remembers")).toBeVisible();
  if (await drawer.isVisible()) {
    await drawer.tap();
  }
}

async function bareSpot(page: Page): Promise<Point> {
  const board = (await corkboard(page).boundingBox())!;

  return { x: board.x + board.width - 40, y: board.y + board.height - 80 };
}

test("an idea is captured on a phone", async ({ page }) => {
  await anOpenProject(page, "PhoneCapture");

  await captureIdea(page, "The loom remembers");

  await expect(page.getByRole("list", { name: "Ideas" }).getByText("The loom remembers")).toBeVisible();
});

test("a scene is written on a phone", async ({ page }) => {
  await anOpenProject(page, "PhoneWriting");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);

  await scene(page, first).tap();
  await page.keyboard.type("The loom stood silent.");

  await expect(saveState(page)).toHaveText("All saved");
  await expect(scene(page, first)).toContainText("The loom stood silent.");
});

test("one finger on the bare board pans it", async ({ page, context }) => {
  await aBoardWithACard(page, "PhonePanning");
  const before = await seenAt(page, "The loom remembers");
  const from = await bareSpot(page);

  await (await fingers(page, context)).drag(from, { x: from.x - 60, y: from.y - 100 });

  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    })
    .toEqual({ x: -60, y: -100 });
});

test("a card is moved with a thumb, and stays where it was put", async ({ page, context }) => {
  await aBoardWithACard(page, "PhoneMoving");
  const before = await seenAt(page, "The loom remembers");
  const from = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const placed = (await boardOnServer(page)).ideas[0].spot;

  await (await fingers(page, context)).drag(from, { x: from.x + 80, y: from.y + 120 });

  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    }, { message: "the card goes where the thumb takes it, its name included" })
    .toEqual({ x: 80, y: 120 });
  await expect
    .poll(async () => (await boardOnServer(page)).ideas[0].spot)
    .toEqual({ x: placed.x + 80, y: placed.y + 120 });
});

test("two fingers pinching apart zoom the board in, and together zoom it out", async ({
  page,
  context,
}) => {
  await aBoardWithACard(page, "PhonePinch");
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("100%");
  const hands = await fingers(page, context);

  const middle = middleOf((await corkboard(page).boundingBox())!);

  await hands.pinch(middle, 60, 120);
  await expect(reading).toHaveText("200%");
  await hands.pinch(middle, 160, 40);

  await expect(reading).toHaveText("50%");
});

test("what lies between the fingers stays between them as they pinch", async ({ page, context }) => {
  await aBoardWithACard(page, "PhonePinchAround");
  const card = (await cardNamed(page, "The loom remembers").boundingBox())!;
  const between = middleOf(card);

  await (await fingers(page, context)).pinch(between, 40, 80);

  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).toHaveText("200%");
  const now = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  expect(Math.abs(now.x - between.x), "the card's middle stays under the fingers' middle").toBeLessThan(3);
  expect(Math.abs(now.y - between.y)).toBeLessThan(3);
});

test("a double tap on the bare board captures an idea there", async ({ page, context }) => {
  await aBoardWithACard(page, "PhoneDoubleTap");

  await (await fingers(page, context)).doubleTap(await bareSpot(page));

  await expect(corkboard(page).getByRole("textbox", { name: "What is the idea?" })).toBeVisible();
});

test("a selected card's grips are big enough for a finger", async ({ page }) => {
  test.fail(true, "M15: the grips are 7px, sized for a mouse");
  await aBoardWithACard(page, "PhoneGrips");

  await cardNamed(page, "The loom remembers").tap();

  const grip = (await cardNamed(page, "The loom remembers").locator(".grip.right").boundingBox())!;
  expect(Math.min(grip.width, grip.height), "a fingertip is about 44px across").toBeGreaterThanOrEqual(44);
});

const steps = (from: Point[], to: Point[]) =>
  Array.from({ length: 10 }, (_, nth) =>
    from.map((start, finger) => ({
      x: start.x + ((to[finger].x - start.x) * (nth + 1)) / 10,
      y: start.y + ((to[finger].y - start.y) * (nth + 1)) / 10,
    })),
  );

test("a pinch on the bare board keeps what lies between the fingers between them", async ({
  page,
  context,
}) => {
  await aBoardWithACard(page, "PhonePinchBare");
  const board = (await corkboard(page).boundingBox())!;
  const middle = { x: board.x + board.width / 2, y: board.y + board.height - 200 };
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);

  await (await fingers(page, context)).pinch(middle, 40, 80);

  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).toHaveText("200%");
  const now = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  expect(Math.abs(now.x - (middle.x + 2 * (card.x - middle.x))), "zoomed around the fingers, and not panned besides").toBeLessThan(3);
  expect(Math.abs(now.y - (middle.y + 2 * (card.y - middle.y)))).toBeLessThan(3);
});

test("a pinch with a finger landing on a card neither selects nor moves it", async ({
  page,
  context,
}) => {
  await aBoardWithACard(page, "PhonePinchOverCard");
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const board = (await corkboard(page).boundingBox())!;
  const bare = { x: card.x, y: board.y + board.height - 120 };
  const placed = (await boardOnServer(page)).ideas[0].spot;
  const hands = await fingers(page, context);

  await hands.press([bare]);
  await hands.press([bare, card]);
  for (const now of steps([bare, card], [{ x: bare.x, y: bare.y + 40 }, { x: card.x, y: card.y - 40 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);

  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).not.toHaveText("100%");
  await expect(cardNamed(page, "The loom remembers")).not.toHaveClass(/selected/);
  expect((await boardOnServer(page)).ideas[0].spot).toEqual(placed);
});

test("a card a finger began to carry stays put once a second finger turns it into a pinch", async ({
  page,
  context,
}) => {
  await aBoardWithACard(page, "PhonePinchAfterCarry");
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const board = (await corkboard(page).boundingBox())!;
  const bare = { x: card.x, y: board.y + board.height - 120 };
  const placed = (await boardOnServer(page)).ideas[0].spot;
  const hands = await fingers(page, context);
  const carried = { x: card.x + 30, y: card.y };

  await hands.press([card]);
  for (const now of steps([card], [carried])) {
    await hands.moveTo(now);
  }
  await hands.press([carried, bare]);
  for (const now of steps([carried, bare], [{ x: carried.x, y: carried.y - 40 }, { x: bare.x, y: bare.y + 40 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);
  await page.waitForTimeout(300);

  expect(
    (await boardOnServer(page)).ideas[0].spot,
    "the second finger makes it a pinch, and a pinch moves no card",
  ).toEqual(placed);
});

test("lifting one finger after a pinch pans the board with the other", async ({ page, context }) => {
  await aBoardWithACard(page, "PhonePinchThenPan");
  const board = (await corkboard(page).boundingBox())!;
  const one = { x: board.x + 60, y: board.y + board.height - 120 };
  const other = { x: board.x + 160, y: board.y + board.height - 120 };
  const hands = await fingers(page, context);

  await hands.press([one]);
  await hands.press([one, other]);
  for (const now of steps([one, other], [{ x: one.x - 20, y: one.y }, { x: other.x + 20, y: other.y }])) {
    await hands.moveTo(now);
  }
  await hands.lift([{ x: other.x + 20, y: other.y, id: 1 }]);
  const before = await seenAt(page, "The loom remembers");
  for (const now of steps([{ x: one.x - 20, y: one.y }], [{ x: one.x - 20, y: one.y - 100 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);

  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).y - before.y)
    .toBe(-100);
});


async function dragReportingRunningTotals(target: Locator, by: Point) {
  const from = middleOf((await target.boundingBox())!);

  await target.evaluate(
    (element, { from, by }) => {
      const fire = (type: string, x: number, y: number, moved: Point) =>
        element.dispatchEvent(
          new PointerEvent(type, {
            bubbles: true,
            cancelable: true,
            composed: true,
            pointerId: 42,
            pointerType: "touch",
            isPrimary: true,
            buttons: 1,
            clientX: x,
            clientY: y,
            movementX: moved.x,
            movementY: moved.y,
          }),
        );
      fire("pointerdown", from.x, from.y, { x: 0, y: 0 });
      for (let step = 1; step <= 10; step++) {
        const x = from.x + (by.x * step) / 10;
        const y = from.y + (by.y * step) / 10;
        fire("pointermove", x, y, { x: x - from.x, y: y - from.y });
      }
      fire("pointerup", from.x + by.x, from.y + by.y, { x: 0, y: 0 });
    },
    { from, by },
  );
}

test("a card carried by a pointer that reports its movement as a running total lands where the pointer does", async ({
  page,
}) => {
  await aBoardWithACard(page, "PhoneRunningTotals");
  const before = await seenAt(page, "The loom remembers");
  const placed = (await boardOnServer(page)).ideas[0].spot;

  await dragReportingRunningTotals(cardNamed(page, "The loom remembers"), { x: 80, y: 120 });

  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    }, { message: "where the pointer is, not the sum of what it said it moved" })
    .toEqual({ x: 80, y: 120 });
  await expect
    .poll(async () => (await boardOnServer(page)).ideas[0].spot)
    .toEqual({ x: placed.x + 80, y: placed.y + 120 });
});

test("a grip pulled by such a pointer stretches the card as far as the pointer went", async ({
  page,
}) => {
  await aBoardWithACard(page, "PhoneRunningTotalsGrip");
  await cardNamed(page, "The loom remembers").tap();
  const before = await seenAt(page, "The loom remembers");

  await dragReportingRunningTotals(
    cardNamed(page, "The loom remembers").locator(".grip.right"),
    { x: 42, y: 0 },
  );

  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).width - before.width, {
      message: "168 and 42 make 210, on the grid",
    })
    .toBe(42);
});
