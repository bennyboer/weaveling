import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aNewProject, API, views } from "./support/shell";
import { aBookInTheWritingView, beginning, flow, heading, label, newScene, openTheWritingView, projectsOutline, savedTitles, scene, theBook } from "./support/writing";

async function aFrame(page: Page) {
  await page.evaluate(
    () =>
      new Promise((settled) =>
        requestAnimationFrame(() => setTimeout(settled)),
      ),
  );
}

const laidOut = (page: Page) =>
  flow(page).evaluate((book) =>
    [...book.querySelectorAll("[role=heading], hr.between-scenes, .scene")]
      .filter(
        (part) => !part.matches("hr") || (part as HTMLElement).offsetParent,
      )
      .map((part) =>
        part.matches("[role=heading]")
          ? `heading ${part.textContent}`
          : part.matches(".scene")
            ? `scene ${part.getAttribute("data-scene")}`
            : "line",
      ),
  );

const scenesOf = async (page: Page) =>
  (await projectsOutline(page)).sections.map(
    (section: { title: string; attachments: { kind: string }[] }) =>
      section.attachments.filter((attached) => attached.kind === "scene")
        .length,
  );

async function aSlowHandover(page: Page) {
  await page.route("**/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 250));
    await route.continue();
  });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
}

const SENTENCE = "The loom stood silent in the grey light.";

const caretIn = (page: Page) =>
  page.evaluate(() => {
    const chosen = getSelection()!;
    const block = chosen.focusNode!.parentElement!.closest("p")!;
    const before = document.createRange();
    before.selectNodeContents(block);
    before.setEnd(chosen.focusNode!, chosen.focusOffset);

    return `${block.textContent} @${before.toString().length}`;
  });

test("Write sits beside the other views and opens the book", async ({
  page,
}) => {
  // given: a project
  await aNewProject(page, "Writing");

  // when: the writing view is opened
  await openTheWritingView(page);

  // then: it has an address of its own
  await expect(page).toHaveURL(/\/projects\/[^/]+\/write$/);
  // and: the views mark it
  await expect(
    views(page).getByRole("link", { name: "Write", exact: true }),
  ).toHaveClass(/here/);
});

test("the book's sections read in order, nested as headings", async ({
  page,
}) => {
  // given: a book of nested sections
  await aNewProject(page, "Shaped");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "The Workshop", under: "Part One" },
    { title: "Upstairs", under: "The Workshop" },
    { title: "The River", under: "Part Two" },
  ]);

  // when: the writing view is opened
  await openTheWritingView(page);

  // then: the sections read in order
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "The Workshop",
    "Upstairs",
    "Part Two",
    "The River",
  ]);
  // and: each is a heading as deep as it is nested
  await expect(
    flow(page).getByRole("heading", { name: "Part One", level: 2 }),
  ).toBeVisible();
  await expect(
    flow(page).getByRole("heading", { name: "The Workshop", level: 3 }),
  ).toBeVisible();
  await expect(
    flow(page).getByRole("heading", { name: "Upstairs", level: 4 }),
  ).toBeVisible();
});

test("an untitled section still has a heading to find it by", async ({
  page,
}) => {
  // given: a book with an untitled section
  await aNewProject(page, "Nameless");
  await aBookShaped(page, [{ title: "" }]);

  // when: the writing view is opened
  await openTheWritingView(page);

  // then: the section has a heading with a placeholder
  await expect(flow(page).getByRole("heading")).toHaveText([""]);
  await expect(flow(page).getByRole("heading")).toHaveAttribute(
    "data-placeholder",
    "Untitled section",
  );
});

