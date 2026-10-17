import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import {
  aLooseScene,
  sceneCatalogued,
  aNewProject,
  openTheOutline,
  views,
} from "./support/shell";

const API = "http://127.0.0.1:3000/api";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

async function openTheWritingView(page: Page) {
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();
}

async function aFrame(page: Page) {
  await page.evaluate(
    () =>
      new Promise((settled) =>
        requestAnimationFrame(() => setTimeout(settled)),
      ),
  );
}

const scene = (page: Page, id: string) =>
  flow(page).locator(`[data-scene="${id}"] .ProseMirror`);

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

const projectOf = (page: Page) =>
  new URL(page.url()).pathname.split("/")[2].split("-").pop();

async function projectsOutline(page: Page) {
  const project = projectOf(page);
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });

  return opened.json();
}

async function cataloguedOpening(page: Page, scene: string) {
  const listed = await page.request.get(
    `${API}/scenes?project=${projectOf(page)}`,
  );
  const { scenes } = await listed.json();

  return (
    scenes.find((held: { id: string }) => held.id === scene)?.opening ?? ""
  );
}

const savedTitles = async (page: Page) =>
  (await projectsOutline(page)).sections.map(
    (section: { title: string }) => section.title,
  );

const beginning = (page: Page) =>
  flow(page).getByRole("textbox", { name: "A new scene" });

const scenesOf = async (page: Page) =>
  (await projectsOutline(page)).sections.map(
    (section: { title: string; attachments: { kind: string }[] }) =>
      section.attachments.filter((attached) => attached.kind === "scene")
        .length,
  );

const sceneOrder = async (page: Page) =>
  (await projectsOutline(page)).sections.map(
    (section: {
      title: string;
      attachments: { kind: string; id: string }[];
    }) => [
      section.title,
      section.attachments
        .filter((attached) => attached.kind === "scene")
        .map((attached) => attached.id),
    ],
  );

const newScene = (page: Page) =>
  flow(page).getByRole("textbox", { name: "A new scene" });

const heading = (page: Page, named: string) =>
  flow(page).getByRole("heading", { name: named });

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
  await aNewProject(page, "Scenes");
  const scenes = await aBookWithScenes(page, { "Part One": 2, "Part Two": 1 });

  await openTheWritingView(page);

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
  await aNewProject(page, "Kept");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

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
  await aNewProject(page, "Sections");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Ends");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
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

test("an empty book starts with its first heading", async ({ page }) => {
  await aNewProject(page, "Beginning");
  await openTheWritingView(page);
  const first = flow(page).getByRole("heading");
  await expect(first).toHaveAttribute(
    "data-placeholder",
    "Your book's first section",
  );

  await first.click();
  await page.keyboard.type("Part One");
  await page.keyboard.press("Enter");

  await expect
    .poll(() => savedTitles(page), {
      message: "typing into an empty book makes its first section",
    })
    .toEqual(["Part One"]);
  await page.reload();
  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
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
  await aNewProject(page, "Empty");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Down");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Hollow");
  await aBookWithScenes(page, { "Part One": 0, "Part Two": 0 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Edges");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 0 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Begun");
  await aBookWithScenes(page, { "Part One": 0 });
  await openTheWritingView(page);
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

  await flow(page).getByRole("heading").click();
  await page.keyboard.type("Part One");
  await expect.poll(() => savedTitles(page)).toEqual(["Part One"]);
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

  await expect.poll(async () => (await sceneOrder(page))[0][1].length).toBe(3);
  const order = (await sceneOrder(page))[0][1];
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
  await aNewProject(page, "Abandoned");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("Only this.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(newScene(page)).toBeFocused();

  await heading(page, "Part One").click();

  await expect(newScene(page)).toHaveCount(0);
  await page.waitForTimeout(300);
  expect((await sceneOrder(page))[0][1]).toEqual([scenes["Part One"][0]]);
});

test("up from a line begun by Enter goes back to the end of the scene", async ({
  page,
}) => {
  await aNewProject(page, "Back");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
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
  await aNewProject(page, "Onward");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await heading(page, "Part One").click();

  await page.keyboard.press("Enter");
  await page.keyboard.type("Straight in.");

  await expect(scene(page, scenes["Part One"][0])).toContainText(
    "Straight in.",
  );
});

const label = (page: Page, id: string) =>
  flow(page).locator(`.scene-row:has([data-scene="${id}"]) .scene-label`);

async function titled(page: Page, scene: string, title: string) {
  const renamed = await page.request.patch(`${API}/scenes/${scene}`, {
    data: { title },
  });
  expect(renamed.ok()).toBe(true);
  await sceneCatalogued(page, scene, title);
}

test("a scene's title stands in the margin, level with the scene", async ({
  page,
}) => {
  await aNewProject(page, "Margin");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  await openTheWritingView(page);

  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  await expect(label(page, scenes["Part One"][0])).not.toHaveClass(/untitled/);
  const shown = (await label(page, scenes["Part One"][0]).boundingBox())!;
  const written = (await scene(page, scenes["Part One"][0]).boundingBox())!;
  expect(Math.abs(shown.y - written.y)).toBeLessThanOrEqual(4);
  expect(
    shown.x + shown.width,
    "the margin keeps the text column to itself",
  ).toBeLessThan(written.x);
});

test("an untitled scene shows its opening words, and they follow the writing", async ({
  page,
}) => {
  await aNewProject(page, "Opening");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  const shown = label(page, scenes["Part One"][0]);
  await expect(shown).toHaveText("");

  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");

  await expect(shown).toHaveText("The loom stood silent.");
  await expect(shown).toHaveClass(/untitled/);
  await page.keyboard.type(" It had stood so since spring, under dust.");
  await expect(shown, "a margin holds a few words, not the scene").toHaveText(
    "The loom stood silent. It had stood so s…",
  );
});

test("on a narrow window the margin folds above its scene", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 900 });
  await aNewProject(page, "Narrow");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  await openTheWritingView(page);

  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  const shown = (await label(page, scenes["Part One"][0]).boundingBox())!;
  const written = (await scene(page, scenes["Part One"][0]).boundingBox())!;
  expect(shown.y + shown.height).toBeLessThanOrEqual(written.y + 1);
  expect(Math.abs(shown.x - written.x)).toBeLessThanOrEqual(2);
});

test("a scene's title in the margin is only to be read", async ({ page }) => {
  await aNewProject(page, "ReadOnly");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  await openTheWritingView(page);

  await label(page, scenes["Part One"][0]).click();
  await page.keyboard.type("x");

  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  await expect(label(page, scenes["Part One"][0])).not.toHaveAttribute(
    "contenteditable",
  );
});

const sceneActions = (page: Page, id: string) =>
  flow(page)
    .locator(`.scene-row:has([data-scene="${id}"])`)
    .getByRole("button", { name: "Scene actions" });

const sectionActions = (page: Page, named: string) =>
  flow(page)
    .locator(".heading-row")
    .filter({ has: page.getByRole("heading", { name: named }) })
    .getByRole("button", { name: "Section actions" });

const menuItem = (page: Page, named: string) =>
  flow(page).getByRole("menuitem", { name: named });

const asked = (page: Page) => page.getByRole("dialog");

async function sceneOnServer(page: Page, id: string) {
  return page.request.get(`${API}/scenes/${id}`);
}

test("a scene is renamed from its menu, right where its title stands", async ({
  page,
}) => {
  await aNewProject(page, "Renaming");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await titled(page, arrival, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, arrival)).toHaveText("The arrival");

  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  const field = flow(page).getByRole("textbox", { name: "Scene title" });
  await expect(field).toBeFocused();
  await expect(field).toHaveValue("The arrival");
  await field.fill("The departure");
  await page.keyboard.press("Enter");

  await expect(label(page, arrival)).toHaveText("The departure");
  await expect(field).toHaveCount(0);
  await expect
    .poll(async () => (await (await sceneOnServer(page, arrival)).json()).title)
    .toBe("The departure");
});

test("Escape leaves a scene's title as it was", async ({ page }) => {
  await aNewProject(page, "KeepTitle");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await titled(page, arrival, "The arrival");
  await openTheWritingView(page);

  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Something else");
  await page.keyboard.press("Escape");

  await expect(label(page, arrival)).toHaveText("The arrival");
  expect((await (await sceneOnServer(page, arrival)).json()).title).toBe(
    "The arrival",
  );
});

test("a scene taken out of the book leaves the flow but is kept", async ({
  page,
}) => {
  await aNewProject(page, "TakeOut");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();

  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();

  await expect(scene(page, first)).toHaveCount(0);
  await expect(scene(page, second)).toBeVisible();
  await expect
    .poll(() => sceneOrder(page), {
      message: "the flow lets go of the scene before the server has it out",
    })
    .toEqual([["Part One", [second]]]);
  expect((await sceneOnServer(page, first)).status()).toBe(200);
});

test("deleting a scene asks first, and Cancel keeps it", async ({ page }) => {
  await aNewProject(page, "DeleteCancel");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await titled(page, only, "The arrival");
  await openTheWritingView(page);

  await sceneActions(page, only).click();
  await menuItem(page, "Delete scene").click();
  await expect(asked(page)).toContainText("“The arrival”");
  await asked(page).getByRole("button", { name: "Cancel" }).click();

  await expect(asked(page)).toHaveCount(0);
  await expect(scene(page, only)).toBeVisible();
  expect((await sceneOnServer(page, only)).status()).toBe(200);
});

test("a deleted scene is gone from the book and for good", async ({ page }) => {
  await aNewProject(page, "DeleteScene");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();

  await sceneActions(page, first).click();
  await menuItem(page, "Delete scene").click();
  await asked(page).getByRole("button", { name: "Delete" }).click();

  await expect(scene(page, first)).toHaveCount(0);
  await expect(scene(page, second)).toBeVisible();
  expect(await sceneOrder(page)).toEqual([["Part One", [second]]]);
  expect((await sceneOnServer(page, first)).status()).toBe(404);
});

test("an empty section is removed at once", async ({ page }) => {
  await aNewProject(page, "RemoveEmpty");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);
  await expect(heading(page, "Part Two")).toBeVisible();

  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();

  await expect(heading(page, "Part Two")).toHaveCount(0);
  await expect(asked(page)).toHaveCount(0);
  expect(await savedTitles(page)).toEqual(["Part One"]);
});

test("removing a section that holds scenes asks first, and keeps the scenes", async ({
  page,
}) => {
  await aNewProject(page, "RemoveHolding");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [kept] = scenes["Part Two"];
  await openTheWritingView(page);
  await expect(scene(page, kept)).toBeVisible();

  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();
  await expect(asked(page)).toContainText("Remove this section?");
  await asked(page).getByRole("button", { name: "Remove" }).click();

  await expect(heading(page, "Part Two")).toHaveCount(0);
  await expect(scene(page, kept)).toHaveCount(0);
  expect(await savedTitles(page)).toEqual(["Part One"]);
  expect((await sceneOnServer(page, kept)).status()).toBe(200);
});

