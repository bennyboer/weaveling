import { test, expect, type Page } from "@playwright/test";

import {
  aLooseScene,
  aNewProject,
  captureIdea,
  openTheOutline,
  openThePool,
} from "./support/shell";

const manuscript = (page: Page) =>
  page.getByRole("list", { name: "The manuscript" });

const waitingScenes = (page: Page) =>
  page.getByRole("list", { name: "Scenes not in the book" });

const waitingIdeas = (page: Page) =>
  page.getByRole("list", { name: "Ideas not in the book" });

const titles = (page: Page) => page.locator(".branch .title");

async function placeScene(page: Page, scene: string, into: string) {
  await place(page, scene, into);
}

async function placeIdea(page: Page, idea: string, into: string) {
  await place(page, idea, into);
}

async function place(page: Page, named: string, into: string) {
  await page.getByRole("button", { name: `Place ${named}` }).click();
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

async function addSections(page: Page, sections: string[]) {
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
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
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
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
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
  await addSections(page, ["Kapitel 1", "Kapitel 2"]);
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

  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect(manuscript(page).getByRole("textbox")).toHaveCount(2);

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a title still being saved survives a structural answer that predates it", async ({
  page,
}) => {
  await aNewProject(page, "Slow titles");
  await openTheOutline(page);
  const saved: string[] = [];
  await page.route("**/api/outlines/*/sections/*", async (route) => {
    if (route.request().method() !== "PATCH") {
      return route.continue();
    }
    saved.push(route.request().postDataJSON().title);
    await new Promise((resolve) => setTimeout(resolve, 300));
    await route.continue();
  });

  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Escape");

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));
  await page.waitForTimeout(800);
  expect(await shape(page)).toBe(["Part One", "Chapter 1"].join("\n"));
  expect(
    saved,
    "an answer computed before the title landed must not make the field save the old title back",
  ).not.toContain("");
});

test("Shift+Tab lifts a section back out", async ({ page }) => {
  await aNewProject(page, "Lifting");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
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
  await addSections(page, ["Part One", "Chapter"]);

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
  await addSections(page, ["The Silent Loom"]);
  const saved = page.waitForResponse(
    (response) =>
      response.request().method() === "PATCH" &&
      response.url().includes("/sections/"),
  );
  await page.keyboard.press("Escape");
  await saved;

  await page.reload();

  await expect(titles(page).first()).toHaveValue("The Silent Loom");
});

test("a section with nothing in it is flagged without being refused", async ({
  page,
}) => {
  await aNewProject(page, "Holes");
  await openTheOutline(page);

  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await expect(page.locator(".hollow")).toHaveCount(1);
  await expect(titles(page).first()).toHaveValue("Chapter 1");
});

test("a scene is placed from the tray and leaves it", async ({ page }) => {
  await aNewProject(page, "Placing");
  await aLooseScene(page);
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await placeScene(page, "Empty", "Chapter 1");

  await expect(page.locator(".leaf .name")).toHaveText("Empty");
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);
  await expect(page.locator(".hollow")).toHaveCount(0);
});
test("writing in a section opens the new scene", async ({ page }) => {
  await aNewProject(page, "Writing");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await page.getByRole("button", { name: "Write in Chapter 1" }).click();

  await expect(
    page.locator(".surface .ProseMirror"),
    "the button says write, so it has to leave the author writing rather than appending a blank row they may not even see",
  ).toBeVisible();
  await expect(page).toHaveURL(/\/scenes\/scene_/);
});

test("a scene written in a section is held there", async ({ page }) => {
  await aNewProject(page, "Holding");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();

  await openTheOutline(page);

  await expect(page.locator(".leaf .name")).toHaveText("Empty");
  await expect(page.locator(".hollow")).toHaveCount(0);
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);
});

test("a scene in the book opens from the outline", async ({ page }) => {
  await aNewProject(page, "Opening");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await openTheOutline(page);
  await expect(page.locator(".leaf .name")).toHaveText("Empty");

  await page.locator(".leaf").getByRole("link", { name: "Empty" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/scenes\/scene_/);
});
test("a scene taken out of the book goes back to the tray", async ({
  page,
}) => {
  await aNewProject(page, "Removing");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await openTheOutline(page);
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);

  await page
    .getByRole("button", { name: "Take Empty out of the book" })
    .click();

  await expect(waitingScenes(page).getByRole("button")).toHaveCount(1);
});
test("the row menu promotes, demotes and removes", async ({ page }) => {
  await aNewProject(page, "Menus");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
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

async function haul(page: Page, what: string, onto: string, at: number) {
  const grip = page.getByRole("button", { name: `Move ${what}`, exact: true });
  const target = page.getByRole("textbox", { name: `Section ${onto}` });
  const from = (await grip.boundingBox())!;
  const to = (await target.boundingBox())!;

  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 40, to.y + to.height * at, { steps: 10 });
}

test("dragging a section onto another nests it inside", async ({ page }) => {
  await aNewProject(page, "Nesting by drag");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2"]);
  await page.keyboard.press("Escape");

  await haul(page, "Kapitel 2", "Kapitel 1", 0.5);
  await expect(page.locator(".row.nesting")).toHaveCount(1);
  await page.mouse.up();

  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "  Kapitel 2"].join("\n"));
});

test("dragging a section to the edge of another puts it beside it", async ({
  page,
}) => {
  await aNewProject(page, "Beside by drag");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  await haul(page, "Kapitel 3", "Kapitel 1", 0.08);
  await expect(page.locator(".row.above")).toHaveCount(1);
  await page.mouse.up();

  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 3", "Kapitel 1", "Kapitel 2"].join("\n"));
});