test("the book survives a reload of the writing view", async ({ page }) => {
  // given: a book open in the writing view
  await aNewProject(page, "Reloaded");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  // when: the page is reloaded
  await page.reload();

  // then: the book is still there
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("each section's scenes follow its heading, a line between them", async ({
  page,
}) => {
  // expect: each section's scenes follow its heading, a line between them
  const scenes = await aBookInTheWritingView(page, "Scenes", { "Part One": 2, "Part Two": 1 });

  await expect
    .poll(() => laidOut(page))
    .toEqual([
      "heading Part One",
      `scene ${scenes["Part One"][0]}`,
      "line",
      `scene ${scenes["Part One"][1]}`,
      "heading Part Two",
      `scene ${scenes["Part Two"][0]}`,
    ]);
});

test("prose written in the flow is kept", async ({ page }) => {
  // given: a scene open in the writing view
  const scenes = await aBookInTheWritingView(page, "Kept", { "Part One": 1 });

  // when: prose is written in it
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
  // and: stored
  await expect
    .poll(
      async () =>
        (
          await (
            await page.request.get(`${API}/scenes/${scenes["Part One"][0]}`)
          ).json()
        ).text,
      { message: "a reload only shows what the server has stored" },
    )
    .toContain("The loom stood silent.");

  // and: the page is reloaded
  await page.reload();

  // then: the prose is still there
  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "The loom stood silent.",
  );
});

test("the arrow keys carry the cursor from one scene into the next and back", async ({
  page,
}) => {
  // given: a book of two scenes, the cursor at the end of the first
  await aNewProject(page, "Crossing");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  // when: the cursor is moved down and something typed
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("She came back.");

  // then: it lands in the second scene
  await expect(scene(page, second)).toContainText("She came back.");

  // when: the cursor is moved up from its start and something typed
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Dust.");

  // then: it lands at the end of the first scene
  await expect(
    scene(page, first),
    "arriving from below lands at the end of the scene above",
  ).toContainText("The loom stood silent. Dust.");
});

test("the cursor crosses a section's heading on its way to the next section", async ({
  page,
}) => {
  // given: a book of two sections, the cursor in the first's scene
  const scenes = await aBookInTheWritingView(page, "Sections", { "Part One": 1, "Part Two": 1 });
  await scene(page, scenes["Part One"][0]).click();

  // when: the cursor is moved right
  await page.keyboard.press("ArrowRight");
  // then: it is in the next heading
  await expect(heading(page, "Part Two")).toBeFocused();
  // when: it is moved down and something typed
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Across the heading.");

  // then: it lands in the next section's scene
  await expect(scene(page, scenes["Part Two"][0])).toContainText(
    "Across the heading.",
  );
});

test("moving up inside a scene stays in it", async ({ page }) => {
  // given: the cursor at the end of a scene of two lines
  await aNewProject(page, "Staying");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("First line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second line.");

  // when: it is moved up and something typed
  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Still here.");

  // then: it lands in the first line
  await expect(scene(page, second)).toContainText("First line. Still here.");
  // and: not in the scene above
  await expect(scene(page, first)).not.toContainText("Still here.");
});

test("an arrow key just after a scene takes focus is not undone, even on a busy page", async ({
  page,
}) => {
  // given: the cursor at the end of a scene of two lines
  await aNewProject(page, "Busy");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const written = scene(page, scenes["Part One"][0]);
  await openTheWritingView(page);
  await written.click();
  await page.keyboard.type("First line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second line.");

  // when: again and again a busy page focuses the scene just before the cursor moves up
  for (let round = 0; round < 15; round++) {
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("End");
    await written.evaluate((editor) => (editor as HTMLElement).blur());
    await written.evaluate((editor) => {
      (editor as HTMLElement).focus();
      setTimeout(() => {
        const until = performance.now() + 80;
        while (performance.now() < until) {}
      }, 5);
    });
    await page.keyboard.press("ArrowUp");
    await page.keyboard.type("X");
  }

  // then: every move up is kept
  await expect(written.locator("p")).toHaveText([
    `First line.${"X".repeat(15)}`,
    "Second line.",
  ]);
});

test("the first scene's top and the last scene's end lead nowhere", async ({
  page,
}) => {
  // given: the cursor in the only scene of a book
  const scenes = await aBookInTheWritingView(page, "Ends", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();

  // when: it is moved up, then down, and something typed
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Still the only scene.");

  // then: it lands in that scene
  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Still the only scene.",
  );
});

test("a word jump at a scene's edge carries on into the next scene", async ({
  page,
}) => {
  // given: a book of two scenes, the cursor at the end of the first
  await aNewProject(page, "Jumping");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  // when: a word jump forward is made and something typed
  await page.keyboard.press("ControlOrMeta+ArrowRight");
  await page.keyboard.type("Forward.");
  // then: it lands in the second scene
  await expect(scene(page, second)).toContainText("Forward.");

  // when: from its start a word jump back is made and something typed
  await page.keyboard.press("Home");
  await aFrame(page);
  await page.keyboard.press("ControlOrMeta+ArrowUp");
  await page.keyboard.type(" Back.");
  // then: it lands at the end of the first scene
  await expect(
    scene(page, second),
    "the jump should have left this scene",
  ).not.toContainText("Back.");
  await expect(scene(page, first)).toContainText("silent. Back.");
});

test("a word jump inside a scene stays a word jump", async ({ page }) => {
  // given: the cursor at the end of a scene's text
  await aNewProject(page, "Words");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("The loom stood silent");

  // when: a word jump back is made and something typed
  await page.keyboard.press("ControlOrMeta+ArrowLeft");
  await page.keyboard.type("very ");

  // then: it lands before the last word
  await expect(scene(page, second)).toContainText("stood very silent");
  // and: not in the scene above
  await expect(scene(page, first)).not.toContainText("very");
});

test("words typed while the first section is being made are kept", async ({
  page,
}) => {
  // given: a project whose first section is slow to be made
  await aNewProject(page, "Quick");
  await page.route("**/api/outlines/*/sections", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 600));
    await route.continue();
  });
  // and: the writing view open
  await openTheWritingView(page);

  // when: its heading is typed into
  await flow(page).getByRole("heading").click();
  await page.keyboard.type("The Workshop");

  // then: the heading keeps every word
  await expect(flow(page).getByRole("heading")).toHaveText(["The Workshop"]);
  // and: they are saved
  await expect.poll(() => savedTitles(page)).toEqual(["The Workshop"]);
});

