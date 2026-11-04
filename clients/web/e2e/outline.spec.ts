import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject, API, captureIdea, openTheOutline, openThePool, sceneCatalogued } from "./support/shell";

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

async function savedTitles(page: Page) {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });

  return (await opened.json()).sections.map(
    (section: { title: string }) => section.title,
  );
}

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
  // given: a new project
  await aNewProject(page, "Switching");

  // when: its outline is opened
  await openTheOutline(page);

  // then: the outline is the view shown
  await expect(page).toHaveURL(/\/projects\/.+\/outline$/);
  await expect(
    page.getByRole("navigation", { name: "Views" }).getByRole("link", {
      name: "Outline",
      exact: true,
    }),
  ).toHaveClass(/here/);
});

test("a book starts with nothing in it", async ({ page }) => {
  // given: a new project
  await aNewProject(page, "Empty");

  // when: its outline is opened
  await openTheOutline(page);

  // then: the book has nothing in it
  await expect(
    page.getByText("Nothing in the book yet. Add a section to begin."),
  ).toBeVisible();
  await expect(titles(page)).toHaveCount(0);
});

test("the add button appends rather than pushing in at the top", async ({
  page,
}) => {
  // given: an empty outline
  await aNewProject(page, "Appending");
  await openTheOutline(page);

  // when: three sections are added one after another
  for (const named of ["Kapitel 1", "Kapitel 2", "Kapitel 3"]) {
    await page.getByRole("button", { name: "Add a section" }).click();
    await expect(titles(page).last()).toHaveValue("");
    await page.keyboard.type(named);
    await page.keyboard.press("Escape");
  }

  // then: they stand in the order they were added
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "Kapitel 2", "Kapitel 3"].join("\n"));
});

test("a section can be moved among its siblings", async ({ page }) => {
  // given: an outline of three sections
  await aNewProject(page, "Reordering");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  // when: the last is moved up from the keyboard
  await page.getByRole("textbox", { name: "Section Kapitel 3" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  // then: it moves above the one before
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "Kapitel 3", "Kapitel 2"].join("\n"));

  // when: the first is moved later by its button
  await page.getByRole("button", { name: "Move later Kapitel 1" }).click();
  // then: it moves below the one after
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 3", "Kapitel 1", "Kapitel 2"].join("\n"));
});

test("reordering moves sections without rewriting their titles", async ({
  page,
}) => {
  // given: an outline of three sections
  await aNewProject(page, "Intact");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  // when: the last is moved up
  await page.getByRole("textbox", { name: "Section Kapitel 3" }).click();
  await page.keyboard.press("Alt+ArrowUp");
  // then: the book is saved in the new order
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Kapitel 1", "Kapitel 3", "Kapitel 2"]);

  // and: after a reload each section keeps its title
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
  // given: an outline of two sections
  await aNewProject(page, "Ends");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2"]);
  await page.keyboard.press("Escape");

  // then: neither can be moved past its end, nor the top one promoted
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
  // given: an empty outline
  await aNewProject(page, "Nesting");
  await openTheOutline(page);

  // when: a section is added, then another with enter, and tab pressed
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect(manuscript(page).getByRole("textbox")).toHaveCount(2);

  // then: the second is nested under the first
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a title still being saved survives a structural answer that predates it", async ({
  page,
}) => {
  // given: an outline whose titles are slow to save
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

  // when: a second section is typed, nested and lifted out again
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Shift+Tab");
  await page.keyboard.press("Escape");

  // then: both stand at the top with their titles
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));
  await page.waitForTimeout(800);
  expect(await shape(page)).toBe(["Part One", "Chapter 1"].join("\n"));
  // and: no title was saved empty
  expect(
    saved,
    "an answer computed before the title landed must not make the field save the old title back",
  ).not.toContain("");
});

test("Shift+Tab lifts a section back out", async ({ page }) => {
  // given: a section nested under another
  await aNewProject(page, "Lifting");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");

  // when: shift and tab are pressed
  await page.keyboard.press("Shift+Tab");

  // then: it is lifted back out
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));
});

test("typing carries on in the same section after it is nested", async ({
  page,
}) => {
  // given: a second section being typed
  await aNewProject(page, "Carrying");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter"]);

  // when: it is nested and typing carries on
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter");
  await page.keyboard.type(" 1");

  // then: the title is typed whole in the nested section
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a title survives a reload", async ({ page }) => {
  // given: a section titled and saved
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

  // when: the page is reloaded
  await page.reload();

  // then: the title is still there
  await expect(titles(page).first()).toHaveValue("The Silent Loom");
});

