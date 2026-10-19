import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { flow, openTheWritingView, scene, statusNote } from "./support/writing";

const writingSettings = (page: Page) =>
  page.getByRole("dialog", { name: "Writing settings" });

async function typewriterScrolling(page: Page, wanted: boolean) {
  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await writingSettings(page)
    .getByLabel("Typewriter scrolling")
    .setChecked(wanted);
  await page.keyboard.press("Escape");
}

async function aSceneToWriteIn(page: Page, named: string) {
  await page.setViewportSize({ width: 1280, height: 600 });
  await aNewProject(page, named);
  const [only] = (
    await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 })
  )["Part One"];
  await openTheWritingView(page);

  return only;
}

async function paragraphs(page: Page, count: number) {
  for (let nth = 1; nth <= count; nth++) {
    await page.keyboard.type(`Paragraph ${nth} of the morning by the river.`);
    await page.keyboard.press("Enter");
  }
}

const whereTheCaretIs = (page: Page) =>
  page.evaluate(() => {
    const range = window.getSelection()!.getRangeAt(0);
    const rect = range.getBoundingClientRect();
    const line =
      rect.height > 0
        ? rect
        : (range.startContainer as Element).getBoundingClientRect();

    return {
      line: line.top + line.height / 2,
      middle: window.innerHeight / 2,
    };
  });

test("typewriter scrolling is off at first, and once on stays on across a reload", async ({
  page,
}) => {
  await aSceneToWriteIn(page, "TypewriterKept");
  await expect(flow(page)).not.toHaveClass(/typewriter/);

  await typewriterScrolling(page, true);
  await page.reload();

  await expect(flow(page)).toHaveClass(/typewriter/);
  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await expect(
    writingSettings(page).getByLabel("Typewriter scrolling"),
  ).toBeChecked();
});

test("with typewriter scrolling on, the line being written stays in the middle of the window", async ({
  page,
}) => {
  const only = await aSceneToWriteIn(page, "TypewriterMiddle");
  await typewriterScrolling(page, true);
  await scene(page, only).click();

  await paragraphs(page, 14);

  await expect
    .poll(async () => {
      const { line, middle } = await whereTheCaretIs(page);
      return Math.abs(line - middle);
    })
    .toBeLessThanOrEqual(20);
});

test("without typewriter scrolling the line being written runs on down the window", async ({
  page,
}) => {
  const only = await aSceneToWriteIn(page, "TypewriterOff");
  await scene(page, only).click();

  await paragraphs(page, 14);

  const { line, middle } = await whereTheCaretIs(page);
  expect(
    line - middle,
    "nothing pulls the line up, so after fourteen paragraphs it is well below the middle",
  ).toBeGreaterThan(100);
});

test("a heading further down comes to the middle once the button is let go, not while it is held", async ({
  page,
}) => {
  await aSceneToWriteIn(page, "TypewriterClicked");
  await page.setViewportSize({ width: 1280, height: 1000 });
  await typewriterScrolling(page, true);
  const first = (await flow(page)
    .getByRole("heading", { name: "Part Two" })
    .boundingBox())!;
  await page.mouse.move(first.x + 20, first.y + 10);
  const before = await page.evaluate(() => window.scrollY);

  await page.mouse.down();
  await page.waitForTimeout(300);
  expect(
    await page.evaluate(() => window.scrollY),
    "the page must not run off under a held button",
  ).toBe(before);
  await page.mouse.up();

  await expect
    .poll(
      async () => {
        const { line, middle } = await whereTheCaretIs(page);
        return Math.abs(line - middle);
      },
      {
        message:
          "even the first line can reach the middle, with room above the book",
      },
    )
    .toBeLessThanOrEqual(20);
});

test("at the top of the book the first heading already sits in the middle, the masthead in view", async ({
  page,
}) => {
  await aSceneToWriteIn(page, "TypewriterTop");
  await page.setViewportSize({ width: 1280, height: 900 });
  await typewriterScrolling(page, true);
  await page.waitForTimeout(300);

  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.waitForTimeout(300);

  expect(
    await page.evaluate(() => window.scrollY),
    "nothing needs to move to put the first line in the middle",
  ).toBeLessThanOrEqual(2);
  await expect(page.getByRole("heading", { level: 1 })).toBeInViewport();
  const { line, middle } = await whereTheCaretIs(page);
  expect(Math.abs(line - middle)).toBeLessThanOrEqual(20);
});

const firstHeadingFromTheTop = (page: Page) =>
  flow(page)
    .getByRole("heading", { name: "Part One" })
    .evaluate((shown) => shown.getBoundingClientRect().top + window.scrollY);

test("the first heading stays in the middle when the window changes size", async ({
  page,
}) => {
  await aSceneToWriteIn(page, "TypewriterResized");
  await page.setViewportSize({ width: 1280, height: 900 });
  await typewriterScrolling(page, true);

  await page.setViewportSize({ width: 1280, height: 700 });
  await page.waitForTimeout(300);

  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.waitForTimeout(300);
  expect(await page.evaluate(() => window.scrollY)).toBeLessThanOrEqual(2);
  const { line, middle } = await whereTheCaretIs(page);
  expect(Math.abs(line - middle)).toBeLessThanOrEqual(20);
});

test("turned off again, the room above the book goes", async ({ page }) => {
  await aSceneToWriteIn(page, "TypewriterTurnedOff");
  const usual = await firstHeadingFromTheTop(page);
  await typewriterScrolling(page, true);
  await expect
    .poll(() => firstHeadingFromTheTop(page))
    .toBeGreaterThan(usual + 50);

  await typewriterScrolling(page, false);

  await expect.poll(() => firstHeadingFromTheTop(page)).toBeCloseTo(usual, 0);
});