test("a heading is renamed where it stands", async ({ page }) => {
  // given: a section open in the writing view
  await aNewProject(page, "Renaming");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  // when: its heading is renamed
  await heading(page, "Part One").click();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("The Beginning");
  await page.keyboard.press("Enter");

  // then: the heading shows the new name
  await expect(flow(page).getByRole("heading")).toHaveText(["The Beginning"]);
  // and: it is saved
  await expect.poll(() => savedTitles(page)).toEqual(["The Beginning"]);
});

test("a heading being typed is saved after a pause, even before leaving it", async ({
  page,
}) => {
  // given: a section open in the writing view
  await aNewProject(page, "Pausing");
  await aBookShaped(page, [{ title: "Part" }]);
  await openTheWritingView(page);

  // when: its heading is typed into
  await heading(page, "Part").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" One");

  // then: it is saved after a pause
  await expect.poll(() => savedTitles(page)).toEqual(["Part One"]);
  // and: the heading is still being typed in
  await expect(heading(page, "Part One")).toBeFocused();
});

test("Enter leaves a heading rather than breaking it", async ({ page }) => {
  // given: a section open in the writing view
  await aNewProject(page, "Entering");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  // when: Enter is pressed at the end of its heading
  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");

  // then: the heading is left
  await expect(heading(page, "Part One")).not.toBeFocused();
  // and: it is not broken
  expect(await heading(page, "Part One").textContent()).toBe("Part One");
});

test("a scene being written is left alone while its section is renamed", async ({
  page,
}) => {
  // given: a scene being written
  await aNewProject(page, "Undisturbed");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const written = scene(page, scenes["Part One"][0]);
  await openTheWritingView(page);
  await written.click();
  await page.keyboard.type("The loom stood silent.");
  await written.evaluate((editor) => {
    (editor as HTMLElement & { marked?: boolean }).marked = true;
  });

  // when: its section is renamed
  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", Revised");
  await page.keyboard.press("Enter");
  await expect.poll(() => savedTitles(page)).toEqual(["Part One, Revised"]);

  // then: the scene is not built anew
  expect(
    await written.evaluate(
      (editor) => (editor as HTMLElement & { marked?: boolean }).marked,
    ),
    "a renamed outline must not rebuild the scenes around it",
  ).toBe(true);
  // and: it keeps its text
  await expect(written).toContainText("The loom stood silent.");
});

