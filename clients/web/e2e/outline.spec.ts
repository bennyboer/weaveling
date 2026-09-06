import { test, expect, type Page } from "@playwright/test";

import {
  aNewProject,
  capture,
  openTheOutline,
  openThePool,
} from "./support/shell";

const manuscript = (page: Page) =>
  page.getByRole("list", { name: "The manuscript" });

const waiting = (page: Page) =>
  page.getByRole("list", { name: "Pieces not in the book" });

const titles = (page: Page) => page.locator(".branch .title");

async function place(page: Page, piece: string, into: string) {
  await page.getByRole("button", { name: `Place ${piece}` }).click();
  await page.getByRole("textbox", { name: `Section ${into}` }).click();
}

const shape = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll(".branch")]
      .map((branch) => {
        let depth = 0;
        for (let up = branch.parentElement; up; up = up.parentElement) {
          if (up.classList.contains("twigs")) depth += 1;
        }
        const title = branch.querySelector<HTMLInputElement>(".title")!.value;

        return `${"  ".repeat(depth)}${title}`;
      })
      .join("\n"),
  );

async function aBookOf(page: Page, sections: string[]) {
  await page.getByRole("button", { name: "Add a section" }).click();

  for (const [nth, named] of sections.entries()) {
    await expect(titles(page)).toHaveCount(nth + 1);
    await page.keyboard.type(named);

    if (nth < sections.length - 1) {
      await page.keyboard.press("Enter");
    }
  }
}

test("the switcher offers the outline beside the board and the pool", async ({
  page,
}) => {
  await aNewProject(page, "Switching");

  await openTheOutline(page);

  await expect(page).toHaveURL(/\/projects\/.+\/outline$/);
  await expect(
    page.getByRole("navigation", { name: "Views" }).getByRole("link", {
      name: "Outline",
      exact: true,
    }),
  ).toHaveClass(/here/);
});

test("a book starts with nothing in it", async ({ page }) => {
  await aNewProject(page, "Empty");

  await openTheOutline(page);

  await expect(
    page.getByText("Nothing in the book yet. Add a section to begin."),
  ).toBeVisible();
  await expect(titles(page)).toHaveCount(0);
});

test("the add button appends rather than pushing in at the top", async ({
  page,
}) => {
  await aNewProject(page, "Appending");
  await openTheOutline(page);

  for (const named of ["Kapitel 1", "Kapitel 2", "Kapitel 3"]) {
    await page.getByRole("button", { name: "Add a section" }).click();
    await expect(titles(page).last()).toHaveValue("");
    await page.keyboard.type(named);
    await page.keyboard.press("Escape");
  }

  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "Kapitel 2", "Kapitel 3"].join("\n"));
});

