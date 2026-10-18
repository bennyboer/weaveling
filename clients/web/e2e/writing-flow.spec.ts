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
  await aNewProject(page, "Writing");

  await openTheWritingView(page);

  await expect(page).toHaveURL(/\/projects\/[^/]+\/write$/);
  await expect(
    views(page).getByRole("link", { name: "Write", exact: true }),
  ).toHaveClass(/here/);
});

test("the book's sections read in order, nested as headings", async ({
  page,
}) => {
  await aNewProject(page, "Shaped");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "The Workshop", under: "Part One" },
    { title: "Upstairs", under: "The Workshop" },
    { title: "The River", under: "Part Two" },
  ]);

  await openTheWritingView(page);

  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "The Workshop",
    "Upstairs",
    "Part Two",
    "The River",
  ]);
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
  await aNewProject(page, "Nameless");
  await aBookShaped(page, [{ title: "" }]);

  await openTheWritingView(page);

  await expect(flow(page).getByRole("heading")).toHaveText([""]);
  await expect(flow(page).getByRole("heading")).toHaveAttribute(
    "data-placeholder",
    "Untitled section",
  );
});

test("the book survives a reload of the writing view", async ({ page }) => {
  await aNewProject(page, "Reloaded");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await page.reload();

  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});

test("each section's scenes follow its heading, a line between them", async ({
  page,
}) => {
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
  const scenes = await aBookInTheWritingView(page, "Kept", { "Part One": 1 });

  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
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

  await page.reload();

  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "The loom stood silent.",
  );
});

test("the arrow keys carry the cursor from one scene into the next and back", async ({
  page,
}) => {
  await aNewProject(page, "Crossing");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("She came back.");

  await expect(scene(page, second)).toContainText("She came back.");

  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Dust.");

  await expect(
    scene(page, first),
    "arriving from below lands at the end of the scene above",
  ).toContainText("The loom stood silent. Dust.");
});

test("the cursor crosses a section's heading on its way to the next section", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Sections", { "Part One": 1, "Part Two": 1 });
  await scene(page, scenes["Part One"][0]).click();

  await page.keyboard.press("ArrowRight");
  await expect(heading(page, "Part Two")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Across the heading.");

  await expect(scene(page, scenes["Part Two"][0])).toContainText(
    "Across the heading.",
  );
});

test("moving up inside a scene stays in it", async ({ page }) => {
  await aNewProject(page, "Staying");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("First line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second line.");

  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Still here.");

  await expect(scene(page, second)).toContainText("First line. Still here.");
  await expect(scene(page, first)).not.toContainText("Still here.");
});

test("an arrow key just after a scene takes focus is not undone, even on a busy page", async ({
  page,
}) => {
  await aNewProject(page, "Busy");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const written = scene(page, scenes["Part One"][0]);
  await openTheWritingView(page);
  await written.click();
  await page.keyboard.type("First line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Second line.");

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

  await expect(written.locator("p")).toHaveText([
    `First line.${"X".repeat(15)}`,
    "Second line.",
  ]);
});

test("the first scene's top and the last scene's end lead nowhere", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Ends", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();

  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Still the only scene.");

  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Still the only scene.",
  );
});

test("a word jump at a scene's edge carries on into the next scene", async ({
  page,
}) => {
  await aNewProject(page, "Jumping");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  await page.keyboard.press("ControlOrMeta+ArrowRight");
  await page.keyboard.type("Forward.");
  await expect(scene(page, second)).toContainText("Forward.");

  await page.keyboard.press("Home");
  await aFrame(page);
  await page.keyboard.press("ControlOrMeta+ArrowUp");
  await page.keyboard.type(" Back.");
  await expect(
    scene(page, second),
    "the jump should have left this scene",
  ).not.toContainText("Back.");
  await expect(scene(page, first)).toContainText("silent. Back.");
});

test("a word jump inside a scene stays a word jump", async ({ page }) => {
  await aNewProject(page, "Words");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("The loom stood silent");

  await page.keyboard.press("ControlOrMeta+ArrowLeft");
  await page.keyboard.type("very ");

  await expect(scene(page, second)).toContainText("stood very silent");
  await expect(scene(page, first)).not.toContainText("very");
});