test("an empty scene can be seen and clicked into", async ({ page }) => {
  // given: an empty scene open in the writing view
  const scenes = await aBookInTheWritingView(page, "Empty", { "Part One": 1 });
  const empty = scene(page, scenes["Part One"][0]);

  // then: it can be seen, inviting to write
  await expect(empty).toBeVisible();
  expect(
    await empty.evaluate(
      (editor) =>
        getComputedStyle(editor.querySelector("p")!, "::before").content,
    ),
  ).toBe('"An empty scene. Start writing…"');
  expect((await empty.boundingBox())!.height).toBeGreaterThan(20);

  // when: it is clicked into and written in
  await empty.click();
  await page.keyboard.type("A first line.");

  // then: the invitation goes
  expect(
    await empty.evaluate(
      (editor) =>
        getComputedStyle(editor.querySelector("p")!, "::before").content,
    ),
    "the invitation goes once there are words",
  ).toBe("none");
});

test("down from a heading goes into its first scene, up from there comes back", async ({
  page,
}) => {
  // given: the cursor in a section's heading
  const scenes = await aBookInTheWritingView(page, "Down", { "Part One": 1 });
  await heading(page, "Part One").click();

  // when: it is moved down and something typed
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Below the heading.");
  // then: it lands in the section's first scene
  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Below the heading.",
  );

  // when: from the scene's start the cursor is moved up
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  // then: it is back in the heading, where typing goes on
  await expect(heading(page, "Part One")).toBeFocused();
  await page.keyboard.type(", Revised");
  await page.keyboard.press("Enter");
  await expect.poll(() => savedTitles(page)).toEqual(["Part One, Revised"]);
});

test("an empty section's writing line sits between its heading and the next", async ({
  page,
}) => {
  // given: two empty sections, the cursor in the first's heading
  await aBookInTheWritingView(page, "Hollow", { "Part One": 0, "Part Two": 0 });
  await heading(page, "Part One").click();

  // when: the cursor is moved down
  await page.keyboard.press("ArrowDown");
  // then: it is on the first section's writing line
  await expect(beginning(page).first()).toBeFocused();
  // when: it is moved down again
  await page.keyboard.press("ArrowDown");
  // then: it is in the next heading
  await expect(heading(page, "Part Two")).toBeFocused();
  // when: it is moved up twice
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  // then: it is back in the first heading
  await expect(heading(page, "Part One")).toBeFocused();
});

test("left and right leave a heading only from its very edge", async ({
  page,
}) => {
  // given: the cursor at the end of a heading below a scene
  const scenes = await aBookInTheWritingView(page, "Edges", { "Part One": 1, "Part Two": 0 });
  await heading(page, "Part Two").click();
  await page.keyboard.press("End");

  // when: it is moved left
  await page.keyboard.press("ArrowLeft");
  // then: it stays in the heading
  await expect(
    heading(page, "Part Two"),
    "inside a heading the arrow moves through its letters",
  ).toBeFocused();

  // when: from the heading's start it is moved left and something typed
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.type("The end.");
  // then: it lands at the end of the scene above
  await expect(scene(page, scenes["Part One"][0])).toContainText("The end.");
});

test("typing into an empty section begins a scene there", async ({ page }) => {
  // given: an empty section open in the writing view
  await aBookInTheWritingView(page, "Begun", { "Part One": 0 });
  // then: its writing line invites to write
  await expect(beginning(page)).toHaveAttribute(
    "data-placeholder",
    "An empty scene. Start writing…",
  );

  // when: it is typed into
  await beginning(page).click();
  await page.keyboard.type("The loom stood silent.");

  // then: a scene is begun there with what is typed
  await expect(beginning(page)).toHaveCount(0);
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent.",
  );
  // and: the section holds it
  await expect.poll(() => scenesOf(page)).toEqual([1]);
  // and: typing goes on in it
  await page.keyboard.type(" Still writing.");
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent. Still writing.",
  );
});

