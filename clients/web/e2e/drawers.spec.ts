import { test, expect, type Page } from "@playwright/test";

import {
  aNewProject,
  API,
  aSceneMade,
  scenesCatalogued,
  views,
} from "./support/shell";
import { aBookShaped } from "./support/book";
import { flow, rail } from "./support/writing";

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
  // given: both drawers open at their first width
  await aWritingView(page);
  await openBoth(page);
  expect(await widthOf(page, rail(page))).toBe(320);
  expect(await widthOf(page, tray(page))).toBe(320);

  // when: each is dragged wider by its inner edge
  await drag(page, railHandle(page), 100);
  await drag(page, trayHandle(page), -60);

  // then: each is wider, its tab following
  expect(await widthOf(page, rail(page))).toBe(420);
  expect(await widthOf(page, tray(page))).toBe(380);
  const tab = (await page
    .getByRole("button", { name: "Contents", exact: true })
    .boundingBox())!;
  expect(tab.x, "the tab follows the edge it hangs from").toBe(420);

  // and: stays so after a reload
  await page.reload();
  await expect(rail(page)).toBeVisible();
  expect(await widthOf(page, rail(page))).toBe(420);
  expect(await widthOf(page, tray(page))).toBe(380);
});

test("a drawer goes no narrower than it can be read", async ({ page }) => {
  // given: both drawers open
  await aWritingView(page);
  await openBoth(page);

  // when: one is dragged far narrower
  await drag(page, railHandle(page), -400);

  // then: it stops at the narrowest it can be read
  expect(await widthOf(page, rail(page))).toBe(224);
  await expect(railHandle(page)).toHaveAttribute("aria-valuenow", "224");
});

test("a drawer is never wider than the window allows, even one kept from a wider window", async ({
  page,
}) => {
  // given: a drawer width kept from a far wider window
  await aWritingView(page);
  await page.evaluate(() =>
    localStorage.setItem("weaveling.book-rail.width", "5000"),
  );
  await page.reload();
  // when: the drawer is opened
  await page.getByRole("button", { name: "Contents", exact: true }).click();

  // then: it leaves the book in sight beside it
  expect(
    await widthOf(page, rail(page)),
    "the book stays in sight beside it",
  ).toBe(1440 - 160);

  // when: the window is made narrower
  await page.setViewportSize({ width: 1200, height: 800 });
  // then: the drawer narrows too
  await expect
    .poll(() => widthOf(page, rail(page)), {
      message: "a window made narrower narrows it too",
    })
    .toBe(1200 - 160);

  // when: the window is phone-sized
  await page.setViewportSize({ width: 375, height: 700 });
  // then: the drawer takes most of it, never all
  await expect
    .poll(() => widthOf(page, rail(page)), {
      message: "on a phone it may take most of the screen, never all of it",
    })
    .toBeCloseTo(375 * 0.85, 0);
});

test("a drawer's edge moves with the arrow keys", async ({ page }) => {
  // given: both drawers open
  await aWritingView(page);
  await openBoth(page);

  // when: their edges are moved with the arrow keys
  await railHandle(page).focus();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowRight");
  await trayHandle(page).focus();
  await page.keyboard.press("ArrowLeft");

  // then: they widen and narrow by steps
  expect(await widthOf(page, rail(page))).toBe(352);
  expect(await widthOf(page, tray(page))).toBe(336);
  await expect(railHandle(page)).toHaveAttribute("aria-valuenow", "352");
});