test("a section cannot be dragged inside itself", async ({ page }) => {
  await aNewProject(page, "No swallowing");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  await haul(page, "Part One", "Chapter 1", 0.5);

  await expect(
    page.locator(".row.nesting, .row.above, .row.below"),
    "a book cannot contain itself, so no landing may be offered",
  ).toHaveCount(0);
  await page.mouse.up();

  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a scene can be dragged from the rail onto a section", async ({
  page,
}) => {
  await aNewProject(page, "Dragging");
  await aLooseScene(page);
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  const chip = page.getByRole("button", { name: "Place Empty" });
  const onto = page.getByRole("textbox", { name: "Section Chapter 1" });
  const from = (await chip.boundingBox())!;
  const to = (await onto.boundingBox())!;

  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 40, to.y + to.height / 2, { steps: 10 });
  await expect(page.locator(".row.landing")).toHaveCount(1);
  await page.mouse.up();

  await expect(page.locator(".leaf .name")).toHaveText("Empty");
});
test("the connector under the last child stops at its own row", async ({
  page,
}) => {
  await aNewProject(page, "Connectors");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1", "Chapter 2"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Demote Chapter 1" }).click();
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.getByRole("button", { name: "Demote Chapter 2" }).click();
  await expect.poll(() => shape(page)).toContain("  Chapter 2");

  const stems = await page.evaluate(() =>
    [...document.querySelectorAll(".twigs > .branch")].map((branch) => ({
      stem: Math.round(parseFloat(getComputedStyle(branch, "::before").height)),
      row: Math.round(branch.getBoundingClientRect().height),
      last: branch === branch.parentElement!.lastElementChild,
    })),
  );

  expect(stems.length).toBe(2);
  for (const { stem, row, last } of stems) {
    if (last) {
      expect(
        stem,
        "the last child must not trail a line below itself",
      ).toBeLessThan(row);
    } else {
      expect(
        stem,
        "every other child must carry the line down to the next",
      ).toBe(row);
    }
  }
});

test("folding a section hides what is inside it without changing the book", async ({
  page,
}) => {
  await aNewProject(page, "Folding");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
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
  await addSections(page, ["Part One", "Chapter 1"]);
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

test("an idea is noted in a section and is not a scene", async ({ page }) => {
  await aNewProject(page, "Noting");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  await placeIdea(page, "The loom remembers", "Chapter 1");

  await expect(page.locator(".leaf .name")).toHaveText("The loom remembers");
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(0);
  await expect(
    waitingScenes(page).getByRole("button"),
    "the tally is a to-do list for scenes with no home, and an idea noted in a chapter is not a scene",
  ).toHaveCount(0);
});

test("a note and a scene are drawn with different marks", async ({
  page,
}) => {
  await aNewProject(page, "Telling");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await openTheOutline(page);
  await placeIdea(page, "The loom remembers", "Chapter 1");

  await expect(page.locator(".leaf")).toHaveCount(2);
  const marks = await page
    .locator(".leaf > .row > svg path")
    .evaluateAll((drawn) => drawn.map((path) => path.getAttribute("d")));

  expect(marks).toHaveLength(2);
  expect(
    new Set(marks).size,
    "an idea and a scene sitting side by side in a chapter are different kinds of thing, so one glyph for both leaves the author guessing",
  ).toBe(2);
});

test("a note leads to the idea, not to an editor", async ({ page }) => {
  await aNewProject(page, "Leading");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await placeIdea(page, "The loom remembers", "Chapter 1");

  await page
    .locator(".leaf")
    .getByRole("link", { name: "The loom remembers" })
    .click();

  await expect(page).toHaveURL(/\/ideas\/the-loom-remembers-idea_/);
  await expect(page.locator(".surface .ProseMirror")).toHaveCount(0);
});

test("the tally counts scenes with no home, never ideas", async ({
  page,
}) => {
  await aNewProject(page, "Tallying");
  await openThePool(page);
  await captureIdea(page, "A first idea");
  await captureIdea(page, "A second idea");
  await aLooseScene(page);
  await openTheOutline(page);

  await expect(page.locator(".tray-toggle")).toHaveText(
    /^Not in the book · 1$/,
  );
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(2);
});

test("a section shows its scenes and its ideas as two groups", async ({
  page,
}) => {
  await aNewProject(page, "Grouping");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await placeIdea(page, "The loom remembers", "Chapter 1");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();

  await openTheOutline(page);

  await expect(
    page.locator(".kind-label"),
    "the idea went in first, so without grouping it would sit above the scene, at a place in the chapter nobody chose",
  ).toHaveText(["Scenes", "Ideas"]);
  await expect(
    page.getByRole("list", { name: "Scenes", exact: true }),
  ).toContainText("Empty");
  await expect(
    page.getByRole("list", { name: "Ideas", exact: true }),
  ).toContainText("The loom remembers");
});

test("a titled scene is named by its title in the outline", async ({
  page,
}) => {
  await aNewProject(page, "Naming");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  const saved = page.waitForResponse(
    (response) => response.request().method() === "PATCH",
  );
  await page
    .getByRole("textbox", { name: "Scene title" })
    .fill("The arrival");
  await page.getByRole("textbox", { name: "Scene title" }).press("Tab");
  await saved;

  await openTheOutline(page);

  await expect(
    page.locator(".leaf .name"),
    "an untitled scene falls back to its opening words; once named, the name wins",
  ).toHaveText("The arrival");
});
