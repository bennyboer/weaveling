import { test, expect, type Page } from "@playwright/test";

import { aNewProject, aLooseScene, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";

const API = "http://127.0.0.1:3000/api";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

const scene = (page: Page, id: string) =>
  flow(page).locator(`[data-scene="${id}"] .ProseMirror`);

const grip = (page: Page, id: string) =>
  flow(page)
    .locator(`.scene-row:has([data-scene="${id}"])`)
    .getByRole("button", { name: "Move scene" });

const projectOf = (page: Page) =>
  new URL(page.url()).pathname.split("/")[2].split("-").pop();

async function theBook(page: Page) {
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project: projectOf(page) },
  });
  const outline = await opened.json();

  return outline.sections.map(
    (section: {
      title: string;
      attachments: { kind: string; id: string }[];
    }) => [
      section.title,
      section.attachments
        .filter((attached) => attached.kind === "scene")
        .map((attached) => attached.id),
    ],
  );
}

async function writingIn(
  page: Page,
  named: string,
  sections: Record<string, number>,
) {
  await aNewProject(page, named);
  const made = await aBookWithScenes(page, sections);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();

  return made;
}

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
  const made = await writingIn(page, "Carrying", { "Part One": 3 });
  const [first, second, third] = made["Part One"];

  await pickedUp(page, first);
  await carriedBelow(page, third);
  await expect(
    flow(page).locator(".insertion.landing"),
    "where it will land is shown while it is carried",
  ).toHaveCount(1);
  await page.mouse.up();

  await expect
    .poll(() => theBook(page))
    .toEqual([["Part One", [second, third, first]]]);
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
});

test("a scene carried into another section lands there", async ({ page }) => {
  const made = await writingIn(page, "Elsewhere", {
    "Part One": 2,
    "Part Two": 1,
  });
  const [first, second] = made["Part One"];
  const [other] = made["Part Two"];

  await pickedUp(page, first);
  await carriedBelow(page, other);
  await page.mouse.up();

  await expect
    .poll(() => theBook(page))
    .toEqual([
      ["Part One", [second]],
      ["Part Two", [other, first]],
    ]);
});

test("chosen scenes are carried together, in their order", async ({ page }) => {
  const made = await writingIn(page, "Together", { "Part One": 4 });
  const [first, second, third, fourth] = made["Part One"];
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  await pickedUp(page, second);
  await carriedBelow(page, fourth);
  await page.mouse.up();

  await expect
    .poll(() => theBook(page))
    .toEqual([["Part One", [third, fourth, first, second]]]);
});

test("Escape puts a carried scene back where it was", async ({ page }) => {
  const made = await writingIn(page, "PutBack", { "Part One": 3 });
  const [first, second, third] = made["Part One"];
  let moved = false;
  page.on("request", (request) => {
    if (request.url().includes("/attachments")) {
      moved = true;
    }
  });

  await pickedUp(page, first);
  await carriedBelow(page, third);
  await page.keyboard.press("Escape");
  await page.mouse.up();

  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
  await expect(flow(page).locator(".insertion.landing")).toHaveCount(0);
  expect(moved).toBe(false);
  expect(await theBook(page)).toEqual([["Part One", [first, second, third]]]);
});

test("let go right below itself, a scene stays put", async ({ page }) => {
  const made = await writingIn(page, "StaysPut", { "Part One": 2 });
  const [first, second] = made["Part One"];
  let moved = false;
  page.on("request", (request) => {
    if (request.url().includes("/attachments")) {
      moved = true;
    }
  });

  await pickedUp(page, first);
  await carriedBelow(page, first);
  await page.mouse.up();

  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(0);
  expect(moved).toBe(false);
  expect(await theBook(page)).toEqual([["Part One", [first, second]]]);
});

test("scenes outside the book have no grip, having no place to be moved to", async ({
  page,
}) => {
  await writingIn(page, "NoGrip", { "Part One": 1 });
  const loose = await aLooseScene(page);
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));
  await expect(scene(page, loose)).toBeVisible();

  await expect(grip(page, loose)).toHaveCount(0);
});

test("carried to the window's edge, the book scrolls on towards where it can land", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 600 });
  const made = await writingIn(page, "Scrolling", { "Part One": 20 });
  const [first] = made["Part One"];
  await pickedUp(page, first);

  for (let step = 0; step < 20; step++) {
    await page.mouse.move(500, 590 - (step % 2));
  }

  const scrolled = await page.evaluate(() => window.scrollY);
  expect(scrolled).toBeGreaterThan(100);

  for (let step = 0; step < 10; step++) {
    await page.mouse.move(500, 10 + (step % 2));
  }
  expect(
    await page.evaluate(() => window.scrollY),
    "and at the top edge back up",
  ).toBeLessThan(scrolled);
  await page.keyboard.press("Escape");
  await page.mouse.up();
});

test("a drag the browser takes away puts the scene back", async ({ page }) => {
  const made = await writingIn(page, "Cancelled", { "Part One": 2 });
  const [first, second] = made["Part One"];
  await pickedUp(page, first);
  await carriedBelow(page, second);
  await expect(flow(page).locator(".scene-slot.carried")).toHaveCount(1);

  await page.evaluate(() =>
    window.dispatchEvent(new PointerEvent("pointercancel")),
  );
  await page.mouse.up();

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
    const made = await writingIn(page, "Touching", { "Part One": 3 });
    const [first, second, third] = made["Part One"];
    const held = (await grip(page, first).boundingBox())!;
    const below = (await flow(page)
      .locator(`[data-scene="${third}"]`)
      .boundingBox())!;
    const touch = await context.newCDPSession(page);
    const at = (x: number, y: number) => [{ x, y, id: 1 }];
    const start = { x: held.x + held.width / 2, y: held.y + held.height / 2 };
    const end = { x: start.x, y: below.y + below.height + 24 };

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

    await expect
      .poll(() => theBook(page))
      .toEqual([["Part One", [second, third, first]]]);
  });
});