test("a section with nothing in it is flagged without being refused", async ({
  page,
}) => {
  // given: an empty outline
  await aNewProject(page, "Holes");
  await openTheOutline(page);

  // when: a section is added with nothing in it
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  // then: it is flagged
  await expect(page.locator(".hollow")).toHaveCount(1);
  // and: kept
  await expect(titles(page).first()).toHaveValue("Chapter 1");
});

test("a scene is placed from the tray and leaves it", async ({ page }) => {
  // given: an outline with a section and a scene not in the book
  await aNewProject(page, "Placing");
  await aLooseScene(page);
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  // when: the scene is placed in the section
  await placeScene(page, "Empty", "Chapter 1");

  // then: it is held there and leaves the tray
  await expect(page.locator(".leaf .name")).toHaveText("Empty");
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);
  await expect(page.locator(".hollow")).toHaveCount(0);
});
test("writing in a section opens the new scene", async ({ page }) => {
  // given: an outline with a section
  await aNewProject(page, "Writing");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  // and: placing slow to answer
  await page.route("**/api/outlines/*/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });

  // when: the section is written in
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();

  // then: a new scene opens, ready to write in
  await expect(
    page
      .getByRole("article", { name: "The book" })
      .locator(".scene .ProseMirror"),
    "the button says write, so it has to leave the author writing rather than appending a blank row they may not even see",
  ).toBeFocused();
  await expect(page).toHaveURL(/\/write$/);
});

test("writing in a section twice in a hurry makes one scene", async ({ page }) => {
  // given: an outline with two sections
  await aNewProject(page, "WritingTwice");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1", "Chapter 2"]);
  await page.keyboard.press("Escape");
  // and: placing slow to answer
  await page.route("**/api/outlines/*/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });
  // and: every scene made counted
  let made = 0;
  page.on("request", (request) => {
    if (request.method() === "POST" && new URL(request.url()).pathname === "/api/scenes") {
      made += 1;
    }
  });
  const writeIn = page.getByRole("button", { name: "Write in Chapter 1" });

  // when: the section is written in twice in a hurry
  await writeIn.dblclick();

  // then: one scene was made, open to write in
  await expect(page).toHaveURL(/\/write$/);
  expect(made, "a second click while the first is on its way must not make a second scene").toBe(1);
});

test("a section being written in shows it on its way, and no other section does", async ({ page }) => {
  // given: an outline with two sections
  await aNewProject(page, "WritingShown");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1", "Chapter 2"]);
  await page.keyboard.press("Escape");
  // and: placing slow to answer
  await page.route("**/api/outlines/*/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
  });
  const writeIn = page.getByRole("button", { name: "Write in Chapter 1" });

  // when: the first is written in
  await writeIn.click();

  // then: its button shows it on its way
  await expect(writeIn).toHaveAttribute("aria-busy", "true");
  // and: the other's does not
  await expect(page.getByRole("button", { name: "Write in Chapter 2" })).not.toHaveAttribute("aria-busy");
});

test("a section written in and left before the answer comes leaves the author where they went", async ({ page }) => {
  // given: an outline with a section
  await aNewProject(page, "WritingLeft");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  // and: placing slow to answer
  let placed = () => {};
  const answered = new Promise<void>((resolve) => {
    placed = resolve;
  });
  await page.route("**/api/outlines/*/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 800));
    await route.continue();
    placed();
  });

  // when: the section is written in
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  // and: the author goes to the board before the answer comes
  await page.getByRole("navigation", { name: "Views" }).getByRole("link", { name: "Board" }).click();
  await expect(page).toHaveURL(/\/board$/);
  await answered;
  await page.waitForTimeout(300);

  // then: the author stays on the board
  await expect(page, "taking them back to write after they chose to leave would undo their choice").toHaveURL(/\/board$/);
});

test("a scene written in a section is held there", async ({ page }) => {
  // given: a section written in
  await aNewProject(page, "Holding");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);

  // when: the outline is opened again
  await openTheOutline(page);

  // then: the new scene is held in the section
  await expect(page.locator(".leaf .name")).toHaveText("Empty");
  await expect(page.locator(".hollow")).toHaveCount(0);
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);
});