test("on a phone, the drawers open from the status bar, with no tabs over the book", async ({
  page,
}) => {
  // given: a phone
  await aWritingView(page, 375);

  // then: no drawer tabs lie over the book, the buttons sit in the status bar
  await expect(page.locator(".book-rail-tab")).toBeHidden();
  await expect(page.locator(".scene-tray-tab")).toBeHidden();
  const status = page.getByRole("contentinfo", { name: "Status" });
  const contents = page.getByRole("button", { name: "Contents", exact: true });
  const thisScene = page.getByRole("button", {
    name: "This scene",
    exact: true,
  });
  const bar = (await status.boundingBox())!;
  for (const button of [contents, thisScene]) {
    const seen = (await button.boundingBox())!;
    expect(seen.y, "it sits in the status bar").toBeGreaterThanOrEqual(bar.y);
  }

  // when: the contents are opened
  await contents.click();
  // then: they show
  await expect(rail(page)).toBeVisible();
  await expect(contents).toHaveAttribute("aria-expanded", "true");

  // when: the scene's drawer is opened
  await thisScene.click();
  // then: it shows and the contents are put away
  await expect(tray(page)).toBeVisible();
  await expect(rail(page), "a phone holds one drawer at a time").toBeHidden();

  // when: its button is pressed again
  await thisScene.click();
  // then: it is put away
  await expect(tray(page), "the same button puts it away").toBeHidden();
});

test("where the drawers lie over the book, opening one puts the other away", async ({
  page,
}) => {
  // given: a window where the drawers lie over the book
  await aWritingView(page, 900);

  // and: the scene's drawer open
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(tray(page)).toBeVisible();
  // when: the contents are opened
  await page.getByRole("button", { name: "Contents", exact: true }).click();

  // then: they show and the scene's drawer is put away
  await expect(rail(page)).toBeVisible();
  await expect(tray(page)).toBeHidden();

  // when: the scene's drawer is opened again
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  // then: it shows and the contents are put away
  await expect(tray(page)).toBeVisible();
  await expect(rail(page)).toBeHidden();
});

test("beside a wide book both drawers stay open, and the one opened last gives way", async ({
  page,
}) => {
  // given: a wide window and drawers kept wide
  await aWritingView(page, 1440);
  await page.evaluate(() => {
    localStorage.setItem("weaveling.book-rail.width", "800");
    localStorage.setItem("weaveling.inspector.width", "800");
  });
  await page.reload();
  // when: both are opened
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  await page.getByRole("button", { name: "This scene", exact: true }).click();

  // then: the one opened last gives way, neither overlapping
  await expect.poll(() => widthOf(page, tray(page))).toBe(1440 - 160 - 800);
  const left = (await rail(page).boundingBox())!;
  const right = (await tray(page).boundingBox())!;
  expect(left.x + left.width, "the drawers never overlap").toBeLessThanOrEqual(
    right.x - 160,
  );
  // and: its tab hangs from it
  await expect(
    page.getByRole("button", { name: "This scene", exact: true }),
    "the tab hangs from its own drawer",
  ).toBeInViewport();
  const tab = (await page
    .getByRole("button", { name: "This scene", exact: true })
    .boundingBox())!;
  expect(tab.x + tab.width).toBeLessThanOrEqual(right.x + 1);
  expect(tab.x).toBeGreaterThanOrEqual(left.x + left.width);

  // when: the window is made narrower
  await page.setViewportSize({ width: 1200, height: 800 });
  // then: the one opened last gives way further
  await expect
    .poll(() => widthOf(page, tray(page)), {
      message: "a narrower window takes from the one that gives way",
    })
    .toBe(1200 - 160 - 800);
});

test("the drawers lie over the book and move nothing in it", async ({
  page,
}) => {
  // given: a book open in a wide window
  await aWritingView(page, 1440);
  const before = (await flow(page).boundingBox())!;

  // when: the contents are opened
  await page.getByRole("button", { name: "Contents", exact: true }).click();
  // then: the book does not move
  await expect(rail(page)).toBeVisible();
  expect((await flow(page).boundingBox())!).toEqual(before);

  // when: the scene's drawer is opened
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  // then: the book does not move
  await expect(tray(page)).toBeVisible();
  expect((await flow(page).boundingBox())!).toEqual(before);
});