test("a margin menu closes on Escape and on a click elsewhere", async ({
  page,
}) => {
  await aNewProject(page, "MenuCloses");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  await sceneActions(page, only).click();
  await expect(menuItem(page, "Rename")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(menuItem(page, "Rename")).toHaveCount(0);

  await sceneActions(page, only).click();
  await expect(menuItem(page, "Rename")).toBeVisible();
  await heading(page, "Part One").click();
  await expect(menuItem(page, "Rename")).toHaveCount(0);
  await expect(sceneActions(page, only)).toHaveAttribute(
    "aria-expanded",
    "false",
  );
});

test("Shift+Tab from a scene goes back to its heading, past the margin", async ({
  page,
}) => {
  await aNewProject(page, "TabBack");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.press("Shift+Tab");

  await expect(heading(page, "Part One")).toBeFocused();
});

test("Shift+F10 opens the menu of the scene being written, and Escape goes back to it", async ({
  page,
}) => {
  await aNewProject(page, "MenuKeys");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  await scene(page, only).click();
  await page.keyboard.press("Shift+F10");
  await expect(menuItem(page, "Rename")).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(menuItem(page, "Take out of the book")).toBeFocused();
  await page.keyboard.press("Escape");

  await expect(menuItem(page, "Rename")).toHaveCount(0);
  await expect(scene(page, only)).toBeFocused();
});

test("Shift+F10 in a heading opens its section's menu", async ({ page }) => {
  await aNewProject(page, "HeadingKeys");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await heading(page, "Part One").click();
  await page.keyboard.press("Shift+F10");

  await expect(menuItem(page, "Remove section")).toBeFocused();
});

test("a scene renamed from the keyboard hands the cursor back to the scene", async ({
  page,
}) => {
  await aNewProject(page, "RenameKeys");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  await scene(page, only).click();
  await page.keyboard.press("Shift+F10");
  await page.keyboard.press("Enter");
  await page.keyboard.type("The arrival");
  await page.keyboard.press("Enter");

  await expect(label(page, only)).toHaveText("The arrival");
  await expect(scene(page, only)).toBeFocused();
});

test("the margin lines up with the letters beside it", async ({ page }) => {
  await aNewProject(page, "LinedUp");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Einleitung" },
    { title: "Kapitel eins", under: "Einleitung" },
    { title: "Die Ankunft", under: "Kapitel eins" },
  ]);
  const placed = await aLooseScene(page);
  await titled(page, placed, "fiwjefwe");
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: placed },
      section: ids["Einleitung"],
      after: null,
    },
  });
  await openTheWritingView(page);
  await expect(label(page, placed)).toHaveText("fiwjefwe");
  await scene(page, placed).click();
  await page.keyboard.type("Hier ist Text");

  const measured = await flow(page).evaluate(async (book) => {
    await document.fonts.ready;
    const pen = document.createElement("canvas").getContext("2d")!;
    const letters = (shown: Element) => {
      const style = getComputedStyle(shown);
      pen.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const font = pen.measureText("Hx");
      const cap = pen.measureText("H").actualBoundingBoxAscent;
      const line = parseFloat(style.lineHeight);
      const top = shown.getBoundingClientRect().top;
      const baseline =
        top +
        (line - font.fontBoundingBoxAscent - font.fontBoundingBoxDescent) / 2 +
        font.fontBoundingBoxAscent;

      return { baseline, capitals: baseline - cap / 2 };
    };
    const middle = (shown: Element) => {
      const box = shown.getBoundingClientRect();

      return box.top + box.height / 2;
    };
    const bar = (shown: Element) => {
      const box = shown.getBoundingClientRect();
      const drawn = getComputedStyle(shown, "::after");

      return (
        box.top +
        (parseFloat(drawn.top) + box.height - parseFloat(drawn.bottom)) / 2
      );
    };

    return {
      headings: [...book.querySelectorAll(".heading-row")].map((row) => ({
        capitals: letters(row.querySelector(".heading")!).capitals,
        button: middle(row.querySelector(".menu-button")!),
        bar: bar(row.querySelector(".heading")!),
      })),
      scene: (() => {
        const row = book.querySelector(".scene-row")!;
        const prose = letters(row.querySelector(".ProseMirror p")!);

        return {
          label: letters(row.querySelector(".scene-label")!).baseline,
          prose: prose.baseline,
          capitals: prose.capitals,
          button: middle(row.querySelector(".menu-button")!),
        };
      })(),
    };
  });

  for (const { capitals, button, bar } of measured.headings) {
    expect(
      Math.abs(button - capitals),
      "a heading's ⋮ sits on its capitals",
    ).toBeLessThanOrEqual(1);
    expect(
      Math.abs(bar - capitals),
      "a heading's bar is centred on its capitals",
    ).toBeLessThanOrEqual(1);
  }
  expect(
    Math.abs(measured.scene.label - measured.scene.prose),
    "a scene's title stands on the baseline of its first line",
  ).toBeLessThanOrEqual(1);
  expect(
    Math.abs(measured.scene.button - measured.scene.capitals),
    "a scene's ⋮ sits on the capitals of its first line",
  ).toBeLessThanOrEqual(1);
});

test("an open margin menu lies above the titles beside the scenes", async ({
  page,
}) => {
  await aNewProject(page, "OnTop");
  const scenes = await aBookWithScenes(page, { Einleitung: 1 });
  const [only] = scenes["Einleitung"];
  await titled(page, only, "A title long enough to reach under the menu");
  await openTheWritingView(page);
  await expect(label(page, only)).toContainText("A title long enough");

  await sectionActions(page, "Einleitung").click();
  const offered = menuItem(page, "Remove section");
  await expect(offered).toBeVisible();

  const onTop = await flow(page)
    .getByRole("menu")
    .evaluate((menu) => {
      const box = menu.getBoundingClientRect();
      const corners = [
        [box.left + 12, box.top + 12],
        [box.right - 12, box.top + 12],
        [box.left + 12, box.bottom - 12],
        [box.right - 12, box.bottom - 12],
      ];

      return corners.every(([x, y]) =>
        menu.contains(document.elementFromPoint(x, y)),
      );
    });
  expect(onTop, "nothing else may draw over the menu").toBe(true);
});

test("the writing view's dialogs speak in the same type as every other dialog", async ({
  page,
}) => {
  await aNewProject(page, "SameType");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  await sceneActions(page, scenes["Part One"][0]).click();
  await menuItem(page, "Delete scene").click();
  const told = asked(page).locator("p");
  await expect(told).toBeVisible();

  const type = await told.evaluate((shown) => {
    const style = getComputedStyle(shown);

    return { size: style.fontSize, weight: style.fontWeight };
  });
  const everywhere = await page.evaluate(() => {
    const style = getComputedStyle(document.body);

    return { size: style.fontSize, weight: style.fontWeight };
  });
  expect(type, "a dialog is not part of the book's text").toEqual(everywhere);
});

const gapBelow = (page: Page, id: string) =>
  flow(page).locator(`.insertion[data-after="${id}"]`);

const gapAtStartOf = (page: Page, named: string) =>
  flow(page)
    .locator(".in-flow")
    .filter({ has: page.getByRole("heading", { name: named }) })
    .locator(".insertion:not(.below-a-scene)");

const addHere = (gap: ReturnType<Page["locator"]>) =>
  gap.getByRole("button", { name: "Add here" });

const bringingBack = (page: Page) =>
  page.getByRole("dialog", { name: "Bring back a scene" });

test("a scene taken out of the book can be brought back below another", async ({
  page,
}) => {
  await aNewProject(page, "BringBack");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await titled(page, first, "The arrival");
  await titled(page, second, "The storm");
  await openTheWritingView(page);

  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();
  await expect(scene(page, first)).toHaveCount(0);

  await addHere(gapBelow(page, second)).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(
    bringingBack(page).getByRole("button"),
    "only scenes outside the book are offered",
  ).toHaveText(["The arrival", "Cancel"]);
  await bringingBack(page).getByRole("button", { name: "The arrival" }).click();

  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, first)).toBeVisible();
  expect(await sceneOrder(page)).toEqual([["Part One", [second, first]]]);
});

test("a heading's + brings a scene back to the start of its section", async ({
  page,
}) => {
  await aNewProject(page, "BringBackFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  const loose = await aLooseScene(page);
  await titled(page, loose, "The prologue");
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "Bring back a scene…").click();
  await bringingBack(page)
    .getByRole("button", { name: "The prologue" })
    .click();

  await expect(scene(page, loose)).toBeVisible();
  expect(await sceneOrder(page)).toEqual([["Part One", [loose, placed]]]);
});

test("with every scene in the book there is nothing to bring back", async ({
  page,
}) => {
  await aNewProject(page, "NothingBack");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(bringingBack(page)).toContainText(
    "Every scene is in the book already.",
  );
  await bringingBack(page).getByRole("button", { name: "Cancel" }).click();

  await expect(bringingBack(page)).toHaveCount(0);
});

test("a gap's + shows only when the pointer comes near the gap", async ({
  page,
}) => {
  await aNewProject(page, "NearTheGap");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  const plus = addHere(gapBelow(page, first));

  await scene(page, first).hover();
  await expect(plus).toHaveCSS("opacity", "0");
  await expect(addHere(gapBelow(page, second))).toHaveCSS("opacity", "0");

  await gapBelow(page, first).locator(".reach").hover();
  await expect(plus).toHaveCSS("opacity", "1");
});

test("the gap between two scenes leaves the writing clickable", async ({
  page,
}) => {
  await aNewProject(page, "ClickThrough");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  for (const id of [first, second]) {
    const written = (await scene(page, id).boundingBox())!;
    const hit = await page.evaluate(
      ([x, y]) => document.elementFromPoint(x, y)?.closest(".reach") === null,
      [written.x + 20, written.y + 4],
    );
    expect(hit, "a scene's first line belongs to the scene").toBe(true);
  }
});

async function aBookWithLooseScenes(page: Page, titles: string[]) {
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const loose: Record<string, string> = {};

  for (const title of titles) {
    loose[title] = await aLooseScene(page);
    await titled(page, loose[title], title);
  }

  return { placed: scenes["Part One"][0], loose };
}

const offered = (page: Page) =>
  bringingBack(page)
    .getByRole("list", { name: "Scenes not in the book" })
    .getByRole("button");

test("the scenes to bring back can be narrowed by what they are called", async ({
  page,
}) => {
  await aNewProject(page, "FindScene");
  const { placed } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
    "The stranger",
  ]);
  await openTheWritingView(page);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  const finding = bringingBack(page).getByRole("searchbox", {
    name: "Find a scene",
  });
  await expect(finding).toBeFocused();
  await expect(offered(page)).toHaveCount(3);

  await page.keyboard.type("the st");
  await expect(offered(page)).toHaveText(["The storm", "The stranger"]);
  await page.keyboard.type("orm");
  await expect(offered(page)).toHaveText(["The storm"]);
  await finding.fill("nothing like it");
  await expect(bringingBack(page)).toContainText("No scene matches.");
});

test("Enter in the search brings back the first scene that matches", async ({
  page,
}) => {
  await aNewProject(page, "EnterBack");
  const { placed, loose } = await aBookWithLooseScenes(page, [
    "The storm",
    "A letter",
  ]);
  await openTheWritingView(page);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "Bring back a scene…").click();
  await expect(
    bringingBack(page).getByRole("searchbox", { name: "Find a scene" }),
  ).toBeFocused();
  await page.keyboard.type("letter");
  await page.keyboard.press("Enter");

  await expect(bringingBack(page)).toHaveCount(0);
  await expect(scene(page, loose["A letter"])).toBeVisible();
  expect(await sceneOrder(page)).toEqual([
    ["Part One", [placed, loose["A letter"]]],
  ]);
});