test("words typed while the scene is being made are kept", async ({ page }) => {
  // given: an empty section whose first scene is slow to be made
  await aNewProject(page, "Patient");
  await aBookWithScenes(page, { "Part One": 0 });
  await page.route("**/api/scenes", async (route) => {
    if (route.request().method() === "POST") {
      await new Promise((resolve) => setTimeout(resolve, 600));
    }
    await route.continue();
  });
  // and: the writing view open
  await openTheWritingView(page);

  // when: its writing line is typed into
  await beginning(page).click();
  await page.keyboard.type("Nothing typed in the wait is lost.");

  // then: everything typed is in the scene
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveText(
    "Nothing typed in the wait is lost.",
  );
  // and: it is stored
  await expect
    .poll(async () => {
      const outline = await projectsOutline(page);
      const scene = outline.sections[0].attachments[0]?.id;

      return scene
        ? (await (await page.request.get(`${API}/scenes/${scene}`)).json()).text
        : "";
    })
    .toBe("Nothing typed in the wait is lost.");
});

test("an empty book is written from its first heading into its first scene", async ({
  page,
}) => {
  // given: an empty book open in the writing view
  await aNewProject(page, "Blank");
  await openTheWritingView(page);
  // then: its first heading invites to name the first section
  const first = flow(page).getByRole("heading");
  await expect(first).toHaveAttribute(
    "data-placeholder",
    "Your book's first section",
  );

  // when: it is typed into
  await first.click();
  await page.keyboard.type("Part One");
  // then: the first section is made
  await expect
    .poll(() => savedTitles(page), {
      message: "typing into an empty book makes its first section",
    })
    .toEqual(["Part One"]);
  // when: the cursor is moved down and something typed
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("It began with a loom.");

  // then: it is stored in the section's first scene
  await expect
    .poll(async () => {
      const scene = (await projectsOutline(page)).sections[0].attachments[0]
        ?.id;

      return scene
        ? (await (await page.request.get(`${API}/scenes/${scene}`)).json()).text
        : "";
    })
    .toContain("It began with a loom.");
  // and: it is still there after a reload
  await page.reload();
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "It began with a loom.",
  );
});

test("Enter twice at a scene's end begins the next scene right after it", async ({
  page,
}) => {
  // given: a book of two scenes, the cursor at the end of the first
  await aNewProject(page, "Twice");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  // when: Enter is pressed twice and something typed
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  // then: a new scene is begun
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Morning came.");

  // and: it goes between the two
  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(3);
  const order = (await theBook(page))[0][1];
  expect(order[0]).toBe(first);
  expect(order[2], "the new scene goes between, not at the end").toBe(second);
  // and: holds what is typed
  await expect(scene(page, order[1])).toContainText("Morning came.");
  // and: the first scene keeps no empty line
  await expect(
    scene(page, first),
    "the empty paragraph left by the first Enter goes",
  ).toHaveText("The loom stood silent.");
});

test("Enter three times begins a new section after this one, at its level", async ({
  page,
}) => {
  // given: a book of nested sections open in the writing view
  await aNewProject(page, "Thrice");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Scene notes", under: "Chapter 1" },
  ]);
  await openTheWritingView(page);

  // when: Enter is pressed three times in a heading and a name typed
  await heading(page, "Chapter 1").click();
  await page.keyboard.press("Enter");
  // then: the first Enter begins a new scene
  await expect(page.locator(":focus")).toHaveAttribute(
    "aria-label",
    "A new scene",
  );
  await page.keyboard.press("Enter");
  // and: the second a new section
  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  await page.keyboard.type("Chapter 2");

  // and: the new section is saved after the one it follows, with everything inside it
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Chapter 1", "Scene notes", "Chapter 2", "Part Two"]);
  // and: shown there
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "Chapter 1",
    "Scene notes",
    "Chapter 2",
    "Part Two",
  ]);
  // and: at the same level
  await expect(
    flow(page).getByRole("heading", { name: "Chapter 2", level: 3 }),
    "a new section sits beside the one it follows, after everything nested inside it",
  ).toBeVisible();
  // and: its heading keeps the cursor
  await expect(heading(page, "Chapter 2")).toBeFocused();
});

