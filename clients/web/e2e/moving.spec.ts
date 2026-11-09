import { test, expect, type Page } from "@playwright/test";

import { aLooseScene } from "./support/shell";
import { aBookInTheWritingView, flow, scene, theBook } from "./support/writing";

const grip = (page: Page, id: string) =>
  flow(page)
    .locator(`.scene-row:has([data-scene="${id}"])`)
    .getByRole("button", { name: "Move scene" });

async function pickedUp(page: Page, id: string) {
  await flow(page).locator(`.scene-row:has([data-scene="${id}"])`).hover();
  const held = (await grip(page, id).boundingBox())!;
  await page.mouse.move(held.x + held.width / 2, held.y + held.height / 2);
  await page.mouse.down();
}

async function carriedBelow(page: Page, id: string) {
  const below = (await flow(page)
    .locator(`[data-scene="${id}"]`)
    .boundingBox())!;
  await page.mouse.move(below.x, below.y + below.height + 24, { steps: 10 });
}

test("a scene carried by its grip lands below the scene it is let go under", async ({
  page,
}) => {
  // given: a book of three scenes
  const made = await aBookInTheWritingView(page, "Carrying", { "Part One": 3 });
  const [first, second, third] = made["Part One"];

  // when: the first is carried by its grip below the third
  await pickedUp(page, first);
  await carriedBelow(page, third);
  // then: where it will land is shown
  await expect(
    flow(page).locator(".insertion.landing"),
    "where it will land is shown while it is carried",
  ).toHaveCount(1);
  // when: it is let go
  await page.mouse.up();

  // then: it lands below the third
  await expect
    .poll(() => theBook(page))
    .toEqual([["Part One", [second, third, first]]]);
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
});

test("a scene carried into another section lands there", async ({ page }) => {
  // given: a book of two sections
  const made = await aBookInTheWritingView(page, "Elsewhere", {
    "Part One": 2,
    "Part Two": 1,
  });
  const [first, second] = made["Part One"];
  const [other] = made["Part Two"];

  // when: a scene is carried below one in the other section
  await pickedUp(page, first);
  await carriedBelow(page, other);
  await page.mouse.up();

  // then: it lands there
  await expect
    .poll(() => theBook(page))
    .toEqual([
      ["Part One", [second]],
      ["Part Two", [other, first]],
    ]);
});

test("chosen scenes are carried together, in their order", async ({ page }) => {
  // given: a book of four scenes, the first two chosen
  const made = await aBookInTheWritingView(page, "Together", { "Part One": 4 });
  const [first, second, third, fourth] = made["Part One"];
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  // when: the second is carried below the fourth
  await pickedUp(page, second);
  await carriedBelow(page, fourth);
  await page.mouse.up();

  // then: both land there, in their order
  await expect
    .poll(() => theBook(page))
    .toEqual([["Part One", [third, fourth, first, second]]]);
});

test("Escape puts a carried scene back where it was", async ({ page }) => {
  // given: a book of three scenes
  const made = await aBookInTheWritingView(page, "PutBack", { "Part One": 3 });
  const [first, second, third] = made["Part One"];
  let moved = false;
  page.on("request", (request) => {
    if (request.url().includes("/attachments")) {
      moved = true;
    }
  });

  // when: the first is carried below the third and escape pressed
  await pickedUp(page, first);
  await carriedBelow(page, third);
  await page.keyboard.press("Escape");
  await page.mouse.up();

  // then: it is back where it was, nothing moved
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
  await expect(flow(page).locator(".insertion.landing")).toHaveCount(0);
  expect(moved).toBe(false);
  expect(await theBook(page)).toEqual([["Part One", [first, second, third]]]);
});

test("let go right below itself, a scene stays put", async ({ page }) => {
  // given: a book of two scenes
  const made = await aBookInTheWritingView(page, "StaysPut", { "Part One": 2 });
  const [first, second] = made["Part One"];
  let moved = false;
  page.on("request", (request) => {
    if (request.url().includes("/attachments")) {
      moved = true;
    }
  });

  // when: the first is let go right below itself
  await pickedUp(page, first);
  await carriedBelow(page, first);
  await page.mouse.up();

  // then: it stays put, nothing moved
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
  expect(moved).toBe(false);
  expect(await theBook(page)).toEqual([["Part One", [first, second]]]);
});