test("New scene in the gap between two scenes begins one right there", async ({
  page,
}) => {
  await aNewProject(page, "NewBetween");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Mitten drin.");

  await expect.poll(async () => (await sceneOrder(page))[0][1].length).toBe(3);
  const made = (await sceneOrder(page))[0][1][1];
  expect((await sceneOrder(page))[0][1]).toEqual([first, made, second]);
  await expect(scene(page, made)).toHaveText("Mitten drin.");
});

test("New scene at a section's start begins one before its first scene", async ({
  page,
}) => {
  await aNewProject(page, "NewFirst");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [placed] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await page.keyboard.type("Ganz vorne.");

  await expect.poll(async () => (await sceneOrder(page))[0][1].length).toBe(2);
  const [made, after] = (await sceneOrder(page))[0][1];
  expect(after).toBe(placed);
  await expect(scene(page, made)).toHaveText("Ganz vorne.");
});

test("New scene in an empty section writes into its opening line", async ({
  page,
}) => {
  await aNewProject(page, "NewEmpty");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await addHere(gapAtStartOf(page, "Part One")).click();
  await menuItem(page, "New scene").click();

  await expect(newScene(page)).toHaveCount(1);
  await expect(newScene(page)).toBeFocused();
});

test("a new scene left empty goes away and makes nothing", async ({ page }) => {
  await aNewProject(page, "NewLeftEmpty");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await menuItem(page, "New scene").click();
  await expect(newScene(page)).toBeFocused();
  await heading(page, "Part One").click();

  await expect(newScene(page)).toHaveCount(0);
  expect(await sceneOrder(page)).toEqual([["Part One", [first, second]]]);
});

const bookStart = (page: Page) => flow(page).locator(".insertion.book-start");

const bookEnd = (page: Page) => flow(page).locator(".insertion.book-end");

async function aNewSectionNamed(page: Page, title: string) {
  await expect(page.locator(":focus")).toHaveAttribute("role", "heading");
  await page.keyboard.type(title);
  await expect.poll(() => savedTitles(page)).toContain(title);
}

test("New section after a section's last scene starts the next one at its level", async ({
  page,
}) => {
  await aNewProject(page, "NewSectionAfter");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 0 });
  await openTheWritingView(page);

  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Interlude");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Interlude",
    "Part Two",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Interlude", level: 2 }),
  ).toBeVisible();
});

test("New section after the scenes of a section with subsections starts its first subsection", async ({
  page,
}) => {
  await aNewProject(page, "NewSubsection");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  const placed = await aLooseScene(page);
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: placed },
      section: ids["Part One"],
      after: null,
    },
  });
  await openTheWritingView(page);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Prelude");

  expect(await savedTitles(page)).toEqual(["Part One", "Prelude", "Chapter 1"]);
  await expect(
    flow(page).getByRole("heading", { name: "Prelude", level: 3 }),
    "it sits where the gap was, so it nests beside the subsections",
  ).toBeVisible();
});

test("New section is offered only where nothing has to move", async ({
  page,
}) => {
  await aNewProject(page, "OnlyAtTheEnd");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);

  await addHere(gapBelow(page, first)).click();
  await expect(menuItem(page, "New scene")).toBeVisible();
  await expect(menuItem(page, "New section")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await addHere(gapAtStartOf(page, "Part One")).click();
  await expect(menuItem(page, "New section")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await addHere(gapBelow(page, second)).click();
  await expect(menuItem(page, "New section")).toBeVisible();
});

test("before the first section, New section begins the book", async ({
  page,
}) => {
  await aNewProject(page, "NewFirstSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await addHere(bookStart(page)).click();
  await expect(menuItem(page, "New scene")).toHaveCount(0);
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Prologue");

  expect(await savedTitles(page)).toEqual([
    "Prologue",
    "Part One",
    "Chapter 1",
  ]);
});

test("after everything, New section ends the book at the top level", async ({
  page,
}) => {
  await aNewProject(page, "NewLastSection");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await addHere(bookEnd(page)).click();
  await menuItem(page, "New section").click();
  await aNewSectionNamed(page, "Epilogue");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Epilogue", level: 2 }),
  ).toBeVisible();
});

test("a book ending in a scene has one gap at its end, not two", async ({
  page,
}) => {
  await aNewProject(page, "OneGapAtTheEnd");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await expect(scene(page, scenes["Part One"][0])).toBeVisible();

  await expect(bookEnd(page)).toHaveCount(0);
  await addHere(gapBelow(page, scenes["Part One"][0])).click();
  await expect(menuItem(page, "New section")).toBeVisible();
  await expect(
    menuItem(page, "New top-level section"),
    "a section at its level already is one",
  ).toHaveCount(0);
});

test("the last gap of a book ending in a nested section can still end the book at the top level", async ({
  page,
}) => {
  await aNewProject(page, "NestedEnd");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  const placed = await aLooseScene(page);
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: placed },
      section: ids["Chapter 1"],
      after: null,
    },
  });
  await openTheWritingView(page);
  await expect(scene(page, placed)).toBeVisible();
  await expect(bookEnd(page)).toHaveCount(0);

  await addHere(gapBelow(page, placed)).click();
  await menuItem(page, "New top-level section").click();
  await aNewSectionNamed(page, "Epilogue");

  expect(await savedTitles(page)).toEqual([
    "Part One",
    "Chapter 1",
    "Epilogue",
  ]);
  await expect(headingAt(page, "Epilogue", 2)).toBeVisible();
});

const headingAt = (page: Page, named: string, level: number) =>
  flow(page).getByRole("heading", { name: named, level });

test("Tab in a heading nests its section under the one before, and the cursor stays", async ({
  page,
}) => {
  await aNewProject(page, "TabNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Part Two").click();
  await page.keyboard.press("Tab");

  await expect(headingAt(page, "Part Two", 3)).toBeVisible();
  await expect(headingAt(page, "Part Two", 3)).toBeFocused();
  await expect
    .poll(async () =>
      (await projectsOutline(page)).sections.map(
        (section: { title: string; parent: string | null }) => [
          section.title,
          section.parent !== null,
        ],
      ),
    )
    .toEqual([
      ["Part One", false],
      ["Part Two", true],
    ]);
});

test("Shift+Tab in a heading lifts its section out a level", async ({
  page,
}) => {
  await aNewProject(page, "ShiftTabLifts");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Chapter 1").click();
  await page.keyboard.press("Shift+Tab");

  await expect(headingAt(page, "Chapter 1", 2)).toBeVisible();
  await expect(headingAt(page, "Chapter 1", 2)).toBeFocused();
});

test("a heading's title being typed survives the heading being nested", async ({
  page,
}) => {
  await aNewProject(page, "TypedThenTab");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await heading(page, "Part Two").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" and more");
  await page.keyboard.press("Tab");

  await expect(headingAt(page, "Part Two and more", 3)).toBeVisible();
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two and more"]);
});

test("a section's menu nests and lifts it, offering only what is possible", async ({
  page,
}) => {
  await aNewProject(page, "MenuNests");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);

  await sectionActions(page, "Part One").click();
  await expect(menuItem(page, "Remove section")).toBeVisible();
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  await expect(menuItem(page, "Lift out a level")).toHaveCount(0);
  await page.keyboard.press("Escape");

  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Nest under the previous section").click();
  await expect(headingAt(page, "Part Two", 3)).toBeVisible();

  await sectionActions(page, "Part Two").click();
  await expect(menuItem(page, "Nest under the previous section")).toHaveCount(
    0,
  );
  await menuItem(page, "Lift out a level").click();
  await expect(headingAt(page, "Part Two", 2)).toBeVisible();
});

const statusNote = (page: Page) => page.locator(".status-note");

const sceneCarried = (frame: string | Buffer) => {
  const bytes = Buffer.from(frame);
  const length = bytes[1];

  return bytes.subarray(2, 2 + length).toString();
};

const goingBack = (page: Page) =>
  page.getByRole("button", { name: /Back to where you left off/ });

async function backToWhereILeftOff(page: Page) {
  await page
    .getByRole("button", { name: /Back to where you left off/ })
    .click();
}

const saveState = (page: Page) => statusNote(page).locator(".save-state");

async function holdingWhatIsWritten(page: Page) {
  const held: (string | Buffer)[] = [];
  const servers: { send: (frame: string | Buffer) => void }[] = [];
  let holding = false;

  await page.routeWebSocket(/\/api\/sync$/, (socket) => {
    const server = socket.connectToServer();
    servers.push(server);

    socket.onMessage((frame) =>
      holding ? held.push(frame) : server.send(frame),
    );
  });

  return {
    hold: () => {
      holding = true;
    },
    held: () => held.length,
    release: () => {
      holding = false;
      held.splice(0).forEach((frame) => servers.at(-1)!.send(frame));
    },
  };
}

test("the status note sits in the status bar, at its right", async ({
  page,
}) => {
  await aNewProject(page, "Noted");
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  const note = (await statusNote(page).boundingBox())!;
  const bar = (await page.locator(".status-bar").boundingBox())!;
  const wide = page.viewportSize()!.width;

  expect(note.y).toBeGreaterThanOrEqual(bar.y);
  expect(note.y + note.height).toBeLessThanOrEqual(bar.y + bar.height + 0.5);
  expect(wide - (note.x + note.width)).toBeLessThanOrEqual(16);
});

test("the status note makes room for the alarm beside it", async ({ page }) => {
  await page.route("**/api/service/refusals", (route) =>
    route.fulfill({
      json: [
        {
          id: 1,
          listener: "catalogue-idea",
          routing: "idea.captured",
          attempts: 5,
          why: "the projection had not landed",
          plainly: "A change to an idea did not reach the list of ideas.",
          occurred_at: "2026-10-10T09:00:00Z",
          given_up_at: "2026-10-10T09:01:00Z",
          acknowledged_at: null,
        },
      ],
    }),
  );
  await aNewProject(page, "Alarmed");
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  const alarm = page.getByRole("button", {
    name: "Changes that could not be applied",
  });
  await expect(alarm).toBeVisible();

  const note = (await statusNote(page).boundingBox())!;
  const raised = (await alarm.boundingBox())!;

  expect(note.x + note.width).toBeLessThan(raised.x);
});

test("the status note says Saving… until the server has stored what was written", async ({
  page,
}) => {
  const sync = await holdingWhatIsWritten(page);
  await aNewProject(page, "Saving");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  const written = scenes["Part One"][0];
  await expect(saveState(page)).toHaveText("All saved");

  sync.hold();
  await scene(page, written).click();
  await page.keyboard.type("Held back.");
  await expect(saveState(page)).toHaveText("Saving…");
  await expect.poll(sync.held).toBeGreaterThan(0);
  await page.waitForTimeout(1000);
  await expect(
    saveState(page),
    "nothing written has reached the server yet",
  ).toHaveText("Saving…");

  sync.release();
  await expect(saveState(page)).toHaveText("All saved");
  const stored = await (
    await page.request.get(`${API}/scenes/${written}`)
  ).json();
  expect(stored.text, "All saved only once it is stored").toContain(
    "Held back.",
  );
});