test("words typed while the first section is being made are kept", async ({
  page,
}) => {
  await aNewProject(page, "Quick");
  await page.route("**/api/outlines/*/sections", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 600));
    await route.continue();
  });
  await openTheWritingView(page);

  await flow(page).getByRole("heading").click();
  await page.keyboard.type("The Workshop");

  await expect(flow(page).getByRole("heading")).toHaveText(["The Workshop"]);
  await expect.poll(() => savedTitles(page)).toEqual(["The Workshop"]);
});

test("a heading is renamed where it stands", async ({ page }) => {
  await aNewProject(page, "Renaming");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await heading(page, "Part One").click();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("The Beginning");
  await page.keyboard.press("Enter");

  await expect(flow(page).getByRole("heading")).toHaveText(["The Beginning"]);
  await expect.poll(() => savedTitles(page)).toEqual(["The Beginning"]);
});

test("a heading being typed is saved after a pause, even before leaving it", async ({
  page,
}) => {
  await aNewProject(page, "Pausing");
  await aBookShaped(page, [{ title: "Part" }]);
  await openTheWritingView(page);

  await heading(page, "Part").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" One");

  await expect.poll(() => savedTitles(page)).toEqual(["Part One"]);
  await expect(heading(page, "Part One")).toBeFocused();
});

test("Enter leaves a heading rather than breaking it", async ({ page }) => {
  await aNewProject(page, "Entering");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");

  await expect(heading(page, "Part One")).not.toBeFocused();
  expect(await heading(page, "Part One").textContent()).toBe("Part One");
});

test("a scene being written is left alone while its section is renamed", async ({
  page,
}) => {
  await aNewProject(page, "Undisturbed");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const written = scene(page, scenes["Part One"][0]);
  await openTheWritingView(page);
  await written.click();
  await page.keyboard.type("The loom stood silent.");
  await written.evaluate((editor) => {
    (editor as HTMLElement & { marked?: boolean }).marked = true;
  });

  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", Revised");
  await page.keyboard.press("Enter");
  await expect.poll(() => savedTitles(page)).toEqual(["Part One, Revised"]);

  expect(
    await written.evaluate(
      (editor) => (editor as HTMLElement & { marked?: boolean }).marked,
    ),
    "a renamed outline must not rebuild the scenes around it",
  ).toBe(true);
  await expect(written).toContainText("The loom stood silent.");
});

test("an empty scene can be seen and clicked into", async ({ page }) => {
  const scenes = await aBookInTheWritingView(page, "Empty", { "Part One": 1 });
  const empty = scene(page, scenes["Part One"][0]);

  await expect(empty).toBeVisible();
  expect(
    await empty.evaluate(
      (editor) =>
        getComputedStyle(editor.querySelector("p")!, "::before").content,
    ),
  ).toBe('"An empty scene. Start writing…"');
  expect((await empty.boundingBox())!.height).toBeGreaterThan(20);

  await empty.click();
  await page.keyboard.type("A first line.");

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
  const scenes = await aBookInTheWritingView(page, "Down", { "Part One": 1 });
  await heading(page, "Part One").click();

  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Below the heading.");
  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Below the heading.",
  );

  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await expect(heading(page, "Part One")).toBeFocused();
  await page.keyboard.type(", Revised");
  await page.keyboard.press("Enter");
  await expect.poll(() => savedTitles(page)).toEqual(["Part One, Revised"]);
});

test("an empty section's writing line sits between its heading and the next", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "Hollow", { "Part One": 0, "Part Two": 0 });
  await heading(page, "Part One").click();

  await page.keyboard.press("ArrowDown");
  await expect(beginning(page).first()).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(heading(page, "Part Two")).toBeFocused();
  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("ArrowUp");
  await expect(heading(page, "Part One")).toBeFocused();
});

test("left and right leave a heading only from its very edge", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Edges", { "Part One": 1, "Part Two": 0 });
  await heading(page, "Part Two").click();
  await page.keyboard.press("End");

  await page.keyboard.press("ArrowLeft");
  await expect(
    heading(page, "Part Two"),
    "inside a heading the arrow moves through its letters",
  ).toBeFocused();

  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowLeft");
  await page.keyboard.type("The end.");
  await expect(scene(page, scenes["Part One"][0])).toContainText("The end.");
});

