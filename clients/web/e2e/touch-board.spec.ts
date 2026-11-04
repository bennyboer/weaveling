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
  // given: a project open on a phone
  await anOpenProject(page, "PhoneCapture");

  // when: an idea is captured
  await captureIdea(page, "The loom remembers");

  // then: it is listed
  await expect(page.getByRole("list", { name: "Ideas" }).getByText("The loom remembers")).toBeVisible();
});

test("a scene is written on a phone", async ({ page }) => {
  // given: a book open in the writing view on a phone
  await anOpenProject(page, "PhoneWriting");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);

  // when: a scene is tapped and written in
  await scene(page, first).tap();
  await page.keyboard.type("The loom stood silent.");

  // then: what is written is saved
  await expect(saveState(page)).toHaveText("All saved");
  await expect(scene(page, first)).toContainText("The loom stood silent.");
});

test("one finger on the bare board pans it", async ({ page, context }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePanning");
  const before = await seenAt(page, "The loom remembers");
  // and: a bare spot on it
  const from = await bareSpot(page);

  // when: one finger drags from there
  await (await fingers(page, context)).drag(from, { x: from.x - 60, y: from.y - 100 });

  // then: the board pans as far
  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    })
    .toEqual({ x: -60, y: -100 });
});

test("a card is moved with a thumb, and stays where it was put", async ({ page, context }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhoneMoving");
  const before = await seenAt(page, "The loom remembers");
  const from = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const placed = (await boardOnServer(page)).ideas[0].spot;

  // when: a thumb drags the card
  await (await fingers(page, context)).drag(from, { x: from.x + 80, y: from.y + 120 });

  // then: the card goes where the thumb takes it
  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    }, { message: "the card goes where the thumb takes it, its name included" })
    .toEqual({ x: 80, y: 120 });
  // and: it stays there
  await expect
    .poll(async () => (await boardOnServer(page)).ideas[0].spot)
    .toEqual({ x: placed.x + 80, y: placed.y + 120 });
});

test("two fingers pinching apart zoom the board in, and together zoom it out", async ({
  page,
  context,
}) => {
  // given: a board with a card on a phone, at its usual size
  await aBoardWithACard(page, "PhonePinch");
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("100%");
  const hands = await fingers(page, context);

  // when: two fingers pinch apart
  const middle = middleOf((await corkboard(page).boundingBox())!);

  await hands.pinch(middle, 60, 120);
  // then: the board is zoomed in
  await expect(reading).toHaveText("200%");
  // when: two fingers pinch together
  await hands.pinch(middle, 160, 40);

  // then: the board is zoomed out
  await expect(reading).toHaveText("50%");
});

test("what lies between the fingers stays between them as they pinch", async ({ page, context }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePinchAround");
  const card = (await cardNamed(page, "The loom remembers").boundingBox())!;
  const between = middleOf(card);

  // when: two fingers pinch apart around the card
  await (await fingers(page, context)).pinch(between, 40, 80);

  // then: the board is zoomed in
  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).toHaveText("200%");
  // and: the card stays between the fingers
  const now = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  expect(Math.abs(now.x - between.x), "the card's middle stays under the fingers' middle").toBeLessThan(3);
  expect(Math.abs(now.y - between.y)).toBeLessThan(3);
});

test("a double tap on the bare board captures an idea there", async ({ page, context }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhoneDoubleTap");

  // when: the bare board is tapped twice
  await (await fingers(page, context)).doubleTap(await bareSpot(page));

  // then: an idea is captured there
  await expect(corkboard(page).getByRole("textbox", { name: "What is the idea?" })).toBeVisible();
});