test("a section can be moved among its siblings", async ({ page }) => {
  await aNewProject(page, "Reordering");
  await openTheOutline(page);
  await aBookOf(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  await page.getByRole("textbox", { name: "Section Kapitel 3" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "Kapitel 3", "Kapitel 2"].join("\n"));

  await page.getByRole("button", { name: "Move later Kapitel 1" }).click();
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 3", "Kapitel 1", "Kapitel 2"].join("\n"));
});

test("reordering moves sections without rewriting their titles", async ({
  page,
}) => {
  await aNewProject(page, "Intact");
  await openTheOutline(page);
  await aBookOf(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  await page.getByRole("textbox", { name: "Section Kapitel 3" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  await expect.poll(() => shape(page)).toContain("Kapitel 2");

  await page.reload();
  await expect(titles(page)).toHaveCount(3);

  const kept = await titles(page).evaluateAll((all) =>
    (all as HTMLInputElement[]).map((it) => it.value).sort(),
  );
  expect(
    kept,
    "a reorder must not let one section take another one title",
  ).toEqual(["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
});

test("moving is refused at the ends rather than doing nothing", async ({
  page,
}) => {
  await aNewProject(page, "Ends");
  await openTheOutline(page);
  await aBookOf(page, ["Kapitel 1", "Kapitel 2"]);
  await page.keyboard.press("Escape");

  await expect(
    page.getByRole("button", { name: "Move earlier Kapitel 1" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Move later Kapitel 2" }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", { name: "Promote Kapitel 1" }),
  ).toBeDisabled();
});

test("Enter adds a sibling and Tab nests it under the one before", async ({
  page,
}) => {
  await aNewProject(page, "Nesting");
  await openTheOutline(page);

  await aBookOf(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect(manuscript(page).getByRole("textbox")).toHaveCount(2);

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("Shift+Tab lifts a section back out", async ({ page }) => {
  await aNewProject(page, "Lifting");
  await openTheOutline(page);
  await aBookOf(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");

  await page.keyboard.press("Shift+Tab");

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));
});

test("typing carries on in the same section after it is nested", async ({
  page,
}) => {
  await aNewProject(page, "Carrying");
  await openTheOutline(page);
  await aBookOf(page, ["Part One", "Chapter"]);

  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter");
  await page.keyboard.type(" 1");

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a title survives a reload", async ({ page }) => {
  await aNewProject(page, "Titling");
  await openTheOutline(page);
  await aBookOf(page, ["The Silent Loom"]);
  await page.keyboard.press("Escape");

  await page.reload();

  await expect(titles(page).first()).toHaveValue("The Silent Loom");
});

test("a section with nothing in it is flagged without being refused", async ({
  page,
}) => {
  await aNewProject(page, "Holes");
  await openTheOutline(page);

  await aBookOf(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await expect(page.locator(".hollow")).toHaveCount(1);
  await expect(titles(page).first()).toHaveValue("Chapter 1");
});

test("a piece is placed from the tray and leaves it", async ({ page }) => {
  await aNewProject(page, "Placing");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheOutline(page);
  await aBookOf(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await place(page, "The loom remembers", "Chapter 1");

  await expect(page.locator(".leaf .name")).toHaveText("The loom remembers");
  await expect(waiting(page).getByRole("button")).toHaveCount(0);
  await expect(page.locator(".hollow")).toHaveCount(0);
});

test("a piece taken out of the book goes back to the tray", async ({
  page,
}) => {
  await aNewProject(page, "Removing");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheOutline(page);
  await aBookOf(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await place(page, "The loom remembers", "Chapter 1");
  await expect(waiting(page).getByRole("button")).toHaveCount(0);

  await page
    .getByRole("button", { name: "Take The loom remembers out of the book" })
    .click();

  await expect(waiting(page).getByRole("button")).toHaveCount(1);
});

test("the row menu promotes, demotes and removes", async ({ page }) => {
  await aNewProject(page, "Menus");
  await openTheOutline(page);
  await aBookOf(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Demote Chapter 1" }).click();
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));

  await page.getByRole("button", { name: "Promote Chapter 1" }).click();
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));

  await page.getByRole("button", { name: "Remove Part One" }).click();
  await expect.poll(() => shape(page)).toBe("Chapter 1");
});

test("a piece can be dragged from the rail onto a section", async ({
  page,
}) => {
  await aNewProject(page, "Dragging");
  await openThePool(page);
  await capture(page, "The loom remembers");
  await openTheOutline(page);
  await aBookOf(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  const chip = page.getByRole("button", { name: "Place The loom remembers" });
  const onto = page.getByRole("textbox", { name: "Section Chapter 1" });
  const from = (await chip.boundingBox())!;
  const to = (await onto.boundingBox())!;

  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 40, to.y + to.height / 2, { steps: 10 });
  await expect(page.locator(".row.landing")).toHaveCount(1);
  await page.mouse.up();

  await expect(page.locator(".leaf .name")).toHaveText("The loom remembers");
  await expect(waiting(page).getByRole("button")).toHaveCount(0);
});

test("folding a section hides what is inside it without changing the book", async ({
  page,
}) => {
  await aNewProject(page, "Folding");
  await openTheOutline(page);
  await aBookOf(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Fold Part One" }).click();

  await expect(titles(page)).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Fold Part One" }),
  ).toHaveAttribute("aria-expanded", "false");

  await page.reload();

  await expect
    .poll(() => shape(page))
    .toBe(
      ["Part One", "  Chapter 1"].join("\n"),
      "folding is this reader's own business, so it must not be written down",
    );
});

test("removing a section lifts its children into its place", async ({
  page,
}) => {
  await aNewProject(page, "Pruning");
  await openTheOutline(page);
  await aBookOf(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Remove Part One" }).click();

  await expect
    .poll(() => shape(page))
    .toBe(
      "Chapter 1",
      "pruning a part must not take the chapters inside it with it",
    );
});