test("typing into an empty section begins a scene there", async ({ page }) => {
  await aBookInTheWritingView(page, "Begun", { "Part One": 0 });
  await expect(beginning(page)).toHaveAttribute(
    "data-placeholder",
    "An empty scene. Start writing…",
  );

  await beginning(page).click();
  await page.keyboard.type("The loom stood silent.");

  await expect(beginning(page)).toHaveCount(0);
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent.",
  );
  await expect.poll(() => scenesOf(page)).toEqual([1]);
  await page.keyboard.type(" Still writing.");
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent. Still writing.",
  );
});

test("words typed while the scene is being made are kept", async ({ page }) => {
  await aNewProject(page, "Patient");
  await aBookWithScenes(page, { "Part One": 0 });
  await page.route("**/api/scenes", async (route) => {
    if (route.request().method() === "POST") {
      await new Promise((resolve) => setTimeout(resolve, 600));
    }
    await route.continue();
  });
  await openTheWritingView(page);

  await beginning(page).click();
  await page.keyboard.type("Nothing typed in the wait is lost.");

  await expect(flow(page).locator(".scene .ProseMirror")).toHaveText(
    "Nothing typed in the wait is lost.",
  );
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
  await aNewProject(page, "Blank");
  await openTheWritingView(page);
  const first = flow(page).getByRole("heading");
  await expect(first).toHaveAttribute(
    "data-placeholder",
    "Your book's first section",
  );

  await first.click();
  await page.keyboard.type("Part One");
  await expect
    .poll(() => savedTitles(page), {
      message: "typing into an empty book makes its first section",
    })
    .toEqual(["Part One"]);
  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("It began with a loom.");

  await expect
    .poll(async () => {
      const scene = (await projectsOutline(page)).sections[0].attachments[0]
        ?.id;

      return scene
        ? (await (await page.request.get(`${API}/scenes/${scene}`)).json()).text
        : "";
    })
    .toContain("It began with a loom.");
  await page.reload();
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "It began with a loom.",
  );
});

test("Enter twice at a scene's end begins the next scene right after it", async ({
  page,
}) => {
  await aNewProject(page, "Twice");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");

  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Morning came.");

  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(3);
  const order = (await theBook(page))[0][1];
  expect(order[0]).toBe(first);
  expect(order[2], "the new scene goes between, not at the end").toBe(second);
  await expect(scene(page, order[1])).toContainText("Morning came.");
  await expect(
    scene(page, first),
    "the empty paragraph left by the first Enter goes",
  ).toHaveText("The loom stood silent.");
});

test("Enter three times begins a new section after this one, at its level", async ({
  page,
}) => {
  await aNewProject(page, "Thrice");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Scene notes", under: "Chapter 1" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Chapter 1").click();
  await page.keyboard.press("Enter");
  await expect(page.locator(":focus")).toHaveAttribute(
    "aria-label",
    "A new scene",
  );
  await page.keyboard.press("Enter");
  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  await page.keyboard.type("Chapter 2");

  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Chapter 1", "Scene notes", "Chapter 2", "Part Two"]);
  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "Chapter 1",
    "Scene notes",
    "Chapter 2",
    "Part Two",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Chapter 2", level: 3 }),
    "a new section sits beside the one it follows, after everything nested inside it",
  ).toBeVisible();
  await expect(heading(page, "Chapter 2")).toBeFocused();
});

test("a line begun by Enter and left empty goes away and makes nothing", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Abandoned", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("Only this.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(newScene(page)).toBeFocused();

  await heading(page, "Part One").click();

  await expect(newScene(page)).toHaveCount(0);
  await page.waitForTimeout(300);
  expect((await theBook(page))[0][1]).toEqual([scenes["Part One"][0]]);
});

test("up from a line begun by Enter goes back to the end of the scene", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Back", { "Part One": 1 });
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");

  await page.keyboard.press("ArrowUp");
  await page.keyboard.type(" Still.");

  await expect(scene(page, scenes["Part One"][0])).toHaveText(
    "The loom stood silent. Still.",
  );
  await expect(newScene(page)).toHaveCount(0);
});

