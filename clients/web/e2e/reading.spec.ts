import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import {
  aNewProject,
  API,
  aSceneMade,
  scenesCatalogued,
  views,
} from "./support/shell";
import { openTheWritingView, projectOf, scene } from "./support/writing";

const book = (page: Page) => page.getByRole("article");

const contents = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

const contentsTab = (page: Page) =>
  page.locator('.drawer-tab[aria-controls="reading-contents"]');

async function openTheReadingView(page: Page) {
  await views(page).getByRole("link", { name: "Read", exact: true }).click();
  await expect(book(page)).toBeVisible();
}

/// A chapter with two scenes, and a part under it with one, each scene with nothing written yet.
async function aBookOfThreeScenes(page: Page) {
  const { outline, ids } = await aBookShaped(page, [
    { title: "Morning" },
    { title: "The market", under: "Morning" },
  ]);
  const placed = { harbour: "", wall: "", stalls: "" };
  for (const [named, section, behind] of [
    ["harbour", "Morning", null],
    ["wall", "Morning", "harbour"],
    ["stalls", "The market", null],
  ] as const) {
    placed[named] = await aSceneMade(page);
    const attached = await page.request.post(
      `${API}/outlines/${outline}/attachments`,
      {
        data: {
          attachment: { kind: "scene", id: placed[named] },
          section: ids[section],
          after: behind ? { kind: "scene", id: placed[behind] } : null,
        },
      },
    );
    expect(attached.ok(), await attached.text()).toBe(true);
  }
  await scenesCatalogued(page, Object.values(placed));

  return placed;
}

/// Waits until the readings have been told every one of these words.
/// A scene first tells its text as it opens, still empty, so counting scenes is not enough.
async function toldToTheReadings(page: Page, words: string[]) {
  await expect
    .poll(
      async () => {
        const live = await page.request.get(
          `${API}/readings/live?project=${projectOf(page)}`,
        );
        const told = live.ok() ? JSON.stringify(await live.json()) : "";

        return words.filter((said) => !told.includes(said));
      },
      {
        message:
          "a live scene tells its text every couple of seconds, and the readings keep it from the message",
        timeout: 20_000,
      },
    )
    .toEqual([]);
}

/// Waits until the readings have heard of the project, so its book is there to be read.
async function readable(page: Page) {
  await expect
    .poll(
      async () =>
        (
          await page.request.get(
            `${API}/readings/live?project=${projectOf(page)}`,
          )
        ).status(),
      { message: "the readings learn of a project from its start" },
    )
    .toBe(200);
}

/// A book of three scenes, written in the writing view: one with words in italics, one plain, and one in bold.
async function aBookWritten(page: Page, named: string) {
  const title = await aNewProject(page, named);
  const placed = await aBookOfThreeScenes(page);
  await openTheWritingView(page);

  await scene(page, placed.harbour).click();
  await page.keyboard.type("The boats came in ");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type("one by one");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type(", their lamps swinging.");
  await scene(page, placed.wall).click();
  await page.keyboard.type("Hedy counted them from the wall.");
  await scene(page, placed.stalls).click();
  await page.keyboard.press("ControlOrMeta+B");
  await page.keyboard.type("Everybody bought the fish.");
  await toldToTheReadings(page, [
    "one by one",
    "Hedy counted them from the wall.",
    "Everybody bought the fish.",
  ]);

  return title;
}

test("the reading view shows the book as it was written, with its sections and scenes", async ({
  page,
}) => {
  // given: a book written in the writing view
  const title = await aBookWritten(page, "Reading");

  // when: its reading view is opened
  await openTheReadingView(page);

  // then: the book is shown under its project's name
  await expect(
    book(page).getByRole("heading", { level: 1, name: title }),
  ).toBeVisible();
  // and: its sections are headings at their depth
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Morning" }),
  ).toBeVisible();
  await expect(
    book(page).getByRole("heading", { level: 3, name: "The market" }),
  ).toBeVisible();
  // and: its scenes are shown with their marks
  await expect(book(page)).toContainText(
    "The boats came in one by one, their lamps swinging.",
  );
  await expect(book(page).locator("em")).toHaveText("one by one");
  await expect(book(page).locator("strong")).toHaveText(
    "Everybody bought the fish.",
  );
  // and: a break stands between the two scenes of the chapter, and nowhere else
  await expect(book(page).getByRole("separator")).toHaveCount(1);
  // and: the Read tab is the one shown
  await expect(
    views(page).getByRole("link", { name: "Read", exact: true }),
  ).toHaveClass(/here/);
});

test("the contents name only the sections, and lead to them", async ({
  page,
}) => {
  // given: a written book's reading view in a short window
  await page.setViewportSize({ width: 1200, height: 500 });
  await aBookWritten(page, "ReadingContents");
  await openTheReadingView(page);

  // expect: the contents list the sections and no scene
  await expect(contents(page).locator(".rail-entry")).toHaveText([
    "Morning",
    "The market",
  ]);

  // when: a section is chosen in the contents
  await contents(page).getByRole("button", { name: "The market" }).click();

  // then: its heading is brought into view
  await expect(
    book(page).getByRole("heading", { name: "The market" }),
  ).toBeInViewport();
});

test("the contents are open at first, and stay as they were left apart from the writing view's", async ({
  page,
}) => {
  // given: a project's reading view opened for the first time
  await aNewProject(page, "ReadingDrawer");
  await aBookShaped(page, [{ title: "Morning" }]);
  await readable(page);
  await openTheReadingView(page);

  // expect: its contents are open
  await expect(contents(page)).toBeVisible();

  // when: they are closed and the view is opened again
  await contentsTab(page).click();
  await expect(contents(page)).toBeHidden();
  await page.reload();
  await expect(book(page)).toBeVisible();

  // then: they are still closed
  await expect(contents(page)).toBeHidden();
  await expect(contentsTab(page)).toHaveAttribute("aria-expanded", "false");

  // when: the writing view's contents are opened, and the reading view is opened again
  await openTheWritingView(page);
  await page.locator('.drawer-tab[aria-controls="book-rail"]').click();
  await openTheReadingView(page);

  // then: the reading view's contents are still closed
  await expect(contents(page)).toBeHidden();
});

test("a project whose outline was never opened reads as its name alone", async ({
  page,
}) => {
  // given: a new project, its book never shaped
  const title = await aNewProject(page, "ReadingEmpty");
  await readable(page);

  // when: its reading view is opened
  await openTheReadingView(page);

  // then: the book shows its name and nothing else
  await expect(
    book(page).getByRole("heading", { level: 1, name: title }),
  ).toBeVisible();
  await expect(book(page).getByRole("heading", { level: 2 })).toHaveCount(0);
});