test("scenes outside the book have no grip, having no place to be moved to", async ({
  page,
}) => {
  // given: a scene outside the book, shown
  await aBookInTheWritingView(page, "NoGrip", { "Part One": 1 });
  const loose = await aLooseScene(page);
  await expect(page).toHaveURL(/\/write$/);
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));
  await expect(scene(page, loose)).toBeVisible();

  // then: it has no grip
  await expect(grip(page, loose)).toHaveCount(0);
});

test("carried to the window's edge, the book scrolls on towards where it can land", async ({
  page,
}) => {
  // given: a long book in a short window
  await page.setViewportSize({ width: 1280, height: 600 });
  const made = await aBookInTheWritingView(page, "Scrolling", { "Part One": 20 });
  const [first] = made["Part One"];
  // and: a scene picked up
  await pickedUp(page, first);

  // when: it is carried to the bottom edge
  for (let step = 0; step < 20; step++) {
    await page.mouse.move(500, 590 - (step % 2));
  }

  // then: the book scrolls down
  const scrolled = await page.evaluate(() => window.scrollY);
  expect(scrolled).toBeGreaterThan(100);

  // when: it is carried to the top edge
  for (let step = 0; step < 10; step++) {
    await page.mouse.move(500, 10 + (step % 2));
  }
  // then: the book scrolls back up
  expect(
    await page.evaluate(() => window.scrollY),
    "and at the top edge back up",
  ).toBeLessThan(scrolled);
  await page.keyboard.press("Escape");
  await page.mouse.up();
});

test("a drag the browser takes away puts the scene back", async ({ page }) => {
  // given: a scene being carried
  const made = await aBookInTheWritingView(page, "Cancelled", { "Part One": 2 });
  const [first, second] = made["Part One"];
  await pickedUp(page, first);
  await carriedBelow(page, second);
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(1);

  // when: the browser takes the drag away
  await page.evaluate(() =>
    window.dispatchEvent(new PointerEvent("pointercancel")),
  );
  await page.mouse.up();

  // then: the scene is back where it was
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
  expect(await theBook(page)).toEqual([["Part One", [first, second]]]);
});

test.describe("on a touchscreen", () => {
  test.use({
    hasTouch: true,
    isMobile: true,
    viewport: { width: 390, height: 844 },
  });

  test("a scene is carried by a finger on its grip", async ({
    page,
    context,
  }) => {
    // given: a book of three scenes on a touchscreen
    const made = await aBookInTheWritingView(page, "Touching", { "Part One": 3 });
    const [first, second, third] = made["Part One"];
    const held = (await grip(page, first).boundingBox())!;
    const below = (await flow(page)
      .locator(`[data-scene="${third}"]`)
      .boundingBox())!;
    const touch = await context.newCDPSession(page);
    const at = (x: number, y: number) => [{ x, y, id: 1 }];
    const start = { x: held.x + held.width / 2, y: held.y + held.height / 2 };
    const end = { x: start.x, y: below.y + below.height + 24 };

    // when: the first is carried by a finger on its grip below the third
    await touch.send("Input.dispatchTouchEvent", {
      type: "touchStart",
      touchPoints: at(start.x, start.y),
    });
    for (let step = 1; step <= 10; step++) {
      await touch.send("Input.dispatchTouchEvent", {
        type: "touchMove",
        touchPoints: at(start.x, start.y + ((end.y - start.y) * step) / 10),
      });
    }
    await touch.send("Input.dispatchTouchEvent", {
      type: "touchEnd",
      touchPoints: [],
    });

    // then: it lands below the third
    await expect
      .poll(() => theBook(page))
      .toEqual([["Part One", [second, third, first]]]);
  });
});