test("a selected card's grips are big enough for a finger", async ({ page }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhoneGrips");

  // when: the card is tapped
  await cardNamed(page, "The loom remembers").tap();
  // then: it is selected
  await expect(cardNamed(page, "The loom remembers")).toHaveClass(/selected/);

  // and: each of its grips is about a fingertip across
  const grips = cardNamed(page, "The loom remembers").locator(".grip");
  await expect(grips).toHaveCount(8);
  for (const grip of await grips.all()) {
    const box = (await grip.boundingBox())!;
    const side = (await grip.getAttribute("class"))!.replace("grip ", "");
    const across = {
      top: box.height,
      bottom: box.height,
      left: box.width,
      right: box.width,
    }[side] ?? Math.min(box.width, box.height);
    expect(across, `the ${side} grip is about a fingertip across`).toBeGreaterThanOrEqual(44);
    expect(Math.max(box.width, box.height), `the ${side} grip is there to be found`).toBeGreaterThan(30);
  }
});

test("a thumb near the edge of a card not selected moves it rather than resizing it", async ({
  page,
  context,
}) => {
  // given: a board with a card on a phone, not selected
  await aBoardWithACard(page, "PhoneEdgeMoves");
  const before = await seenAt(page, "The loom remembers");
  const box = (await cardNamed(page, "The loom remembers").boundingBox())!;
  const nearTheEdge = { x: box.x + box.width / 2, y: box.y + 6 };

  // when: a thumb near its edge drags it
  await (await fingers(page, context)).drag(nearTheEdge, { x: nearTheEdge.x + 40, y: nearTheEdge.y + 60 });

  // then: the card is moved, not resized
  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y, width: now.width, height: now.height };
    })
    .toEqual({ x: 40, y: 60, width: before.width, height: before.height });
});

test("a finger on a selected card's grip, just beside its edge, resizes it", async ({
  page,
  context,
}) => {
  // given: a board with a selected card on a phone
  await aBoardWithACard(page, "PhoneGripPulled");
  await cardNamed(page, "The loom remembers").tap();
  await expect(cardNamed(page, "The loom remembers")).toHaveClass(/selected/);
  const before = await seenAt(page, "The loom remembers");
  const box = (await cardNamed(page, "The loom remembers").boundingBox())!;
  const besideTheEdge = { x: box.x + box.width + 15, y: box.y + box.height / 2 };

  // when: a finger beside its edge drags outwards
  await (await fingers(page, context)).drag(besideTheEdge, { x: besideTheEdge.x + 42, y: besideTheEdge.y });

  // then: the card is resized as far
  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).width - before.width, {
      message: "a fingertip lands beside the edge as often as on it",
    })
    .toBe(42);
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
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePinchBare");
  const board = (await corkboard(page).boundingBox())!;
  const middle = { x: board.x + board.width / 2, y: board.y + board.height - 200 };
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);

  // when: two fingers pinch apart on the bare board
  await (await fingers(page, context)).pinch(middle, 40, 80);

  // then: the board is zoomed in around the fingers, and not panned besides
  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).toHaveText("200%");
  const now = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  expect(Math.abs(now.x - (middle.x + 2 * (card.x - middle.x))), "zoomed around the fingers, and not panned besides").toBeLessThan(3);
  expect(Math.abs(now.y - (middle.y + 2 * (card.y - middle.y)))).toBeLessThan(3);
});

