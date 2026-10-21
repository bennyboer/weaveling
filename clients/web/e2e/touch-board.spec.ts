import { test, expect, type BrowserContext, type Page } from "@playwright/test";

import { cardNamed, corkboard, seenAt, zooming } from "./support/board";
import { aBookWithScenes } from "./support/book";
import { anOpenProject, captureIdea, openTheBoard } from "./support/shell";
import { openTheWritingView, saveState, scene } from "./support/writing";

test.use({
  hasTouch: true,
  isMobile: true,
  viewport: { width: 390, height: 844 },
});

type Point = { x: number; y: number };

async function fingers(page: Page, context: BrowserContext) {
  const touch = await context.newCDPSession(page);
  const send = (type: string, points: Point[]) =>
    touch.send("Input.dispatchTouchEvent", {
      type,
      touchPoints: points.map((point, id) => ({ ...point, id })),
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

test("a card is moved with a thumb", async ({ page, context }) => {
  test.fail(true, "M15: the browser cancels a finger's drag on a card after its first move");
  await aBoardWithACard(page, "PhoneMoving");
  const before = await seenAt(page, "The loom remembers");
  const from = middleOf((await cardNamed(page, "The loom remembers").boundingBox())!);

  await (await fingers(page, context)).drag(from, { x: from.x + 80, y: from.y + 120 });

  await expect
    .poll(async () => {
      const now = await seenAt(page, "The loom remembers");
      return Math.abs(now.x - before.x) + Math.abs(now.y - before.y) > 100;
    }, { message: "the card goes where the thumb takes it, not the board under it" })
    .toBe(true);
});

test("two fingers pinching apart zoom the board in", async ({ page, context }) => {
  test.fail(true, "M15: pinch-to-zoom is not wired; ctrl and the wheel are the only zoom");
  await aBoardWithACard(page, "PhonePinch");
  const reading = zooming(page).getByRole("button", { name: "Reset the zoom" });
  await expect(reading).toHaveText("100%");

  await (await fingers(page, context)).pinch(await bareSpot(page), 60, 180);

  await expect(reading).not.toHaveText("100%");
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
