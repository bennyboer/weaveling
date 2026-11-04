import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, aSceneMade, scenesCatalogued } from "./support/shell";
import { aBookShaped } from "./support/book";
import { flow, openTheWritingView, rail } from "./support/writing";

const tab = (page: Page) =>
  page.getByRole("button", { name: "Contents", exact: true });

const entry = (page: Page, named: string) =>
  rail(page).getByRole("button", { name: named, exact: true });

const shown = (page: Page) =>
  rail(page)
    .locator("li")
    .evaluateAll((rows) =>
      rows.map(
        (row) =>
          `${(row as HTMLElement).style.getPropertyValue("--depth")} ${row.querySelector(".rail-entry")!.textContent}`,
      ),
    );

async function titled(page: Page, scene: string, title: string) {
  const renamed = await page.request.patch(`${API}/scenes/${scene}`, {
    data: { title },
  });
  expect(renamed.ok()).toBe(true);
}

async function aBookInParts(
  page: Page,
  scenesIn: Record<string, string[]>,
): Promise<Record<string, string>> {
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "The Workshop", under: "Part One" },
  ]);
  const scenes: Record<string, string> = {};

  for (const [section, titles] of Object.entries(scenesIn)) {
    let behind: string | undefined;
    for (const title of titles) {
      const scene = await aSceneMade(page);
      await titled(page, scene, title);
      const attached = await page.request.post(
        `${API}/outlines/${outline}/attachments`,
        {
          data: {
            attachment: { kind: "scene", id: scene },
            section: ids[section],
            after: behind ? { kind: "scene", id: behind } : null,
          },
        },
      );
      expect(attached.ok(), await attached.text()).toBe(true);
      behind = scene;
      scenes[title] = scene;
    }
  }
  await scenesCatalogued(page, Object.values(scenes));

  return scenes;
}

async function openTheContents(page: Page) {
  await tab(page).click();
  await expect(rail(page)).toBeVisible();
}

test("the contents show the book's sections, nested, with their scenes folded away", async ({
  page,
}) => {
  // given: a book in nested sections, open in the writing view
  await aNewProject(page, "Contents");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "The Workshop": ["Sawdust"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);

  // and: the contents closed
  await expect(rail(page)).toBeHidden();
  // when: the contents are opened
  await openTheContents(page);

  // then: they show the sections nested, their scenes folded away
  await expect
    .poll(() => shown(page))
    .toEqual(["0 Part One", "1 The Workshop", "0 Part Two"]);
});

