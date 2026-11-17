import { test, expect, type Page } from "@playwright/test";

import { aNewProject, views } from "./support/shell";
import { openTheWritingView } from "./support/writing";

const book = (page: Page) => page.getByRole("article");

const contents = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

const contentsTab = (page: Page) =>
  page.locator('.drawer-tab[aria-controls="reading-contents"]');

async function openTheReadingView(page: Page) {
  await views(page).getByRole("link", { name: "Read", exact: true }).click();
  await expect(book(page)).toBeVisible();
}

test("the reading view shows the book with its sections and scenes", async ({
  page,
}) => {
  // given: a project
  await aNewProject(page, "Reading");

  // when: its reading view is opened
  await openTheReadingView(page);

  // then: the book is shown under its title
  await expect(
    book(page).getByRole("heading", { level: 1, name: "The Loom" }),
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
    "The boats came in before the light did, one by one, their lamps swinging.",
  );
  await expect(book(page).locator("em")).toHaveText("one by one");
  await expect(book(page).locator("strong")).toHaveText(
    "Everybody bought the fish.",
  );
  // and: a break stands between two scenes and where a scene breaks itself
  await expect(book(page).getByRole("separator")).toHaveCount(2);
  // and: the Read tab is the one shown
  await expect(
    views(page).getByRole("link", { name: "Read", exact: true }),
  ).toHaveClass(/here/);
});

test("the contents name only the sections, and lead to them", async ({
  page,
}) => {
  // given: a project's reading view in a short window
  await page.setViewportSize({ width: 1200, height: 500 });
  await aNewProject(page, "ReadingContents");
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