test("a scene begun by typing is all saved once its first words are stored", async ({
  page,
}) => {
  await aNewProject(page, "Begun saved");
  await aBookWithScenes(page, { "Part One": 0 });
  await openTheWritingView(page);

  await beginning(page).click();
  await page.keyboard.type("The loom stood silent.");
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent.",
  );

  await expect(saveState(page)).toHaveText("All saved");
  const [begun] = (await projectsOutline(page)).sections[0].attachments;
  const stored = await (
    await page.request.get(`${API}/scenes/${begun.id}`)
  ).json();
  expect(stored.text, "All saved only once it is stored").toContain(
    "The loom stood silent.",
  );
});

async function holding(page: Page, method: string, url: string) {
  let release!: () => void;
  const released = new Promise<void>((done) => (release = done));
  let asked = 0;

  await page.route(url, async (route) => {
    if (route.request().method() !== method) {
      return route.fallback();
    }
    asked += 1;
    await released;
    await route.continue();
  });

  return { asked: () => asked, release };
}

async function savingUntilReleased(
  page: Page,
  held: Awaited<ReturnType<typeof holding>>,
) {
  await expect.poll(held.asked).toBeGreaterThan(0);
  await page.waitForTimeout(1000);
  await expect(
    saveState(page),
    "the change has not reached the server yet",
  ).toHaveText("Saving…");

  held.release();
  await expect(saveState(page)).toHaveText("All saved");
}

test("a heading being typed is saving from the first key until the server has its title", async ({
  page,
}) => {
  await aNewProject(page, "SavingHeading");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);
  const held = await holding(page, "PATCH", "**/api/outlines/*/sections/*");

  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" and more");
  await expect(saveState(page)).toHaveText("Saving…");
  expect(held.asked(), "saving before the title is even sent").toBe(0);

  await savingUntilReleased(page, held);
  expect(await savedTitles(page)).toEqual(["Part One and more"]);
});

test("a scene renamed in the margin is saving until the server has its title", async ({
  page,
}) => {
  await aNewProject(page, "SavingTitle");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, arrival)).toBeVisible();
  const held = await holding(page, "PATCH", "**/api/scenes/*");

  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("The departure");
  await page.keyboard.press("Enter");
  await expect(saveState(page)).toHaveText("Saving…");

  await savingUntilReleased(page, held);
  expect((await (await sceneOnServer(page, arrival)).json()).title).toBe(
    "The departure",
  );
});

test("a change to the book's shape is saving until the server has made it", async ({
  page,
}) => {
  await aNewProject(page, "SavingShape");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();
  const held = await holding(
    page,
    "DELETE",
    "**/api/outlines/*/attachments/**",
  );

  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();
  await expect(saveState(page)).toHaveText("Saving…");

  await savingUntilReleased(page, held);
  expect(await sceneOrder(page)).toEqual([["Part One", [second]]]);
});

const wordCount = (page: Page) =>
  statusNote(page).getByRole("button", { name: /words? in/ });

test("the word count follows the section being written in, and counts the book on a click", async ({
  page,
}) => {
  await aNewProject(page, "Counting");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await openTheWritingView(page);

  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");

  await scene(page, scenes["Part Two"][0]).click();
  await page.keyboard.type("Ende");
  await expect(wordCount(page)).toHaveText("1 word in “Part Two”");

  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("5 words in the book");
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("1 word in “Part Two”");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await heading(page, "Part One").click();
  await expect(
    wordCount(page),
    "what was written before counts too",
  ).toHaveText("4 words in “Part One”");
});

test("a section's word count takes in the sections nested in it", async ({
  page,
}) => {
  await aNewProject(page, "CountingNested");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  const placed: Record<string, string> = {};
  for (const title of ["Part One", "Chapter 1", "Part Two"]) {
    placed[title] = await aLooseScene(page);
    await page.request.post(`${API}/outlines/${outline}/attachments`, {
      data: {
        attachment: { kind: "scene", id: placed[title] },
        section: ids[title],
        after: null,
      },
    });
  }
  await openTheWritingView(page);

  await scene(page, placed["Part One"]).click();
  await page.keyboard.type("one two three");
  await scene(page, placed["Chapter 1"]).click();
  await page.keyboard.type("four five");
  await expect(wordCount(page)).toHaveText("2 words in “Chapter 1”");
  await scene(page, placed["Part Two"]).click();
  await page.keyboard.type("six");

  await heading(page, "Part One").click();
  await expect(wordCount(page)).toHaveText("5 words in “Part One”");
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("6 words in the book");
});

test("before the cursor is anywhere, the word count counts the first section", async ({
  page,
}) => {
  await aNewProject(page, "CountingFirst");
  await aBookWithScenes(page, { "Part One": 1, "Part Two": 0 });
  await openTheWritingView(page);

  await expect(wordCount(page)).toHaveText("0 words in “Part One”");
});

test("the first time, Write lands at the start of the book", async ({
  page,
}) => {
  await aNewProject(page, "FirstLanding");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  await openTheWritingView(page);

  await expect(scene(page, scenes["Part One"][0])).toBeFocused();
  await expect(goingBack(page), "nothing was left off yet").toHaveCount(0);
  await page.keyboard.type("Begun");
  await expect(scene(page, scenes["Part One"][0])).toHaveText("Begun");
});

test("Write lands where the author left off, after moving away and after a reload", async ({
  page,
}) => {
  await aNewProject(page, "LeftOff");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const later = scene(page, scenes["Part Two"][0]);
  await openTheWritingView(page);
  await later.click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  for (let step = 0; step < 5; step++) {
    await page.keyboard.press("ArrowRight");
  }
  await expect(saveState(page)).toHaveText("All saved");

  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await openTheWritingView(page);
  await backToWhereILeftOff(page);
  await expect(later).toBeFocused();
  await page.keyboard.type("X");
  await expect(later).toHaveText("AlphaX Beta");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await backToWhereILeftOff(page);
  await expect(later).toBeFocused();
  await page.keyboard.type("Y");
  await expect(later).toHaveText("AlphaXY Beta");
});

test("when the scene left off in has left the book, Write lands at its start", async ({
  page,
}) => {
  await aNewProject(page, "LeftOffGone");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = scenes["Part One"];
  const [later] = scenes["Part Two"];
  await openTheWritingView(page);
  await scene(page, later).click();
  await page.keyboard.type("Gone soon");
  await sceneActions(page, later).click();
  await menuItem(page, "Take out of the book").click();
  await expect(scene(page, later)).toHaveCount(0);
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();

  await expect(scene(page, first)).toBeFocused();
});

test("a book with no scenes yet lands on its first section's writing line", async ({
  page,
}) => {
  await aNewProject(page, "NothingWritten");
  await aBookWithScenes(page, { "Part One": 0 });
  await openTheWritingView(page);

  await expect(beginning(page)).toBeFocused();
});

test("landing far down the book brings the line left off into the middle of the window", async ({
  page,
}) => {
  await aNewProject(page, "FarDown");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const middle = scene(page, scenes["Part One"][7]);
  await openTheWritingView(page);
  await middle.click();
  await page.keyboard.type("Half way.");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await backToWhereILeftOff(page);
  await expect(middle).toBeFocused();

  const line = (await middle.locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(
    Math.abs(line.y + line.height / 2 - tall / 2),
    "the line sits in the middle third of the window",
  ).toBeLessThan(tall / 6);
});

test("even the book's last line can be brought to the middle of the window", async ({
  page,
}) => {
  await aNewProject(page, "LastLine");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const last = scene(page, scenes["Part One"][13]);
  await openTheWritingView(page);
  await last.click();
  await page.keyboard.type("At the very end.");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await backToWhereILeftOff(page);
  await expect(last).toBeFocused();

  const line = (await last.locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(
    Math.abs(line.y + line.height / 2 - tall / 2),
    "there is room below the book to scroll its end up",
  ).toBeLessThan(tall / 6);
});

test("leaving the writing view leaves nothing behind that clicks or keys can trip over", async ({
  page,
}) => {
  const panics: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error" && message.text().includes("panicked")) {
      panics.push(message.text());
    }
  });
  await aNewProject(page, "Leaving");
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  await openTheOutline(page);
  await page.mouse.click(5, 300);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Shift+F10");

  expect(panics).toEqual([]);
});

test("a scene followed from the outline opens the writing view at it, far down the book", async ({
  page,
}) => {
  await aNewProject(page, "Followed");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const followed = scenes["Part One"][9];
  await titled(page, followed, "The tenth");
  await openTheOutline(page);

  await page.locator(".leaf").getByRole("link", { name: "The tenth" }).click();

  await expect(scene(page, followed)).toBeFocused();
  await expect(page, "the address is the view's own again").toHaveURL(
    /\/write$/,
  );
  await page.keyboard.type("Here.");
  await expect(scene(page, followed)).toHaveText("Here.");
  const line = (await scene(page, followed).locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(Math.abs(line.y + line.height / 2 - tall / 2)).toBeLessThan(tall / 6);
});

test("following the scene left off in lands where the author left off in it", async ({
  page,
}) => {
  await aNewProject(page, "FollowedBack");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [, second] = scenes["Part One"];
  await titled(page, second, "The second");
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  for (let step = 0; step < 5; step++) {
    await page.keyboard.press("ArrowRight");
  }
  await expect(saveState(page)).toHaveText("All saved");

  await openTheOutline(page);
  await page.locator(".leaf").getByRole("link", { name: "The second" }).click();

  await expect(scene(page, second)).toBeFocused();
  await expect(
    goingBack(page),
    "a scene followed to needs no way back",
  ).toHaveCount(0);
  await page.keyboard.type("X");
  await expect(scene(page, second)).toHaveText("AlphaX Beta");
});

test("a scene whose text has not arrived still counts, by what the server last counted", async ({
  page,
}) => {
  await aNewProject(page, "CountedByTheServer");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [later] = scenes["Part Two"];
  await openTheWritingView(page);
  await scene(page, later).click();
  await page.keyboard.type("one two three");
  await expect(saveState(page)).toHaveText("All saved");
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  await expect
    .poll(
      async () => {
        const counted = await (
          await page.request.get(
            `${API}/statistics/word-counts?project=${project}`,
          )
        ).json();
        return counted.find((held: { scene: string }) => held.scene === later)
          ?.words;
      },
      {
        message: "the server counts what it stores, and statistics keeps it",
        timeout: 10_000,
      },
    )
    .toBe(3);

  await page.routeWebSocket(/\/api\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (sceneCarried(frame) !== later) {
        socket.send(frame);
      }
    });
  });
  await page.reload();
  await expect(scene(page, later)).toBeVisible();
  await wordCount(page).click();

  await expect(wordCount(page)).toHaveText("3 words in the book");
});