test("a section unfolds to its scenes, and folds them away again", async ({
  page,
}) => {
  // given: a book in nested sections with its contents open
  await aNewProject(page, "Unfolding");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "The Workshop": ["Sawdust", "Shavings"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: a section is unfolded
  await rail(page).getByRole("button", { name: "Unfold The Workshop" }).click();
  // then: its scenes show
  await expect
    .poll(() => shown(page))
    .toEqual([
      "0 Part One",
      "1 The Workshop",
      "2 Sawdust",
      "2 Shavings",
      "0 Part Two",
    ]);

  // when: the section above it is folded
  await rail(page).getByRole("button", { name: "Fold Part One" }).click();
  // then: all within it are folded away
  await expect.poll(() => shown(page)).toEqual(["0 Part One", "0 Part Two"]);
});

test("everything unfolds and folds at once, each only when it would change something", async ({
  page,
}) => {
  // given: a book in nested sections with its contents open
  await aNewProject(page, "Everything");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "The Workshop": ["Sawdust"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);
  const unfoldAll = rail(page).getByRole("button", {
    name: "Unfold all",
    exact: true,
  });
  const foldAll = rail(page).getByRole("button", {
    name: "Fold all",
    exact: true,
  });

  // then: both unfolding and folding all are offered
  await expect(unfoldAll).toBeEnabled();
  await expect(foldAll, "the nested sections are open at first").toBeEnabled();
  await expect(unfoldAll).toHaveAttribute("title", "Unfold all");
  await expect(foldAll).toHaveAttribute("title", "Fold all");

  // when: all is unfolded
  await unfoldAll.click();
  // then: every section and scene shows
  await expect
    .poll(() => shown(page))
    .toEqual([
      "0 Part One",
      "1 The Letter",
      "1 The Workshop",
      "2 Sawdust",
      "0 Part Two",
      "1 The Ferry",
    ]);
  // and: only folding is still offered
  await expect(unfoldAll, "nothing is left to unfold").toBeDisabled();
  await expect(foldAll).toBeEnabled();

  // when: all is folded
  await foldAll.click();
  // then: only the top sections show
  await expect.poll(() => shown(page)).toEqual(["0 Part One", "0 Part Two"]);
  // and: only unfolding is still offered
  await expect(foldAll, "nothing is left to fold").toBeDisabled();
  await expect(unfoldAll).toBeEnabled();
});

test("choosing a scene goes there, however far down the book it is", async ({
  page,
}) => {
  // given: a long book with its contents open
  await aNewProject(page, "FarDown");
  const many = Array.from({ length: 12 }, (_, nth) => `Early ${nth + 1}`);
  const scenes = await aBookInParts(page, {
    "Part One": many,
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: a scene far down is chosen
  await rail(page).getByRole("button", { name: "Unfold Part Two" }).click();
  await entry(page, "The Ferry").click();

  // then: it is in view, ready to write in
  const ferry = flow(page).locator(
    `[data-scene="${scenes["The Ferry"]}"] .ProseMirror`,
  );
  await expect(ferry).toBeFocused();
  await expect(ferry).toBeInViewport();
});

test("choosing a section goes to its heading", async ({ page }) => {
  // given: a long book with its contents open
  await aNewProject(page, "ToHeading");
  await aBookInParts(page, {
    "Part One": Array.from({ length: 12 }, (_, nth) => `Early ${nth + 1}`),
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: a section far down is chosen
  await entry(page, "Part Two").click();

  // then: its heading is in view and focused
  const heading = flow(page).getByRole("heading", { name: "Part Two" });
  await expect(heading).toBeFocused();
  await expect(heading).toBeInViewport();
});

test("the contents stay open, or closed, as they were left", async ({
  page,
}) => {
  // given: a book with its contents open
  await aNewProject(page, "Kept");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: the page is reloaded
  await page.reload();
  // then: the contents are still open
  await expect(rail(page)).toBeVisible();

  // when: they are closed and the page reloaded
  await rail(page).getByRole("button", { name: "Close the contents" }).click();
  await page.reload();
  // then: they stay closed
  await expect(flow(page)).toBeVisible();
  await expect(rail(page)).toBeHidden();
});

test("Escape inside the contents closes them", async ({ page }) => {
  // given: a book with its contents open
  await aNewProject(page, "Escaping");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: escape is pressed inside them
  await entry(page, "Part One").focus();
  await page.keyboard.press("Escape");

  // then: they close
  await expect(rail(page)).toBeHidden();
});

test("on a narrow window, choosing an entry puts the contents away", async ({
  page,
}) => {
  // given: a narrow window
  await page.setViewportSize({ width: 700, height: 900 });
  // and: a book with its contents open
  await aNewProject(page, "Narrow");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  // when: a section is chosen
  await entry(page, "Part Two").click();

  // then: the contents are put away
  await expect(rail(page)).toBeHidden();
  // and: the section's heading is focused
  await expect(
    flow(page).getByRole("heading", { name: "Part Two" }),
  ).toBeFocused();
});

test("the scenes not in the book have no contents", async ({ page }) => {
  // given: a book open in the writing view
  await aNewProject(page, "Loose");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await expect(tab(page)).toBeVisible();

  // when: the scenes not in the book are opened
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));

  // then: they offer no contents
  await expect(flow(page)).toBeVisible();
  await expect(tab(page)).toHaveCount(0);
  await expect(rail(page)).toHaveCount(0);
});

test("a section without a title is named untitled in the contents", async ({
  page,
}) => {
  // given: a book with a section without a title
  await aNewProject(page, "ContentsUntitled");
  await aBookShaped(page, [{ title: "" }]);
  await openTheWritingView(page);

  // when: the contents are opened
  await openTheContents(page);

  // then: the section is named untitled
  await expect.poll(() => shown(page)).toEqual(["0 Untitled section"]);
});