test("a scene in the book opens from the outline", async ({ page }) => {
  // given: a section with a scene in it
  await aNewProject(page, "Opening");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);
  await openTheOutline(page);
  await expect(page.locator(".leaf .name")).toHaveText("Empty");

  // when: the scene is opened from the outline
  await page.locator(".leaf").getByRole("link", { name: "Empty" }).click();

  // then: it opens where it stands in the book, ready to write in
  await expect(
    page
      .getByRole("article", { name: "The book" })
      .locator(".scene .ProseMirror"),
    "a scene in the book is written where it stands in the book",
  ).toBeFocused();
  await expect(page).toHaveURL(/\/write$/);
});
test("a scene taken out of the book goes back to the tray", async ({
  page,
}) => {
  // given: a section with a scene in it
  await aNewProject(page, "Removing");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);
  await openTheOutline(page);
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(0);

  // when: the scene is taken out of the book
  await page
    .getByRole("button", { name: "Take Empty out of the book" })
    .click();

  // then: it goes back to the tray
  await expect(waitingScenes(page).getByRole("button")).toHaveCount(1);
});
test("the row menu promotes, demotes and removes", async ({ page }) => {
  // given: an outline of two sections
  await aNewProject(page, "Menus");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Escape");

  // when: the second is demoted from its menu
  await page.getByRole("button", { name: "Demote Chapter 1" }).click();
  // then: it is nested under the first
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));

  // when: it is promoted
  await page.getByRole("button", { name: "Promote Chapter 1" }).click();
  // then: it is lifted back out
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "Chapter 1"].join("\n"));

  // when: the first is removed
  await page.getByRole("button", { name: "Remove Part One" }).click();
  // then: only the second is left
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
  // given: an outline of two sections
  await aNewProject(page, "Nesting by drag");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2"]);
  await page.keyboard.press("Escape");

  // when: the second is dragged onto the first
  await haul(page, "Kapitel 2", "Kapitel 1", 0.5);
  // then: nesting is shown
  await expect(page.locator(".row.nesting")).toHaveCount(1);
  // when: it is let go
  await page.mouse.up();

  // then: it is nested inside
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 1", "  Kapitel 2"].join("\n"));
});

test("dragging a section to the edge of another puts it beside it", async ({
  page,
}) => {
  // given: an outline of three sections
  await aNewProject(page, "Beside by drag");
  await openTheOutline(page);
  await addSections(page, ["Kapitel 1", "Kapitel 2", "Kapitel 3"]);
  await page.keyboard.press("Escape");

  // when: the last is dragged to the top edge of the first
  await haul(page, "Kapitel 3", "Kapitel 1", 0.08);
  // then: landing above is shown
  await expect(page.locator(".row.above")).toHaveCount(1);
  // when: it is let go
  await page.mouse.up();

  // then: it stands above the first
  await expect
    .poll(() => shape(page))
    .toBe(["Kapitel 3", "Kapitel 1", "Kapitel 2"].join("\n"));
});

test("a section cannot be dragged inside itself", async ({ page }) => {
  // given: a section with another nested inside
  await aNewProject(page, "No swallowing");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  // when: it is dragged onto the one inside it
  await haul(page, "Part One", "Chapter 1", 0.5);

  // then: no landing is offered
  await expect(
    page.locator(".row.nesting, .row.above, .row.below"),
    "a book cannot contain itself, so no landing may be offered",
  ).toHaveCount(0);
  // when: it is let go
  await page.mouse.up();

  // then: the outline is unchanged
  await expect
    .poll(() => shape(page))
    .toBe(["Part One", "  Chapter 1"].join("\n"));
});

test("a scene can be dragged from the rail onto a section", async ({
  page,
}) => {
  // given: an outline with a section and a scene not in the book
  await aNewProject(page, "Dragging");
  await aLooseScene(page);
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  // when: the scene is dragged from the rail onto the section
  const chip = page.getByRole("button", { name: "Place Empty" });
  const onto = page.getByRole("textbox", { name: "Section Chapter 1" });
  const from = (await chip.boundingBox())!;
  const to = (await onto.boundingBox())!;

  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + 40, to.y + to.height / 2, { steps: 10 });
  // then: landing is shown
  await expect(page.locator(".row.landing")).toHaveCount(1);
  // when: it is let go
  await page.mouse.up();

  // then: the scene is held in the section
  await expect(page.locator(".leaf .name")).toHaveText("Empty");
});
test("the connector under the last child stops at its own row", async ({
  page,
}) => {
  // given: a section with two nested inside
  await aNewProject(page, "Connectors");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1", "Chapter 2"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Demote Chapter 1" }).click();
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.getByRole("button", { name: "Demote Chapter 2" }).click();
  await expect.poll(() => shape(page)).toContain("  Chapter 2");

  // then: only the connector under the last stops at its own row
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
  // given: a section with another nested inside
  await aNewProject(page, "Folding");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  // when: it is folded
  await page.getByRole("button", { name: "Fold Part One" }).click();

  // then: what is inside is hidden
  await expect(titles(page)).toHaveCount(1);
  await expect(
    page.getByRole("button", { name: "Fold Part One" }),
  ).toHaveAttribute("aria-expanded", "false");

  // when: the page is reloaded
  await page.reload();

  // then: the book is unchanged
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
  // given: a section with another nested inside
  await aNewProject(page, "Pruning");
  await openTheOutline(page);
  await addSections(page, ["Part One", "Chapter 1"]);
  await page.keyboard.press("Tab");
  await expect.poll(() => shape(page)).toContain("  Chapter 1");
  await page.keyboard.press("Escape");

  // when: it is removed
  await page.getByRole("button", { name: "Remove Part One" }).click();

  // then: the one inside takes its place
  await expect
    .poll(() => shape(page))
    .toBe(
      "Chapter 1",
      "pruning a part must not take the chapters inside it with it",
    );
});