test("what the server last counted never stands over what is being written", async ({
  page,
}) => {
  await aNewProject(page, "CountedLate");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("one two three");
  await expect(saveState(page)).toHaveText("All saved");
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  await expect
    .poll(
      async () =>
        (
          await (
            await page.request.get(
              `${API}/statistics/word-counts?project=${project}`,
            )
          ).json()
        )[0]?.words,
      { timeout: 10_000 },
    )
    .toBe(3);

  let answered = false;
  await page.route("**/api/statistics/word-counts*", async (route) => {
    const counted = await route.fetch();
    await new Promise((resolve) => setTimeout(resolve, 1_500));
    await route.fulfill({ response: counted });
    answered = true;
  });
  await page.reload();
  await backToWhereILeftOff(page);
  await expect(scene(page, only)).toBeFocused();
  await page.keyboard.press("End");
  await page.keyboard.type(" four");
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");

  await expect.poll(() => answered).toBe(true);
  await page.waitForTimeout(300);
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");
});

test("a scene far from the window waits without an editor, and gets one as it comes near", async ({
  page,
}) => {
  const joined = new Set<string>();
  page.on("websocket", (socket) =>
    socket.on("framesent", ({ payload }) => {
      if (Buffer.from(payload)[0] === 0) {
        joined.add(sceneCarried(payload));
      }
    }),
  );
  await aNewProject(page, "FarAndNear");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await expect(scene(page, scenes["Part One"][0])).toBeVisible();

  const waiting = flow(page).locator(`.scene.waiting[data-scene="${last}"]`);
  await expect(waiting).toHaveCount(1);
  await expect(scene(page, last)).toHaveCount(0);
  expect(
    joined.has(last),
    "a scene nobody can see has no editor and is not synced",
  ).toBe(false);
  expect(joined.size).toBeGreaterThan(0);

  await waiting.scrollIntoViewIfNeeded();

  await expect(scene(page, last)).toBeVisible();
  await expect(waiting).toHaveCount(0);
});

test("landing where the author left off reaches a scene far down the book", async ({
  page,
}) => {
  await aNewProject(page, "FarLanding");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, last).click();
  await page.keyboard.type("Far down.");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();

  await backToWhereILeftOff(page);
  await expect(scene(page, last)).toBeFocused();
});

test("a scene coming near is named in the margin before its text arrives", async ({
  page,
}) => {
  await aNewProject(page, "FarNamed");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first] = scenes["Part One"];
  const last = scenes["Part One"][59];
  await openTheWritingView(page);
  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();
  await scene(page, last).click();
  await page.keyboard.type("The far scene speaks.");
  await expect(saveState(page)).toHaveText("All saved");
  await expect
    .poll(() => cataloguedOpening(page, last), {
      message:
        "an opening is told when the live scene counts, at most every 2 seconds, so it trails what was saved",
      timeout: 15_000,
    })
    .toBe("The far scene speaks.");
  await page.evaluate(() => window.scrollTo(0, 0));
  await scene(page, first).click();
  await page.keyboard.type("A");
  await expect(saveState(page)).toHaveText("All saved");

  await page.routeWebSocket(/\/api\/sync$/, (socket) => {
    const server = socket.connectToServer();
    socket.onMessage((frame) => server.send(frame));
    server.onMessage((frame) => {
      if (sceneCarried(frame) !== last) {
        socket.send(frame);
      }
    });
  });
  await page.reload();
  await backToWhereILeftOff(page);
  await expect(scene(page, first)).toBeFocused();
  await expect(
    flow(page).locator(`.scene-slot.waiting[data-place-of="${last}"]`),
  ).toHaveCount(1);
  await expect(
    flow(page).locator(`[data-place-of="${last}"] *`),
    "a scene far away draws nothing but its place",
  ).toHaveCount(1);

  await flow(page)
    .locator(`[data-place-of="${last}"]`)
    .scrollIntoViewIfNeeded();

  await expect(scene(page, last)).toHaveText("");
  await expect(label(page, last)).toHaveText("The far scene speaks.");
});

test("another author's cursor shows in the scene they are writing in", async ({
  page,
  context,
}) => {
  await aNewProject(page, "TwoAuthors");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  const other = await context.newPage();
  await other.goto(page.url());
  await expect(scene(other, second)).toBeVisible();

  await scene(other, second).click();
  await other.keyboard.type("Written elsewhere.");

  await expect(scene(page, second)).toContainText("Written elsewhere.");
  await expect(
    flow(page).locator(`[data-scene="${second}"] .ProseMirror-yjs-cursor`),
  ).toHaveCount(1);
  await expect(
    flow(page).locator(`[data-scene="${first}"] .ProseMirror-yjs-cursor`),
  ).toHaveCount(0);
  await expect(
    flow(page).locator(
      `[data-scene="${second}"] .ProseMirror-yjs-cursor > div`,
    ),
    "the name above another author's cursor floats over the text rather than pushing it apart",
  ).toHaveCSS("position", "absolute");
  await other.close();
});

test("a dropped connection comes back, and what was written meanwhile is saved", async ({
  page,
}) => {
  let down = false;
  let live: { close: () => Promise<void> } | null = null;
  await page.routeWebSocket(/\/api\/sync$/, (socket) => {
    if (down) {
      socket.close();
      return;
    }
    socket.connectToServer();
    live = socket;
  });
  await aNewProject(page, "Dropped");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("Before.");
  await expect(saveState(page)).toHaveText("All saved");

  down = true;
  await live!.close();
  await page.keyboard.type(" During.");
  await expect(saveState(page)).toHaveText("Saving…");
  await page.waitForTimeout(1_000);
  await expect(
    saveState(page),
    "nothing can be stored while the connection is down",
  ).toHaveText("Saving…");

  down = false;
  await expect(saveState(page)).toHaveText("All saved", { timeout: 10_000 });
  expect(
    (await (await page.request.get(`${API}/scenes/${only}`)).json()).text,
  ).toBe("Before. During.");
});

async function aLongBookWrittenAtItsStart(page: Page, named: string) {
  await aNewProject(page, named);
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("She had not touched it since spring.");

  return { first, last: scenes["Part One"][59] };
}

const toTheEnd = (page: Page) =>
  page.evaluate(() => window.scrollTo(0, document.body.scrollHeight));

const nobodyWriting = (page: Page) =>
  page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());

test("a scene scrolled far away is let go, its height kept, and back again as it comes near", async ({
  page,
}) => {
  const left: string[] = [];
  page.on("websocket", (socket) =>
    socket.on("framesent", ({ payload }) => {
      if (Buffer.from(payload)[0] === 1) {
        left.push(sceneCarried(payload));
      }
    }),
  );
  const { first } = await aLongBookWrittenAtItsStart(page, "LetGo");
  await expect(saveState(page)).toHaveText("All saved");
  const height = (await flow(page)
    .locator(`[data-place-of="${first}"]`)
    .boundingBox())!.height;
  await nobodyWriting(page);

  await toTheEnd(page);

  const waiting = flow(page).locator(`.scene.waiting[data-scene="${first}"]`);
  await expect(waiting).toHaveCount(1);
  expect(
    Math.abs((await waiting.boundingBox())!.height - height),
    "the page keeps its length, so nothing below jumps",
  ).toBeLessThan(1);
  expect(left, "a scene let go leaves the connection").toContain(first);

  await page.evaluate(() => window.scrollTo(0, 0));

  await expect(scene(page, first)).toContainText(
    "She had not touched it since spring.",
  );
});

test("the scene the cursor is in stays however far the window scrolls", async ({
  page,
}) => {
  const { first } = await aLongBookWrittenAtItsStart(page, "KeptByTheCursor");

  await toTheEnd(page);
  await page.waitForTimeout(500);

  await expect(scene(page, first)).toBeFocused();
  await page.keyboard.type(" Still here.");
  await expect(scene(page, first)).toContainText("since spring. Still here.");
});

test("a scene still saving stays until it is saved", async ({ page }) => {
  const sync = await holdingWhatIsWritten(page);
  const { first } = await aLongBookWrittenAtItsStart(page, "KeptUntilSaved");
  await expect(saveState(page)).toHaveText("All saved");
  sync.hold();
  await page.keyboard.type(" More.");
  await expect(saveState(page)).toHaveText("Saving…");
  await nobodyWriting(page);

  await toTheEnd(page);
  await page.waitForTimeout(500);

  await expect(
    scene(page, first),
    "letting go of the only copy of an unsaved edit would lose it",
  ).toHaveCount(1);
  sync.release();
  await expect(saveState(page)).toHaveText("All saved");
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${first}"]`),
  ).toHaveCount(1);
});

test("the arrow keys carry the cursor into a scene let go while the window was elsewhere", async ({
  page,
}) => {
  await aNewProject(page, "WalkingIntoTheFar");
  const scenes = await aBookWithScenes(page, { "Part One": 60 });
  const [first, second, third] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Second.");
  await scene(page, first).click();
  await page.keyboard.type("First.");
  await expect(saveState(page)).toHaveText("All saved");

  await toTheEnd(page);
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${second}"]`),
  ).toHaveCount(1);
  await page.keyboard.press("ArrowDown");

  await expect(scene(page, second)).toBeFocused();
  await page.keyboard.type("X");
  await expect(scene(page, second)).toHaveText("XSecond.");

  await scene(page, third).click();
  await toTheEnd(page);
  await expect(
    flow(page).locator(`.scene.waiting[data-scene="${second}"]`),
  ).toHaveCount(1);
  await page.keyboard.press("ArrowUp");

  await expect(scene(page, second)).toBeFocused();
  await page.keyboard.type("Y");
  await expect(
    scene(page, second),
    "coming from below, the cursor lands after the scene's last word",
  ).toHaveText("XSecond.Y");
});

const sceneTab = (page: Page) =>
  page.getByRole("button", { name: "This scene" });

const sceneTray = (page: Page) =>
  page.getByRole("complementary", { name: "This scene" });

const trayTitle = (page: Page) =>
  sceneTray(page).getByRole("textbox", { name: "Scene title" });

async function aTitledPair(page: Page) {
  await aNewProject(page, "Tray");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await titled(page, first, "The arrival");
  await titled(page, second, "The loom");
  await openTheWritingView(page);

  return [first, second];
}

test("the tray is shut at first and opens from its tab on the scene the cursor is in", async ({
  page,
}) => {
  const [, second] = await aTitledPair(page);
  await expect(sceneTray(page)).toBeHidden();

  await scene(page, second).click();
  await sceneTab(page).click();

  await expect(sceneTab(page)).toHaveAttribute("aria-expanded", "true");
  await expect(trayTitle(page)).toHaveValue("The loom");
});

test("the tray follows the cursor, and keeps the last scene when the cursor leaves the scenes", async ({
  page,
}) => {
  const [first, second] = await aTitledPair(page);
  await sceneTab(page).click();

  await scene(page, first).click();
  await expect(trayTitle(page)).toHaveValue("The arrival");
  await scene(page, second).click();
  await expect(trayTitle(page)).toHaveValue("The loom");
  await flow(page).getByRole("heading", { name: "Part One" }).click();

  await expect(
    trayTitle(page),
    "reaching for the tray must not empty it",
  ).toHaveValue("The loom");
});

test("an untitled scene is named in the tray by its opening words", async ({
  page,
}) => {
  await aNewProject(page, "TrayUntitled");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await sceneTab(page).click();
  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "An empty scene",
  );

  await scene(page, only).click();
  await page.keyboard.type("The loom stood silent.");

  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
  await expect(trayTitle(page)).toHaveValue("");
});

