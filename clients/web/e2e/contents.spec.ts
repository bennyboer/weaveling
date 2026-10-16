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

const tab = (page: Page) =>
  page.getByRole("button", { name: "Contents", exact: true });

const rail = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

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

async function openTheWritingView(page: Page) {
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();
}

async function openTheContents(page: Page) {
  await tab(page).click();
  await expect(rail(page)).toBeVisible();
}

test("the contents show the book's sections, nested, with their scenes folded away", async ({
  page,
}) => {
  await aNewProject(page, "Contents");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "The Workshop": ["Sawdust"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);

  await expect(rail(page)).toBeHidden();
  await openTheContents(page);

  await expect
    .poll(() => shown(page))
    .toEqual(["0 Part One", "1 The Workshop", "0 Part Two"]);
});

test("a section unfolds to its scenes, and folds them away again", async ({
  page,
}) => {
  await aNewProject(page, "Unfolding");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "The Workshop": ["Sawdust", "Shavings"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  await rail(page).getByRole("button", { name: "Unfold The Workshop" }).click();
  await expect
    .poll(() => shown(page))
    .toEqual([
      "0 Part One",
      "1 The Workshop",
      "2 Sawdust",
      "2 Shavings",
      "0 Part Two",
    ]);

  await rail(page).getByRole("button", { name: "Fold Part One" }).click();
  await expect.poll(() => shown(page)).toEqual(["0 Part One", "0 Part Two"]);
});

test("everything unfolds and folds at once, each only when it would change something", async ({
  page,
}) => {
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

  await expect(unfoldAll).toBeEnabled();
  await expect(foldAll, "the nested sections are open at first").toBeEnabled();
  await expect(unfoldAll).toHaveAttribute("title", "Unfold all");
  await expect(foldAll).toHaveAttribute("title", "Fold all");

  await unfoldAll.click();
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
  await expect(unfoldAll, "nothing is left to unfold").toBeDisabled();
  await expect(foldAll).toBeEnabled();

  await foldAll.click();
  await expect.poll(() => shown(page)).toEqual(["0 Part One", "0 Part Two"]);
  await expect(foldAll, "nothing is left to fold").toBeDisabled();
  await expect(unfoldAll).toBeEnabled();
});

test("choosing a scene goes there, however far down the book it is", async ({
  page,
}) => {
  await aNewProject(page, "FarDown");
  const many = Array.from({ length: 12 }, (_, nth) => `Early ${nth + 1}`);
  const scenes = await aBookInParts(page, {
    "Part One": many,
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  await rail(page).getByRole("button", { name: "Unfold Part Two" }).click();
  await entry(page, "The Ferry").click();

  const ferry = flow(page).locator(
    `[data-scene="${scenes["The Ferry"]}"] .ProseMirror`,
  );
  await expect(ferry).toBeFocused();
  await expect(ferry).toBeInViewport();
});

test("choosing a section goes to its heading", async ({ page }) => {
  await aNewProject(page, "ToHeading");
  await aBookInParts(page, {
    "Part One": Array.from({ length: 12 }, (_, nth) => `Early ${nth + 1}`),
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  await entry(page, "Part Two").click();

  const heading = flow(page).getByRole("heading", { name: "Part Two" });
  await expect(heading).toBeFocused();
  await expect(heading).toBeInViewport();
});

test("the contents stay open, or closed, as they were left", async ({
  page,
}) => {
  await aNewProject(page, "Kept");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await openTheContents(page);

  await page.reload();
  await expect(rail(page)).toBeVisible();

  await rail(page).getByRole("button", { name: "Close the contents" }).click();
  await page.reload();
  await expect(flow(page)).toBeVisible();
  await expect(rail(page)).toBeHidden();
});

test("Escape inside the contents closes them", async ({ page }) => {
  await aNewProject(page, "Escaping");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await openTheContents(page);

  await entry(page, "Part One").focus();
  await page.keyboard.press("Escape");

  await expect(rail(page)).toBeHidden();
});

test("on a narrow window, choosing an entry puts the contents away", async ({
  page,
}) => {
  await page.setViewportSize({ width: 700, height: 900 });
  await aNewProject(page, "Narrow");
  await aBookInParts(page, {
    "Part One": ["The Letter"],
    "Part Two": ["The Ferry"],
  });
  await openTheWritingView(page);
  await openTheContents(page);

  await entry(page, "Part Two").click();

  await expect(rail(page)).toBeHidden();
  await expect(
    flow(page).getByRole("heading", { name: "Part Two" }),
  ).toBeFocused();
});

test("the scenes not in the book have no contents", async ({ page }) => {
  await aNewProject(page, "Loose");
  await aBookInParts(page, { "Part One": ["The Letter"] });
  await openTheWritingView(page);
  await expect(tab(page)).toBeVisible();

  await page.goto(page.url().replace(/\/write$/, "/write/loose"));

  await expect(flow(page)).toBeVisible();
  await expect(tab(page)).toHaveCount(0);
  await expect(rail(page)).toHaveCount(0);
});