test("an idea is noted in a section and is not a scene", async ({ page }) => {
  // given: an outline with a section and an idea not in the book
  await aNewProject(page, "Noting");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");

  // when: the idea is placed in the section
  await placeIdea(page, "The loom remembers", "Chapter 1");

  // then: it is noted there and leaves the tray
  await expect(page.locator(".leaf .name")).toHaveText("The loom remembers");
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(0);
  // and: is not counted as a scene
  await expect(
    waitingScenes(page).getByRole("button"),
    "the tally is a to-do list for scenes with no home, and an idea noted in a chapter is not a scene",
  ).toHaveCount(0);
});

test("a note and a scene are drawn with different marks", async ({ page }) => {
  // given: a section with a scene and an idea in it
  await aNewProject(page, "Telling");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);
  await openTheOutline(page);
  await placeIdea(page, "The loom remembers", "Chapter 1");

  // then: each is drawn with a mark of its own
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
  // given: a section with an idea noted in it
  await aNewProject(page, "Leading");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await placeIdea(page, "The loom remembers", "Chapter 1");

  // when: the note is followed
  await page
    .locator(".leaf")
    .getByRole("link", { name: "The loom remembers" })
    .click();

  // then: the idea opens, not an editor
  await expect(page).toHaveURL(/\/ideas\/the-loom-remembers-idea_/);
  await expect(page.locator(".surface .ProseMirror")).toHaveCount(0);
});

test("the tally counts scenes with no home, never ideas", async ({ page }) => {
  // given: two ideas and a scene not in the book
  await aNewProject(page, "Tallying");
  await openThePool(page);
  await captureIdea(page, "A first idea");
  await captureIdea(page, "A second idea");
  await aLooseScene(page);
  // when: the outline is opened
  await openTheOutline(page);

  // then: the tally counts only the scene
  await expect(
    page.getByRole("button", { name: /^Not in the book ·/ }),
  ).toHaveText(/^Not in the book · 1$/);
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(2);
});