test("the tray stays open or shut across a reload, and Escape in it shuts it", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await page.reload();
  await expect(sceneTray(page)).toBeVisible();

  await sceneTray(page).getByRole("button", { name: "Close the tray" }).focus();
  await page.keyboard.press("Escape");
  await expect(sceneTray(page)).toBeHidden();
  await page.reload();
  await expect(flow(page)).toBeVisible();
  await expect(sceneTray(page)).toBeHidden();
});

test("Escape while writing leaves the tray open", async ({ page }) => {
  const [first] = await aTitledPair(page);
  await sceneTab(page).click();
  await scene(page, first).click();

  await page.keyboard.press("Escape");

  await expect(
    sceneTray(page),
    "Escape in the prose belongs to the margin, not the tray",
  ).toBeVisible();
});

test("on a wide window the open tray sits beside the column and ends above the status bar", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  const noteBefore = (await statusNote(page).boundingBox())!;

  await sceneTab(page).click();

  const tray = (await sceneTray(page).locator(".inside").boundingBox())!;
  const written = (await scene(page, first).boundingBox())!;
  const note = (await statusNote(page).boundingBox())!;
  expect(written.x + written.width).toBeLessThanOrEqual(tray.x);
  expect(
    tray.y + tray.height,
    "the tray covers nothing of the status bar",
  ).toBeLessThanOrEqual(note.y + 1);
  expect(note.x, "the status bar keeps its place").toBe(noteBefore.x);
  expect(
    await statusNote(page).evaluate((shown) => {
      const at = shown.getBoundingClientRect();

      return shown.contains(
        document.elementFromPoint(at.x + at.width / 2, at.y + at.height / 2),
      );
    }),
    "nothing covers the status note",
  ).toBe(true);
});

test("on a phone the tray slides over the text and is toggled from the status bar", async ({
  page,
}) => {
  await page.setViewportSize({ width: 400, height: 800 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  const tray = (await sceneTray(page).boundingBox())!;
  expect(tray.x).toBeGreaterThanOrEqual(0);
  expect(tray.x + tray.width).toBeLessThanOrEqual(400);
  await expect(trayTitle(page)).toHaveValue("The arrival");

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(sceneTray(page)).toBeHidden();
});

test("a scene renamed in the tray is named so in the margin, on the server and in the outline", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("Begun.");
  await sceneTab(page).click();

  await trayTitle(page).fill("The homecoming");
  await trayTitle(page).press("Enter");

  await expect(label(page, first)).toHaveText("The homecoming");
  await expect(
    scene(page, first),
    "Enter puts the cursor back where it was writing",
  ).toBeFocused();
  await page.keyboard.type(" Still.");
  await expect(scene(page, first)).toHaveText("Begun. Still.");
  await sceneCatalogued(page, first, "The homecoming");
  await openTheOutline(page);
  await expect(page.locator(".leaf .name").first()).toHaveText(
    "The homecoming",
  );
});

test("Escape in the tray's title drops what was typed and keeps the tray open", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await trayTitle(page).fill("Not this");
  await trayTitle(page).press("Escape");

  await expect(trayTitle(page)).toHaveValue("The arrival");
  await expect(sceneTray(page)).toBeVisible();
  await expect(scene(page, first)).toBeFocused();
  await expect(label(page, first)).toHaveText("The arrival");
});

test("a title cleared in the tray leaves the opening words to name the scene", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await sceneTab(page).click();

  await trayTitle(page).fill("");
  await trayTitle(page).press("Enter");

  await expect(label(page, first)).toHaveText("The loom stood silent.");
  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
});

test("a scene renamed in the margin is named so in the open tray", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await flow(page)
    .locator(`.scene-row:has([data-scene="${first}"])`)
    .getByRole("button", { name: "Scene actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Renamed");
  await flow(page).getByRole("textbox", { name: "Scene title" }).press("Enter");

  await expect(trayTitle(page)).toHaveValue("Renamed");
});

async function anIdea(page: Page, title: string) {
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: projectOf(page), title },
  });
  expect(captured.status()).toBe(201);

  return (await captured.json()).id as string;
}

async function anIdeaLinkedTo(page: Page, scene: string, title: string) {
  const id = await anIdea(page, title);
  const linked = await page.request.post(`${API}/scenes/${scene}/ideas`, {
    data: { idea: id },
  });
  expect(linked.ok()).toBe(true);

  return id as string;
}

const drawnFrom = (page: Page) =>
  sceneTray(page).getByRole("list", { name: "Drawn from" });

test("the tray lists the ideas a scene draws on, each leading to its idea", async ({
  page,
}) => {
  await aNewProject(page, "TrayIdeas");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();

  await sceneTab(page).click();

  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
    "A war down the valley",
  ]);
  await drawnFrom(page)
    .getByRole("link", { name: "The loom remembers" })
    .click();
  await expect(page).toHaveURL(new RegExp(`/ideas/[^/]*${loom}$`));
});

test("an idea unlinked in the tray no longer counts for the scene", async ({
  page,
}) => {
  await aNewProject(page, "TrayUnlink");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  const war = await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await drawnFrom(page)
    .getByRole("button", { name: "Unlink The loom remembers" })
    .click();

  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "A war down the valley",
  ]);
  await expect
    .poll(
      async () =>
        (await (await page.request.get(`${API}/scenes/${first}`)).json()).ideas,
    )
    .toEqual([war]);
  expect(loom).not.toBe(war);
});

test("the tray's ideas follow the cursor to the next scene, and a scene without any says so", async ({
  page,
}) => {
  await aNewProject(page, "TrayIdeasFollow");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
  ]);

  await scene(page, second).click();

  await expect(drawnFrom(page)).toHaveCount(0);
  await expect(sceneTray(page)).toContainText("No ideas linked yet.");
});

const linkDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Link ideas" });

test("ideas linked from the tray are listed in the order they were picked, and kept", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinking");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  const winter = await anIdea(page, "Winter comes early");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();
  await linkDialog(page).getByLabel("Winter comes early").check();
  await linkDialog(page).getByLabel("The loom remembers").check();
  await linkDialog(page).getByRole("button", { name: "Link 2" }).click();

  await expect(linkDialog(page)).toBeHidden();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "Winter comes early",
    "The loom remembers",
  ]);
  await expect
    .poll(
      async () =>
        (await (await page.request.get(`${API}/scenes/${first}`)).json()).ideas,
    )
    .toEqual([winter, loom]);
});

test("the tray's dialog offers only ideas the scene does not draw on yet", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinkingOffers");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();

  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("A war down the valley");
});

test("Escape closes the tray's dialog and leaves the tray open", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinkingEscape");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();
  await expect(linkDialog(page)).toBeVisible();

  await page.keyboard.press("Escape");

  await expect(linkDialog(page)).toBeHidden();
  await expect(sceneTray(page)).toBeVisible();
  await expect(sceneTray(page)).toContainText("No ideas linked yet.");
});

const sides = (page: Page) =>
  page.getByRole("navigation", { name: "Sides of the work" });

async function openTheLooseSide(page: Page) {
  await sides(page)
    .getByRole("link", { name: /Not in the book/ })
    .click();
  await expect(page).toHaveURL(/\/write\/loose$/);
}

test("the switch shows the book on one side and the scenes outside it on the other", async ({
  page,
}) => {
  await aNewProject(page, "Sides");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await expect(scene(page, placed)).toBeVisible();
  await expect(scene(page, loose)).toHaveCount(0);
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");

  await openTheLooseSide(page);

  await expect(scene(page, loose)).toBeVisible();
  await expect(scene(page, placed)).toHaveCount(0);
  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toHaveAttribute("aria-current", "page");
  await expect(
    sides(page).getByRole("link", { name: "The book", exact: true }),
  ).not.toHaveAttribute("aria-current", "page");
});

test("a scene outside the book is written in on its side and kept", async ({
  page,
}) => {
  await aNewProject(page, "LooseWriting");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);

  await scene(page, loose).click();
  await page.keyboard.type("Cut, but kept.");

  await expect(saveState(page)).toHaveText("All saved");
  await expect
    .poll(async () => (await (await sceneOnServer(page, loose)).json()).text)
    .toBe("Cut, but kept.");
});

test("the arrow keys walk among the scenes outside the book", async ({
  page,
}) => {
  await aNewProject(page, "LooseWalk");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  const second = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, first).click();
  await page.keyboard.type("First.");

  await page.keyboard.press("ArrowDown");
  await page.keyboard.type("Second.");

  await expect(scene(page, second)).toHaveText("Second.");
});

test("each side counts its own words", async ({ page }) => {
  await aNewProject(page, "LooseCount");
  const [placed] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, placed).click();
  await page.keyboard.type("Two words.");
  await expect(saveState(page)).toHaveText("All saved");

  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Three more words.");

  await expect(statusNote(page).locator(".word-count")).toHaveText(
    "3 words not in the book",
  );
  await expect(saveState(page)).toHaveText("All saved");
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();
  await expect(statusNote(page).locator(".word-count")).toContainText(
    "2 words",
  );
});

test("the tray names a scene outside the book", async ({ page }) => {
  await aNewProject(page, "LooseTray");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);

  await scene(page, loose).click();
  await sceneTab(page).click();

  await expect(trayTitle(page)).toHaveValue("The cut scene");
});

test("with every scene in the book, the switch stays out of the way and the other side says so", async ({
  page,
}) => {
  await aNewProject(page, "NoLoose");
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(sides(page)).toHaveCount(0);

  await page.goto(page.url().replace(/\/write$/, "/write/loose"));

  await expect(flow(page)).toContainText("Every scene is in the book.");
  await expect(flow(page).locator(".scene")).toHaveCount(0);
  await expect(
    sides(page),
    "on the other side the switch is the only way back",
  ).toBeVisible();
});

test("a scene outside the book is offered its own ways, and Enter twice begins nothing", async ({
  page,
}) => {
  await aNewProject(page, "LooseMenu");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Alone.");

  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Still here.");
  await sceneActions(page, loose).click();

  await expect(
    flow(page).getByRole("menuitem"),
    "taking out of the book means nothing for a scene not in it",
  ).toHaveText(["Rename", "Bring into the book…", "Delete scene"]);
  await expect(flow(page).locator(".scene")).toHaveCount(1);
  await expect(
    scene(page, loose).locator("p"),
    "with no next scene to begin, Enter twice is two line breaks like anywhere else",
  ).toHaveText(["Alone.", "", "Still here."]);
});