test("a line begun by Enter and left empty goes away and makes nothing", async ({
  page,
}) => {
  // given: a line begun by Enter twice at a scene's end
  const scenes = await aBookInTheWritingView(page, "Abandoned", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("Only this.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(newScene(page)).toBeFocused();

  // when: the cursor leaves it empty
  await heading(page, "Part One").click();

  // then: the line goes
  await expect(newScene(page)).toHaveCount(0);
  // and: no scene is made
  await page.waitForTimeout(300);
  expect((await theBook(page))[0][1]).toEqual([scenes["Part One"][0]]);
});

test("up from a line begun by Enter goes back to the end of the scene", async ({
  page,
}) => {
  // given: a line begun by Enter twice at a scene's end
  const scenes = await aBookInTheWritingView(page, "Back", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");

  // when: the cursor is moved up and something typed
  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Still.");

  // then: it lands at the end of the scene
  await expect(scene(page, scenes["Part One"][0])).toHaveText(
    "The loom stood silent. Still.",
  );
  // and: the line goes
  await expect(newScene(page)).toHaveCount(0);
});

test("Enter on an empty line in the middle of a scene stays a paragraph", async ({
  page,
}) => {
  // given: the cursor at the end of a scene's first paragraph, with one more after it
  await aNewProject(page, "Middle");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const written = scene(page, scenes["Part One"][0]);
  await openTheWritingView(page);
  await written.click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Last.");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("End");

  // when: Enter is pressed twice
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");

  // then: no scene is begun
  await expect(newScene(page)).toHaveCount(0);
  // and: empty paragraphs are made
  await expect(written.locator("p")).toHaveCount(4);
});

test("Enter in a heading moves on into its section", async ({ page }) => {
  // given: the cursor in a section's heading
  const scenes = await aBookInTheWritingView(page, "Onward", { "Part One": 1 });
  await heading(page, "Part One").click();

  // when: Enter is pressed and something typed
  await page.keyboard.press("Enter");
  await page.keyboard.type("Straight in.");

  // then: it lands in the section's scene
  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Straight in.",
  );
});

test("a key pressed right after moving the cursor acts where the cursor now is", async ({
  page,
}) => {
  // given: the cursor at the end of a scene of two paragraphs
  await aNewProject(page, "Quick");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Last.");

  // when: the cursor is moved to the end of the first and Enter pressed at once
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");

  // then: the paragraph is split where the cursor now is
  await expect(
    scene(page, only).locator("p"),
    "the browser moves the cursor at once and tells the editor a moment later, so a fast Enter must not split where the cursor was",
  ).toHaveText(["First.", "", "Last."]);
});

test("nothing typed while a section's first scene is being made is lost", async ({
  page,
}) => {
  // given: an empty section whose first scene is slow to be made
  await aBookInTheWritingView(page, "HandoverOpening", { "Part One": 0 });
  await aSlowHandover(page);

  // when: its writing line is typed into
  await beginning(page).click();
  await page.keyboard.type(SENTENCE, { delay: 15 });

  // then: nothing typed is lost
  await expect(
    flow(page).locator(".scene .ProseMirror"),
    "the line typed into stays until the scene's own editor has its words and the cursor",
  ).toHaveText(SENTENCE);
});

test("nothing typed while the next scene is being made after Enter twice is lost", async ({
  page,
}) => {
  // given: a line begun by Enter twice at a scene's end
  await aNewProject(page, "HandoverPending");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  // and: the next scene slow to be made
  await aSlowHandover(page);

  // when: it is typed into
  await page.keyboard.type(SENTENCE, { delay: 15 });

  // then: nothing typed is lost
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveText([
    "First.",
    SENTENCE,
  ]);
});

test("moving the cursor just as the scene goes away raises no error", async ({
  page,
}) => {
  // given: every error on the page kept
  const failures: string[] = [];
  page.on("pageerror", (failure) => failures.push(failure.message));
  // and: the cursor in a scene of the writing view
  await aBookInTheWritingView(page, "Vanishing", { "Part One": 1 });
  const prose = flow(page).locator(".scene .ProseMirror").first();
  await prose.click();
  await page.keyboard.type("Some words to move about in.");
  await expect(prose).toHaveText("Some words to move about in.");

  // when: the cursor moves just as another view is opened
  await page.evaluate(() => {
    const words = document.querySelector(".scene .ProseMirror p")!.firstChild!;
    getSelection()!.collapse(words, 3);
    document.dispatchEvent(new Event("selectionchange"));
    document
      .querySelector<HTMLElement>(
        'nav[aria-label="Views"] a[href$="/outline"]',
      )!
      .click();
  });
  // then: the book is gone
  await expect(page.getByRole("article", { name: "The book" })).toHaveCount(0);
  await page.evaluate(() => new Promise((settled) => setTimeout(settled, 100)));

  // and: no error was raised
  expect(
    failures,
    "the cursor's last move is let go with the scene, not dispatched into it",
  ).toEqual([]);
});

test("Ctrl+Down goes to the end of the paragraph, then to the end of the next, then on into the next scene", async ({
  page,
}) => {
  // given: the cursor at the start of a scene of two paragraphs, with a scene after it
  await aNewProject(page, "ByParagraph");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("One two three.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Four five.");
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("ArrowRight");

  // when: Ctrl+Down is pressed
  await page.keyboard.press("Control+ArrowDown");
  // then: the cursor is at the end of the paragraph
  expect(await caretIn(page)).toBe("One two three. @14");

  // when: Ctrl+Down is pressed again
  await page.keyboard.press("Control+ArrowDown");
  // then: it is at the end of the next
  expect(await caretIn(page)).toBe("Four five. @10");

  // when: Ctrl+Down is pressed again
  await page.keyboard.press("Control+ArrowDown");
  // then: it is in the next scene
  await expect(scene(page, second)).toBeFocused();
});

test("Ctrl+Up goes to the start of the paragraph, then to the start of the one before", async ({
  page,
}) => {
  // given: the cursor at the end of a scene of two paragraphs
  await aNewProject(page, "ByParagraphUp");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("One two three.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Four five.");

  // when: Ctrl+Up is pressed
  await page.keyboard.press("Control+ArrowUp");
  // then: the cursor is at the start of the paragraph
  expect(await caretIn(page)).toBe("Four five. @0");

  // when: Ctrl+Up is pressed again
  await page.keyboard.press("Control+ArrowUp");
  // then: it is at the start of the one before
  expect(await caretIn(page)).toBe("One two three. @0");
});

test("Ctrl+Shift+Up selects back to the start of the paragraph, then of each one before, and on into whole scenes", async ({
  page,
}) => {
  // given: the cursor at the end of the second of two scenes, it of two paragraphs
  await aNewProject(page, "SelectingParagraphs");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("One two three.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Four five.");

  // when: Ctrl+Shift+Up is pressed
  await page.keyboard.press("Control+Shift+ArrowUp");
  // then: the paragraph is selected back to its start
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "Four five.",
  );

  // when: Ctrl+Shift+Up is pressed again
  await page.keyboard.press("Control+Shift+ArrowUp");
  // then: the paragraph before is taken in
  expect(
    (await page.evaluate(() => getSelection()!.toString())).replace(
      /\s+/g,
      " ",
    ),
  ).toBe("One two three. Four five.");

  // when: Ctrl+Shift+Up is pressed again
  await page.keyboard.press("Control+Shift+ArrowUp");
  // then: the scene above is taken in whole
  await expect
    .poll(
      () =>
        flow(page)
          .locator(".scene-slot.chosen")
          .evaluateAll((slots) =>
            slots.map((slot) => slot.getAttribute("data-place-of")),
          ),
      {
        message:
          "with the scene already selected whole, the next press takes in the one above",
      },
    )
    .toEqual([first, second]);
});
