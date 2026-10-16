import { test, expect, type Page } from "@playwright/test";

import {
  aNewProject,
  aSceneMade,
  scenesCatalogued,
  views,
} from "./support/shell";
import { aBookShaped } from "./support/book";

const API = "http://127.0.0.1:3000/api";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

const rail = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

const tray = (page: Page) =>
  page.getByRole("complementary", { name: "This scene" });

const railHandle = (page: Page) =>
  page.getByRole("separator", { name: "Resize the contents" });

const trayHandle = (page: Page) =>
  page.getByRole("separator", { name: "Resize the tray" });

async function aWritingView(page: Page, width = 1440) {
  await page.setViewportSize({ width, height: 800 });
  await aNewProject(page, "Drawers");
  const { outline, ids } = await aBookShaped(page, [{ title: "Part One" }]);
  const scene = await aSceneMade(page);
  const attached = await page.request.post(
    `${API}/outlines/${outline}/attachments`,
    {
      data: {
        attachment: { kind: "scene", id: scene },
        section: ids["Part One"],
        after: null,
      },
    },
  );
  expect(attached.ok()).toBe(true);
  const titled = await page.request.patch(`${API}/scenes/${scene}`, {
    data: { title: "The Letter" },
  });
  expect(titled.ok()).toBe(true);
  await scenesCatalogued(page, [scene]);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();
}

async function openBoth(page: Page) {
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(rail(page)).toBeVisible();
  await expect(tray(page)).toBeVisible();
}

const widthOf = async (page: Page, drawer: ReturnType<typeof rail>) =>
  (await drawer.boundingBox())!.width;

async function drag(
  page: Page,
  handle: ReturnType<typeof railHandle>,
  by: number,
) {
  const at = (await handle.boundingBox())!;
  const x = at.x + at.width / 2;
  const y = at.y + Math.min(at.height / 2, 200);

  await page.mouse.move(x, y);
  await page.mouse.down();
  await page.mouse.move(x + by / 2, y, { steps: 4 });
  await page.mouse.move(x + by, y, { steps: 4 });
  await page.mouse.up();
}

test("each drawer widens by dragging its inner edge, and stays so after a reload", async ({
  page,
}) => {
  await aWritingView(page);
  await openBoth(page);
  expect(await widthOf(page, rail(page))).toBe(320);
  expect(await widthOf(page, tray(page))).toBe(320);

  await drag(page, railHandle(page), 100);
  await drag(page, trayHandle(page), -60);

  expect(await widthOf(page, rail(page))).toBe(420);
  expect(await widthOf(page, tray(page))).toBe(380);
  const tab = (await page
    .getByRole("button", { name: "Contents", exact: true })
    .boundingBox())!;
  expect(tab.x, "the tab follows the edge it hangs from").toBe(420);

  await page.reload();
  await expect(rail(page)).toBeVisible();
  expect(await widthOf(page, rail(page))).toBe(420);
  expect(await widthOf(page, tray(page))).toBe(380);
});

test("a drawer goes no narrower than it can be read", async ({ page }) => {
  await aWritingView(page);
  await openBoth(page);

  await drag(page, railHandle(page), -400);

  expect(await widthOf(page, rail(page))).toBe(224);
  await expect(railHandle(page)).toHaveAttribute("aria-valuenow", "224");
});

test("a drawer is never wider than the window allows, even one kept from a wider window", async ({
  page,
}) => {
  await aWritingView(page);
  await page.evaluate(() =>
    localStorage.setItem("weaveling.book-rail.width", "5000"),
  );
  await page.reload();
  await page.getByRole("button", { name: "Contents", exact: true }).click();

  expect(
    await widthOf(page, rail(page)),
    "the book stays in sight beside it",
  ).toBe(1440 - 160);

  await page.setViewportSize({ width: 1200, height: 800 });
  await expect
    .poll(() => widthOf(page, rail(page)), {
      message: "a window made narrower narrows it too",
    })
    .toBe(1200 - 160);

  await page.setViewportSize({ width: 375, height: 700 });
  await expect
    .poll(() => widthOf(page, rail(page)), {
      message: "on a phone it may take most of the screen, never all of it",
    })
    .toBeCloseTo(375 * 0.85, 0);
});