test("writing outside the book leaves where you left off in the book alone", async ({
  page,
}) => {
  await aNewProject(page, "LooseLeftOff");
  const [, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Here.");
  await expect(saveState(page)).toHaveText("All saved");
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Elsewhere.");
  await expect(saveState(page)).toHaveText("All saved");

  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  await backToWhereILeftOff(page);
  await expect(scene(page, second)).toBeFocused();
});

const placingDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Bring into the book" });

test("a scene outside the book is brought into the section chosen, and the cursor lands in it", async ({
  page,
}) => {
  await aNewProject(page, "BringingIn");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const loose = await aLooseScene(page);
  await titled(page, loose, "The cut scene");
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();

  await expect(placingDialog(page)).toContainText(
    "“The cut scene” goes at the end of the section you choose.",
  );
  await expect(
    placingDialog(page)
      .getByRole("list", { name: "Sections of the book" })
      .getByRole("button"),
  ).toHaveText(["Part One", "Part Two"]);
  await placingDialog(page).getByRole("button", { name: "Part Two" }).click();

  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, loose)).toBeFocused();
  await expect
    .poll(() => sceneOrder(page))
    .toEqual([
      ["Part One", scenes["Part One"]],
      ["Part Two", [...scenes["Part Two"], loose]],
    ]);
  await expect(
    sides(page),
    "with the last scene brought in, nothing is left outside the book",
  ).toHaveCount(0);
});

test("a scene outside the book is deleted after asking", async ({ page }) => {
  await aNewProject(page, "LooseDelete");
  await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Delete scene").click();

  await asked(page).getByRole("button", { name: "Delete" }).click();

  await expect(scene(page, loose)).toHaveCount(0);
  await expect(flow(page)).toContainText("Every scene is in the book.");
  await expect(sides(page)).toBeVisible();
  await expect
    .poll(async () => (await sceneOnServer(page, loose)).status())
    .toBe(404);
});

test("Escape in the section dialog leaves the scene outside the book", async ({
  page,
}) => {
  await aNewProject(page, "BringingInEscape");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await openTheLooseSide(page);
  await sceneActions(page, loose).click();
  await menuItem(page, "Bring into the book…").click();
  await expect(placingDialog(page)).toBeVisible();

  await page.keyboard.press("Escape");

  await expect(placingDialog(page)).toBeHidden();
  await expect(scene(page, loose)).toBeVisible();
  expect(await sceneOrder(page)).toEqual([["Part One", scenes["Part One"]]]);
});

const following = (page: Page, side: string, scene: string) =>
  page.goto(
    page.url().replace(/\/write(\/loose)?(\?.*)?$/, `${side}?scene=${scene}`),
  );

test("a scene outside the book followed into the book's side opens the other side at it", async ({
  page,
}) => {
  await aNewProject(page, "FollowLoose");
  await aBookWithScenes(page, { "Part One": 1 });
  await aLooseScene(page);
  const wanted = await aLooseScene(page);
  await openTheWritingView(page);

  await following(page, "/write", wanted);

  await expect(page).toHaveURL(/\/write\/loose$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("a scene in the book followed into the other side goes back to the book", async ({
  page,
}) => {
  await aNewProject(page, "FollowPlaced");
  const [, wanted] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await aLooseScene(page);
  await openTheWritingView(page);

  await following(page, "/write/loose", wanted);

  await expect(page).toHaveURL(/\/write$/);
  await expect(scene(page, wanted)).toBeFocused();
});

test("each side lands where you left off on it", async ({ page }) => {
  await aNewProject(page, "LeftOffSides");
  const [, placed] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await aLooseScene(page);
  const loose = await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, placed).click();
  await page.keyboard.type("In the book.");
  await openTheLooseSide(page);
  await scene(page, loose).click();
  await page.keyboard.type("Outside.");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await backToWhereILeftOff(page);
  await expect(scene(page, loose)).toBeFocused();
  await sides(page)
    .getByRole("link", { name: "The book", exact: true })
    .click();

  await backToWhereILeftOff(page);
  await expect(scene(page, placed)).toBeFocused();
});

test("the other side starts in its first scene when nothing was left off there", async ({
  page,
}) => {
  await aNewProject(page, "LooseFirst");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  await aLooseScene(page);
  await openTheWritingView(page);

  await openTheLooseSide(page);

  await expect(scene(page, first)).toBeFocused();
});

async function aSceneBegunHere(page: Page) {
  await beginning(page).click();
  await page.keyboard.type("Begun here.");
  await expect(beginning(page)).toHaveCount(0);
  await expect(saveState(page)).toHaveText("All saved");
  const [begun] = await flow(page)
    .locator(".scene[data-scene]")
    .evaluateAll((all) => all.map((one) => one.getAttribute("data-scene")!));

  return begun;
}

test("a scene begun in the view and taken out of the book shows up outside it at once", async ({
  page,
}) => {
  await aNewProject(page, "TakenOutAtOnce");
  await aBookWithScenes(page, { "Part One": 0 });
  await openTheWritingView(page);
  const begun = await aSceneBegunHere(page);
  await expect(sides(page)).toHaveCount(0);

  await sceneActions(page, begun).click();
  await menuItem(page, "Take out of the book").click();

  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
    "the view knows of the scene it began, though the listing it opened with does not",
  ).toContainText("1");
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toHaveText("Begun here.");
});

test("the scenes of a section removed from the book show up outside it at once", async ({
  page,
}) => {
  await aNewProject(page, "RemovedAtOnce");
  await aBookWithScenes(page, { "Part One": 0, "Part Two": 1 });
  await openTheWritingView(page);
  const begun = await aSceneBegunHere(page);

  await sectionActions(page, "Part One").click();
  await menuItem(page, "Remove section").click();
  await asked(page).getByRole("button", { name: "Remove" }).click();

  await expect(
    sides(page).getByRole("link", { name: /Not in the book/ }),
  ).toContainText("1");
  await openTheLooseSide(page);
  await expect(scene(page, begun)).toBeVisible();
});

test("two tabs writing in one scene converge", async ({ page, context }) => {
  await aNewProject(page, "Converge");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  const second = await context.newPage();
  await second.goto(page.url());
  await expect(scene(second, only)).toBeVisible();

  await scene(page, only).click();
  await page.keyboard.type("She threaded the shuttle.");
  await expect(scene(second, only)).toContainText("She threaded the shuttle.");
  await scene(second, only).click();
  await second.keyboard.press("Control+End");
  await second.keyboard.type(" The loom answered.");

  const written = (tab: Page) =>
    scene(tab, only).evaluate((editor) => {
      const copy = editor.cloneNode(true) as HTMLElement;
      copy
        .querySelectorAll(".ProseMirror-yjs-cursor")
        .forEach((cursor) => cursor.remove());

      return copy.textContent;
    });
  const woven = "She threaded the shuttle. The loom answered.";
  await expect.poll(() => written(page)).toBe(woven);
  await expect.poll(() => written(second)).toBe(woven);
  await second.close();
});

test("leaving the writing view closes its connection", async ({ page }) => {
  await aNewProject(page, "TearDown");
  const sockets: { isClosed(): boolean }[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().endsWith("/api/sync")) {
      sockets.push(socket);
    }
  });
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(1);
  expect(sockets).toHaveLength(1);

  await views(page).getByRole("link", { name: "Board", exact: true }).click();

  await expect.poll(() => sockets[0].isClosed()).toBe(true);
  await expect(flow(page)).toHaveCount(0);
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

test("the tray's dialog finds ideas by name", async ({ page }) => {
  await aNewProject(page, "TrayLinkingFinds");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();

  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("loom");

  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("The loom remembers");
  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("nothing like it");
  await expect(linkDialog(page)).toContainText("No idea matches.");
});

const writingSettings = (page: Page) =>
  page.getByRole("dialog", { name: "Writing settings" });

const checkedForSpelling = (page: Page) =>
  flow(page)
    .locator(".scene .ProseMirror, .heading")
    .evaluateAll((all) => all.map((one) => (one as HTMLElement).spellcheck));

test("scenes are checked for spelling by default, headings never", async ({
  page,
}) => {
  await aNewProject(page, "Spelling");
  await aBookWithScenes(page, { "Part One": 2 });
  await openTheWritingView(page);
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(2);

  expect(
    await flow(page)
      .locator(".scene .ProseMirror")
      .evaluateAll((all) => all.map((one) => (one as HTMLElement).spellcheck)),
  ).toEqual([true, true]);
  expect(
    await flow(page)
      .locator(".heading")
      .evaluateAll((all) => all.map((one) => (one as HTMLElement).spellcheck)),
  ).toEqual([false]);
});

test("spelling switched off in the writing settings stays off for every scene, across a reload", async ({
  page,
}) => {
  await aNewProject(page, "SpellingOff");
  await aBookWithScenes(page, { "Part One": 2 });
  await openTheWritingView(page);
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(2);

  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await writingSettings(page).getByLabel("Check spelling").uncheck();
  await writingSettings(page).getByRole("button", { name: "Close" }).click();

  await expect(writingSettings(page)).toBeHidden();
  expect(await checkedForSpelling(page)).toEqual([false, false, false]);
  await page.reload();
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(2);
  expect(await checkedForSpelling(page)).toEqual([false, false, false]);

  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await expect(
    writingSettings(page).getByLabel("Check spelling"),
  ).not.toBeChecked();
  await writingSettings(page).getByLabel("Check spelling").check();
  await page.keyboard.press("Escape");

  await expect(writingSettings(page)).toBeHidden();
  expect(
    await flow(page)
      .locator(".scene .ProseMirror")
      .evaluateAll((all) => all.map((one) => (one as HTMLElement).spellcheck)),
  ).toEqual([true, true]);
});

async function aSlowHandover(page: Page) {
  await page.route("**/attachments", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 250));
    await route.continue();
  });
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: 4 });
}

const SENTENCE = "The loom stood silent in the grey light.";

test("nothing typed while a section's first scene is being made is lost", async ({
  page,
}) => {
  await aNewProject(page, "HandoverOpening");
  await aBookWithScenes(page, { "Part One": 0 });
  await openTheWritingView(page);
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

async function theBookIsIn(page: Page, language: string | null) {
  const changed = await page.request.put(
    `${API}/projects/${projectOf(page)}/details`,
    { data: { language, description: "" } },
  );
  expect(changed.ok()).toBe(true);
  await page.reload();
}

test("the book is marked with its language, so the browser checks it in that language", async ({
  page,
}) => {
  await aNewProject(page, "Sprache");
  await aBookWithScenes(page, { "Part One": 1 });
  await theBookIsIn(page, "de");

  await openTheWritingView(page);

  await expect(flow(page)).toHaveAttribute("lang", "de");
});

test("a book without a language leaves the page's own", async ({ page }) => {
  await aNewProject(page, "NoLanguage");
  await aBookWithScenes(page, { "Part One": 1 });
  await theBookIsIn(page, null);

  await openTheWritingView(page);
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(1);

  await expect(flow(page)).not.toHaveAttribute("lang", /.+/);
});

test("a project's details are fetched once while moving between its views", async ({
  page,
}) => {
  await aNewProject(page, "OnceOnly");
  await aBookWithScenes(page, { "Part One": 1 });
  const asked: string[] = [];
  page.on("request", (request) => {
    if (
      request.method() === "GET" &&
      /\/api\/projects\/[^/?]+$/.test(request.url())
    ) {
      asked.push(request.url());
    }
  });
  await theBookIsIn(page, "de");

  await openTheWritingView(page);
  await expect(flow(page)).toHaveAttribute("lang", "de");
  await openTheOutline(page);
  await openTheWritingView(page);
  await expect(flow(page)).toHaveAttribute("lang", "de");

  expect(
    asked,
    "the project being worked in is held while its views come and go",
  ).toHaveLength(1);
});

test("moving the cursor just as the scene goes away raises no error", async ({
  page,
}) => {
  const failures: string[] = [];
  page.on("pageerror", (failure) => failures.push(failure.message));
  await aNewProject(page, "Vanishing");
  await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);
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

test("on a phone, the menu of a place to add opens where it can be seen", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 700 });
  await aNewProject(page, "PhoneMenu");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await openTheWritingView(page);

  await addHere(gapBelow(page, scenes["Part One"][0])).click();

  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const seen = (await menu.boundingBox())!;
  expect(seen.x).toBeGreaterThanOrEqual(0);
  expect(seen.x + seen.width).toBeLessThanOrEqual(375);
});