test("a pinch with a finger landing on a card neither selects nor moves it", async ({
  page,
  context,
}) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePinchOverCard");
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const board = (await corkboard(page).boundingBox())!;
  const bare = { x: card.x, y: board.y + board.height - 120 };
  const placed = (await boardOnServer(page)).ideas[0].spot;
  const hands = await fingers(page, context);

  // when: a pinch begins on the bare board and its second finger lands on the card
  await hands.press([bare]);
  await hands.press([bare, card]);
  for (const now of steps([bare, card], [{ x: bare.x, y: bare.y + 40 }, { x: card.x, y: card.y - 40 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);

  // then: the board is zoomed
  await expect(zooming(page).getByRole("button", { name: "Reset the zoom" })).not.toHaveText("100%");
  // and: the card is neither selected nor moved
  await expect(cardNamed(page, "The loom remembers")).not.toHaveClass(/selected/);
  expect((await boardOnServer(page)).ideas[0].spot).toEqual(placed);
});

test("a card a finger began to carry stays put once a second finger turns it into a pinch", async ({
  page,
  context,
}) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePinchAfterCarry");
  const card = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);
  const board = (await corkboard(page).boundingBox())!;
  const bare = { x: card.x, y: board.y + board.height - 120 };
  const placed = (await boardOnServer(page)).ideas[0].spot;
  const hands = await fingers(page, context);
  const carried = { x: card.x + 30, y: card.y };

  // when: a finger begins to carry the card
  await hands.press([card]);
  for (const now of steps([card], [carried])) {
    await hands.moveTo(now);
  }
  // and: a second finger turns it into a pinch
  await hands.press([carried, bare]);
  for (const now of steps([carried, bare], [{ x: carried.x, y: carried.y - 40 }, { x: bare.x, y: bare.y + 40 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);
  await page.waitForTimeout(300);

  // then: the card stays where it was put
  expect(
    (await boardOnServer(page)).ideas[0].spot,
    "the second finger makes it a pinch, and a pinch moves no card",
  ).toEqual(placed);
});

test("lifting one finger after a pinch pans the board with the other", async ({ page, context }) => {
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhonePinchThenPan");
  const board = (await corkboard(page).boundingBox())!;
  const one = { x: board.x + 60, y: board.y + board.height - 120 };
  const other = { x: board.x + 160, y: board.y + board.height - 120 };
  const hands = await fingers(page, context);

  // when: two fingers pinch
  await hands.press([one]);
  await hands.press([one, other]);
  for (const now of steps([one, other], [{ x: one.x - 20, y: one.y }, { x: other.x + 20, y: other.y }])) {
    await hands.moveTo(now);
  }
  // and: one is lifted and the other moves on
  await hands.lift([{ x: other.x + 20, y: other.y, id: 1 }]);
  const before = await seenAt(page, "The loom remembers");
  for (const now of steps([{ x: one.x - 20, y: one.y }], [{ x: one.x - 20, y: one.y - 100 }])) {
    await hands.moveTo(now);
  }
  await hands.lift([]);

  // then: the board pans with the other
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
  // given: a board with a card on a phone
  await aBoardWithACard(page, "PhoneRunningTotals");
  const before = await seenAt(page, "The loom remembers");
  const placed = (await boardOnServer(page)).ideas[0].spot;

  // when: a pointer telling its movement as a running total drags the card
  await dragReportingRunningTotals(cardNamed(page, "The loom remembers"), { x: 80, y: 120 });

  // then: the card lands where the pointer does
  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return { x: now.x - before.x, y: now.y - before.y };
    }, { message: "where the pointer is, not the sum of what it said it moved" })
    .toEqual({ x: 80, y: 120 });
  // and: it stays there
  await expect
    .poll(async () => (await boardOnServer(page)).ideas[0].spot)
    .toEqual({ x: placed.x + 80, y: placed.y + 120 });
});

test("a grip pulled by such a pointer stretches the card as far as the pointer went", async ({
  page,
}) => {
  // given: a board with a selected card on a phone
  await aBoardWithACard(page, "PhoneRunningTotalsGrip");
  await cardNamed(page, "The loom remembers").tap();
  const before = await seenAt(page, "The loom remembers");

  // when: such a pointer pulls a grip
  await dragReportingRunningTotals(
    cardNamed(page, "The loom remembers").locator(".grip.right"),
    { x: 42, y: 0 },
  );

  // then: the card stretches as far as the pointer went
  await expect
    .poll(async () => (await seenAt(page, "The loom remembers")).width - before.width, {
      message: "168 and 42 make 210, on the grid",
    })
    .toBe(42);
});