test("a section shows its scenes and its ideas as two groups", async ({
  page,
}) => {
  // given: a section noting an idea, then written in
  await aNewProject(page, "Grouping");
  await openThePool(page);
  await captureIdea(page, "The loom remembers");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await placeIdea(page, "The loom remembers", "Chapter 1");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);
  const begun = await page
    .getByRole("article", { name: "The book" })
    .locator(".scene[data-scene]")
    .first()
    .getAttribute("data-scene");
  await sceneCatalogued(page, begun!);

  // when: the outline is opened again
  await openTheOutline(page);

  // then: its scenes and its ideas show as two groups, scenes first
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
  // given: a section with a scene in it, titled
  await aNewProject(page, "Naming");
  await openTheOutline(page);
  await addSections(page, ["Chapter 1"]);
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Write in Chapter 1" }).click();
  await expect(page).toHaveURL(/\/write$/);
  const saved = page.waitForResponse(
    (response) => response.request().method() === "PATCH",
  );
  await page.getByRole("button", { name: "Scene actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("textbox", { name: "Scene title" }).fill("The arrival");
  await page.getByRole("textbox", { name: "Scene title" }).press("Enter");
  await saved;
  await sceneCatalogued(
    page,
    new URL((await saved).url()).pathname.split("/").pop()!,
    "The arrival",
  );

  // when: the outline is opened again
  await openTheOutline(page);

  // then: the scene is named by its title
  await expect(
    page.locator(".leaf .name"),
    "an untitled scene falls back to its opening words; once named, the name wins",
  ).toHaveText("The arrival");
});

test("a title being typed survives a slower answer about another section", async ({
  page,
}) => {
  // given: an outline whose titles are slow to save
  await aNewProject(page, "Overtaken");
  await openTheOutline(page);
  await page.route("**/api/outlines/*/sections/*", async (route) => {
    if (route.request().method() === "PATCH") {
      await new Promise((resolve) => setTimeout(resolve, 800));
    }
    await route.continue();
  });
  const firstRenamed = page.waitForResponse(
    (response) =>
      response.request().method() === "PATCH" &&
      response.request().postData()?.includes("Kapitel 1") === true,
  );

  // when: a second section is typed while the title of the first is saved
  await addSections(page, ["Kapitel 1", "Kapitel 2"]);
  await firstRenamed;

  // then: what is typed stays
  await expect(
    titles(page).last(),
    "the rename of Kapitel 1 lands while Kapitel 2 is being typed, and must not wipe it",
  ).toHaveValue("Kapitel 2");
  // and: both titles are saved
  await page.keyboard.press("Escape");
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Kapitel 1", "Kapitel 2"]);
});

test.describe("a chapter far longer than the window", () => {
  test.describe.configure({ mode: "serial" });

  const book = { at: "", scenes: [] as string[] };

  test.beforeAll(async ({ browser }) => {
    test.setTimeout(180_000);
    const page = await browser.newPage();
    await aNewProject(page, "LongChapter");
    book.scenes = (await aBookWithScenes(page, { "Part One": 250 }))[
      "Part One"
    ];
    book.at = new URL(page.url()).pathname;
    await page.close();
  });

  const group = (page: Page, nth: number) =>
    page.locator(`[data-leaves$="/scenes/${nth}"]`);

  const leafOf = (page: Page, scene: string) =>
    page.locator(`.leaf a[href$="${scene}"]`);

  test("only the leaves near the window are drawn, and a far group stands as tall as its leaves", async ({
    page,
  }) => {
    // when: the outline of a far longer chapter is opened
    await page.goto(book.at);
    await openTheOutline(page);

    // then: only the leaves near the window are drawn, a far group standing in its place
    await expect(group(page, 0)).not.toHaveClass(/waiting/);
    await expect(group(page, 2)).toHaveClass(/waiting/);
    await expect(
      group(page, 2).locator(".leaf"),
      "a group far below the window draws nothing but its place",
    ).toHaveCount(0);
    const waiting = (await group(page, 2).boundingBox())!.height;
    const height = await page.evaluate(() => document.body.scrollHeight);

    // when: the far group is scrolled to
    await group(page, 2).scrollIntoViewIfNeeded();

    // then: its leaves are drawn, and neither it nor the outline changes height
    await expect(group(page, 2).locator(".leaf")).toHaveCount(50);
    await expect(leafOf(page, book.scenes[149])).toBeVisible();
    expect(
      Math.abs((await group(page, 2).boundingBox())!.height - waiting),
      "a group must not change height as it is drawn, or the outline jumps under the reader",
    ).toBeLessThanOrEqual(0.5);
    expect(
      Math.abs(
        (await page.evaluate(() => document.body.scrollHeight)) - height,
      ),
      "the outline as a whole keeps its height, give or take the rounding of a pixel",
    ).toBeLessThanOrEqual(1);
  });

  test("a group scrolled far away is let go, but not the one holding the focus", async ({
    page,
  }) => {
    // given: the outline of a far longer chapter, a leaf near the top focused
    await page.goto(book.at);
    await openTheOutline(page);
    await expect(group(page, 1)).not.toHaveClass(/waiting/);
    await leafOf(page, book.scenes[0]).focus();

    // when: the outline is scrolled to the bottom
    await page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));

    // then: the groups scrolled away are let go
    await expect(group(page, 4)).not.toHaveClass(/waiting/);
    await expect(group(page, 1)).toHaveClass(/waiting/);
    // and: the one holding the focus is kept
    await expect(
      group(page, 0),
      "taking down the leaf with the cursor would drop the focus",
    ).not.toHaveClass(/waiting/);
    await expect(leafOf(page, book.scenes[0])).toBeFocused();
  });
});

test("a section title the server refuses is told as it is, not as overtaken", async ({
  page,
}) => {
  // given: an outline with a section
  await aNewProject(page, "TooLongATitle");
  await openTheOutline(page);
  await addSections(page, ["Part One"]);
  await page.keyboard.press("Escape");

  // when: it is given a title too long
  await titles(page).first().fill("a".repeat(2049));
  await page.keyboard.press("Escape");

  // then: the refusal is told as it is
  await expect(page.getByRole("alert")).toContainText(
    "A section's title can be at most 2048 characters long; this one has 2049.",
  );
});