test("Write opens at the start of the book, and offers the way back to where the author left off", async ({
  page,
}) => {
  await aNewProject(page, "OfferedBack");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const [first] = scenes["Part One"];
  const far = scenes["Part One"][11];
  await titled(page, far, "The far scene");
  await openTheWritingView(page);
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  await scene(page, far).click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowRight");
  await expect(saveState(page)).toHaveText("All saved");

  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await openTheWritingView(page);

  await expect(goingBack(page)).toContainText("The far scene");
  expect(
    await page.evaluate(() => window.scrollY),
    "the book is open at its start",
  ).toBe(0);
  await expect(scene(page, first)).not.toBeFocused();
  await expect(scene(page, far)).not.toBeFocused();

  await goingBack(page).click();

  await expect(scene(page, far)).toBeFocused();
  await expect(scene(page, far)).toBeInViewport();
  await expect(goingBack(page), "taken, the offer goes").toHaveCount(0);
  await page.keyboard.type("X");
  await expect(scene(page, far)).toHaveText("AXlpha Beta");
});

test("putting the cursor in the book lets the offer go", async ({ page }) => {
  await aNewProject(page, "OfferLetGo");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Left here.");
  await expect(saveState(page)).toHaveText("All saved");
  await page.reload();
  await expect(goingBack(page)).toBeVisible();

  await scene(page, first).click();

  await expect(goingBack(page)).toHaveCount(0);
  await expect(scene(page, first)).toBeFocused();
});

const caretIn = (page: Page) =>
  page.evaluate(() => {
    const chosen = getSelection()!;
    const block = chosen.focusNode!.parentElement!.closest("p")!;
    const before = document.createRange();
    before.selectNodeContents(block);
    before.setEnd(chosen.focusNode!, chosen.focusOffset);

    return `${block.textContent} @${before.toString().length}`;
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

test("Ctrl+I and Ctrl+B set a word in italics and in bold, and take it off again", async ({
  page,
}) => {
  await aNewProject(page, "Emphasis");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("She never returned.");
  await page.keyboard.press("Home");
  for (let step = 0; step < "She".length; step++) {
    await page.keyboard.press("Shift+ArrowRight");
  }

  await page.keyboard.press("ControlOrMeta+I");
  await expect(scene(page, only).locator("em")).toHaveText("She");
  await page.keyboard.press("ControlOrMeta+B");
  await expect(scene(page, only).locator("strong")).toHaveText("She");
  const weights = await scene(page, only).evaluate((prose) => [
    Number(getComputedStyle(prose).fontWeight),
    Number(getComputedStyle(prose.querySelector("strong")!).fontWeight),
  ]);
  expect(
    weights[1] - weights[0],
    "bold stands out from the light prose around it, in every browser",
  ).toBeGreaterThanOrEqual(200);

  await page.keyboard.press("ControlOrMeta+I");
  await expect(scene(page, only).locator("em")).toHaveCount(0);
  await expect(scene(page, only)).toHaveText("She never returned.");
});

test("italics begun by Ctrl+I go on as typed, and are kept", async ({
  page,
}) => {
  await aNewProject(page, "EmphasisKept");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("A ship called ");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type("Endurance");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type(" sailed.");
  await expect(scene(page, only).locator("em")).toHaveText("Endurance");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();

  await expect(scene(page, only).locator("em")).toHaveText("Endurance");
  await expect(scene(page, only)).toHaveText("A ship called Endurance sailed.");
});

async function quoting(page: Page, named: string, language: string | null) {
  await aNewProject(page, named);
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await theBookIsIn(page, language);
  await openTheWritingView(page);
  await scene(page, only).click();

  return only;
}

const shown = (page: Page, id: string) =>
  scene(page, id).evaluate((prose) => {
    const marks = (spoken: Element) =>
      [...getComputedStyle(spoken).quotes.matchAll(/"([^"]*)"/g)].map(
        (found) => found[1],
      );
    const read = (node: Node): string =>
      node instanceof Element && node.matches("q.speech")
        ? `${marks(node)[0]}${[...node.childNodes].map(read).join("")}${marks(node)[1]}`
        : node instanceof Element
          ? [...node.childNodes].map(read).join("")
          : (node.textContent ?? "");

    return [...prose.children].map(read).join("\n");
  });

test("a typed \" begins what is said and another ends it, drawn in the book's marks", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingDe", "de");

  await page.keyboard.type('"Komm her," sagte sie. ("Bitte.")');

  await expect
    .poll(() => shown(page, only))
    .toBe("»Komm her,« sagte sie. (»Bitte.«)");
  await expect(scene(page, only).locator("q.speech")).toHaveText([
    "Komm her,",
    "Bitte.",
  ]);
});

test("the opening mark shows as soon as \" is typed", async ({ page }) => {
  const only = await quoting(page, "QuotingBegun", "de");

  await page.keyboard.type('Sie sagte: "');

  await expect(scene(page, only)).toHaveText("Sie sagte: »");
  await page.keyboard.type("Ja.");
  await expect.poll(() => shown(page, only)).toBe("Sie sagte: »Ja.«");
});

test("a book with no language quotes as English does", async ({ page }) => {
  const only = await quoting(page, "QuotingNone", null);

  await page.keyboard.type('"Come here," she said.');

  await expect
    .poll(() => shown(page, only))
    .toBe("“Come here,” she said.");
});

test("French marks are set with narrow spaces inside", async ({ page }) => {
  const only = await quoting(page, "QuotingFr", "fr");

  await page.keyboard.type('"Viens"');

  await expect
    .poll(() => shown(page, only))
    .toBe("« Viens »");
});

test("a typed ' becomes an apostrophe", async ({ page }) => {
  const only = await quoting(page, "Apostrophe", "de");

  await page.keyboard.type("Wie geht's? Don't.");

  await expect(scene(page, only)).toHaveText("Wie geht’s? Don’t.");
});

test("a quotation style chosen while writing redraws what is already said", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingChanged", "de");
  await page.keyboard.type('"Eins" ');
  await expect.poll(() => shown(page, only)).toBe("»Eins« ");

  await page.getByRole("button", { name: "Project details" }).click();
  const details = page.getByRole("dialog", { name: "Project details" });
  await details.getByLabel("Quotation marks").selectOption({ label: "„…“" });
  await details.getByRole("button", { name: "Save" }).click();
  await expect(details).toHaveCount(0);
  await scene(page, only).click();
  await page.keyboard.press("ControlOrMeta+End");
  await page.keyboard.type('"Zwei"');

  await expect.poll(() => shown(page, only)).toBe("„Eins“ „Zwei“");
});

test("what is said stays one pair of marks with emphasis inside it", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingEmphasis", "de");

  await page.keyboard.type('"Das ist ');
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type("meine");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type(' Sache."');

  await expect(scene(page, only).locator("q.speech")).toHaveCount(1);
  await expect
    .poll(() => shown(page, only))
    .toBe("»Das ist meine Sache.«");
});

test("selected text becomes what is said when \" is typed over it", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingSelected", "de");
  await page.keyboard.type("Sie sagte Ja.");
  await page.keyboard.press("End");
  for (let step = 0; step < "Ja.".length; step++) {
    await page.keyboard.press("Shift+ArrowLeft");
  }

  await page.keyboard.type('"');

  await expect.poll(() => shown(page, only)).toBe("Sie sagte »Ja.«");
});

test("copied, what is said comes with the book's marks", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const only = await quoting(page, "QuotingCopied", "de");
  await page.keyboard.type('"Komm her," sagte sie.');
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.press("ControlOrMeta+A");
  await expect(flow(page).locator(".scene-slot.chosen")).toHaveCount(1);

  await page.keyboard.press("ControlOrMeta+C");

  await expect
    .poll(async () =>
      (await page.evaluate(() => navigator.clipboard.readText())).replace(
        /\r\n/g,
        "\n",
      ),
    )
    .toBe("Part One\n\n»Komm her,« sagte sie.");
});


test("copied from within a scene, what is said comes with the book's marks", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await quoting(page, "QuotingCopiedWithin", "de");
  await page.keyboard.type('"Komm her," sagte sie.');
  await page.keyboard.press("Shift+Home");

  await page.keyboard.press("ControlOrMeta+C");

  await expect
    .poll(() => page.evaluate(() => navigator.clipboard.readText()))
    .toBe("»Komm her,« sagte sie.");
  await expect
    .poll(() =>
      page.evaluate(async () => {
        const [item] = await navigator.clipboard.read();
        const html = await (await item.getType("text/html")).text();

        return new DOMParser().parseFromString(html, "text/html").body.textContent;
      }),
    )
    .toBe("»Komm her,« sagte sie.");
});

test("what is said, pasted back in, stays what is said with one pair of marks", async ({
  page,
  context,
}) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const only = await quoting(page, "QuotingPastedBack", "de");
  await page.keyboard.type('"Komm her," sagte sie.');
  await page.keyboard.press("Shift+Home");
  await page.keyboard.press("ControlOrMeta+C");
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");

  await page.keyboard.press("ControlOrMeta+V");

  await expect(scene(page, only).locator("q.speech")).toHaveCount(2);
  expect(await shown(page, only)).toBe(
    "»Komm her,« sagte sie.\n»Komm her,« sagte sie.",
  );
});

test("Ctrl+Shift+2 begins a line of dialogue and ends it again", async ({ page }) => {
  const only = await quoting(page, "DialogueKey", "de");

  await page.keyboard.press("ControlOrMeta+Shift+2");
  await page.keyboard.type("Komm her,");
  await page.keyboard.press("ControlOrMeta+Shift+2");
  await page.keyboard.type(" sagte sie.");

  await expect.poll(() => shown(page, only)).toBe("»Komm her,« sagte sie.");
  await expect(scene(page, only).locator("q.speech")).toHaveText(["Komm her,"]);
});

test("Ctrl+Shift+2 makes selected text dialogue, and plain again", async ({ page }) => {
  const only = await quoting(page, "DialogueKeySelected", "de");
  await page.keyboard.type("Sie sagte Ja.");
  await page.keyboard.press("End");
  for (let step = 0; step < "Ja.".length; step++) {
    await page.keyboard.press("Shift+ArrowLeft");
  }

  await page.keyboard.press("ControlOrMeta+Shift+2");

  await expect.poll(() => shown(page, only)).toBe("Sie sagte »Ja.«");

  await page.keyboard.press("ControlOrMeta+Shift+2");

  await expect.poll(() => shown(page, only)).toBe("Sie sagte Ja.");
  await expect(scene(page, only).locator("q.speech")).toHaveCount(0);
});