test("a drawer's edge moves with the arrow keys", async ({ page }) => {
  await aWritingView(page);
  await openBoth(page);

  await railHandle(page).focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await trayHandle(page).focus();
  await page.keyboard.press("ArrowLeft");

  expect(await widthOf(page, rail(page))).toBe(352);
  expect(await widthOf(page, tray(page))).toBe(336);
  await expect(railHandle(page)).toHaveAttribute("aria-valuenow", "352");
});

for (const width of [375, 600]) {
  test(`on a window ${width}px wide, the drawers' tabs keep clear of the text`, async ({
    page,
  }) => {
    await aWritingView(page, width);
    const scene = flow(page).locator(".scene .ProseMirror").first();
    await scene.click();
    await page.keyboard.type(
      "Lorem ipsum dolor sit amet, consetetur sadipscing elitr, sed diam nonumy eirmod tempor invidunt ut labore et dolore magna aliquyam erat, sed diam voluptua.",
    );

    const text = (await scene.boundingBox())!;
    const contents = (await page
      .getByRole("button", { name: "Contents", exact: true })
      .boundingBox())!;
    const thisScene = (await page
      .getByRole("button", { name: "This scene", exact: true })
      .boundingBox())!;

    expect(text.x).toBeGreaterThanOrEqual(contents.x + contents.width);
    expect(text.x + text.width).toBeLessThanOrEqual(thisScene.x);
  });
}

test("where the drawers lie over the book, opening one puts the other away", async ({
  page,
}) => {
  await aWritingView(page, 900);

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(tray(page)).toBeVisible();
  await page.getByRole("button", { name: "Contents", exact: true }).click();

  await expect(rail(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(tray(page)).toBeVisible();
  await expect(rail(page)).toBeHidden();
});

test("beside a wide book both drawers stay open, and the one opened last gives way", async ({
  page,
}) => {
  await aWritingView(page, 1440);
  await page.evaluate(() => {
    localStorage.setItem("weaveling.book-rail.width", "800");
    localStorage.setItem("weaveling.scene-tray.width", "800");
  });
  await page.reload();
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await page.getByRole("button", { name: "This scene", exact: true }).click();

  await expect.poll(() => widthOf(page, tray(page))).toBe(1440 - 160 - 800);
  const left = (await rail(page).boundingBox())!;
  const right = (await tray(page).boundingBox())!;
  expect(left.x + left.width, "the drawers never overlap").toBeLessThanOrEqual(
    right.x - 160,
  );
  await expect(
    page.getByRole("button", { name: "This scene", exact: true }),
    "the tab hangs from its own drawer",
  ).toBeInViewport();
  const tab = (await page
    .getByRole("button", { name: "This scene", exact: true })
    .boundingBox())!;
  expect(tab.x + tab.width).toBeLessThanOrEqual(right.x + 1);
  expect(tab.x).toBeGreaterThanOrEqual(left.x + left.width);

  await page.setViewportSize({ width: 1200, height: 800 });
  await expect
    .poll(() => widthOf(page, tray(page)), {
      message: "a narrower window takes from the one that gives way",
    })
    .toBe(1200 - 160 - 800);
});

test("the drawers lie over the book and move nothing in it", async ({
  page,
}) => {
  await aWritingView(page, 1440);
  const before = (await flow(page).boundingBox())!;

  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await expect(rail(page)).toBeVisible();
  expect((await flow(page).boundingBox())!).toEqual(before);

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(tray(page)).toBeVisible();
  expect((await flow(page).boundingBox())!).toEqual(before);
});