test("Enter on an empty line in the middle of a scene stays a paragraph", async ({
  page,
}) => {
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

  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");

  await expect(newScene(page)).toHaveCount(0);
  await expect(written.locator("p")).toHaveCount(4);
});

test("Enter in a heading moves on into its section", async ({ page }) => {
  const scenes = await aBookInTheWritingView(page, "Onward", { "Part One": 1 });
  await heading(page, "Part One").click();

  await page.keyboard.press("Enter");
  await page.keyboard.type("Straight in.");

  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Straight in.",
  );
});

test("a key pressed right after moving the cursor acts where the cursor now is", async ({
  page,
}) => {
  await aNewProject(page, "Quick");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Last.");

  await page.keyboard.press("ArrowUp");
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");

  await expect(
    scene(page, only).locator("p"),
    "the browser moves the cursor at once and tells the editor a moment later, so a fast Enter must not split where the cursor was",
  ).toHaveText(["First.", "", "Last."]);
});

test("nothing typed while a section's first scene is being made is lost", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "HandoverOpening", { "Part One": 0 });
  await aSlowHandover(page);

  await beginning(page).click();
  await page.keyboard.type(SENTENCE, { delay: 15 });

  await expect(
    flow(page).locator(".scene .ProseMirror"),
    "the line typed into stays until the scene's own editor has its words and the cursor",
  ).toHaveText(SENTENCE);
});

test("nothing typed while the next scene is being made after Enter twice is lost", async ({
  page,
}) => {
  await aNewProject(page, "HandoverPending");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await aSlowHandover(page);

  await page.keyboard.type(SENTENCE, { delay: 15 });

  await expect(flow(page).locator(".scene .ProseMirror")).toHaveText([
    "First.",
    SENTENCE,
  ]);
});

test("moving the cursor just as the scene goes away raises no error", async ({
  page,
}) => {
  const failures: string[] = [];
  page.on("pageerror", (failure) => failures.push(failure.message));
  await aBookInTheWritingView(page, "Vanishing", { "Part One": 1 });
  const prose = flow(page).locator(".scene .ProseMirror").first();
  await prose.click();
  await page.keyboard.type("Some words to move about in.");
  await expect(prose).toHaveText("Some words to move about in.");

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
  await expect(page.getByRole("article", { name: "The book" })).toHaveCount(0);
  await page.evaluate(() => new Promise((settled) => setTimeout(settled, 100)));

  expect(
    failures,
    "the cursor's last move is let go with the scene, not dispatched into it",
  ).toEqual([]);
});

test("Ctrl+Down goes to the end of the paragraph, then to the end of the next, then on into the next scene", async ({
  page,
}) => {
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

  await page.keyboard.press("Control+ArrowDown");
  expect(await caretIn(page)).toBe("One two three. @14");

  await page.keyboard.press("Control+ArrowDown");
  expect(await caretIn(page)).toBe("Four five. @10");

  await page.keyboard.press("Control+ArrowDown");
  await expect(scene(page, second)).toBeFocused();
});

test("Ctrl+Up goes to the start of the paragraph, then to the start of the one before", async ({
  page,
}) => {
  await aNewProject(page, "ByParagraphUp");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("One two three.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Four five.");

  await page.keyboard.press("Control+ArrowUp");
  expect(await caretIn(page)).toBe("Four five. @0");

  await page.keyboard.press("Control+ArrowUp");
  expect(await caretIn(page)).toBe("One two three. @0");
});

test("Ctrl+Shift+Up selects back to the start of the paragraph, then of each one before, and on into whole scenes", async ({
  page,
}) => {
  await aNewProject(page, "SelectingParagraphs");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("One two three.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Four five.");

  await page.keyboard.press("Control+Shift+ArrowUp");
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "Four five.",
  );

  await page.keyboard.press("Control+Shift+ArrowUp");
  expect(
    (await page.evaluate(() => getSelection()!.toString())).replace(
      /\s+/g,
      " ",
    ),
  ).toBe("One two three. Four five.");

  await page.keyboard.press("Control+Shift+ArrowUp");
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
