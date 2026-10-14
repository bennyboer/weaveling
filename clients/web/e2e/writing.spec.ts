import { test, expect, type Page } from "@playwright/test";

import {
  aLooseScene,
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

async function aBookShaped(
  page: Page,
  sections: { title: string; under?: string; after?: string }[],
) {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });
  const outline = (await opened.json()).id;
  const ids: Record<string, string> = {};

  for (const { title, under, after } of sections) {
    const added = await page.request.post(
      `${API}/outlines/${outline}/sections`,
      {
        data: {
          title,
          under: under ? ids[under] : null,
          after: after ? ids[after] : null,
        },
      },
    );
    expect(added.status(), await added.text()).toBe(201);
    ids[title] = (await added.json()).section;
  }

  return { outline, ids };
}

async function aBookWithScenes(page: Page, sections: Record<string, number>) {
  const titles = Object.keys(sections);
  const { outline, ids } = await aBookShaped(
    page,
    titles.map((title, nth) => ({ title, after: titles[nth - 1] })),
  );
  const scenes: Record<string, string[]> = {};

  for (const title of titles) {
    scenes[title] = [];
    for (let nth = 0; nth < sections[title]; nth++) {
      const scene = await aLooseScene(page);
      const behind = scenes[title].at(-1);
      const attached = await page.request.post(
        `${API}/outlines/${outline}/attachments`,
        {
          data: {
            attachment: { kind: "scene", id: scene },
            section: ids[title],
            after: behind ? { kind: "scene", id: behind } : null,
          },
        },
      );
      expect(attached.ok(), await attached.text()).toBe(true);
      scenes[title].push(scene);
    }
  }

  return scenes;
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

async function projectsOutline(page: Page) {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });

  return opened.json();
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
  expect(await sceneOrder(page)).toEqual([["Part One", [second]]]);
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

const saveState = (page: Page) => statusNote(page).locator(".save-state");

async function holdingWhatIsWritten(page: Page) {
  const held: (string | Buffer)[] = [];
  const servers: { send: (frame: string | Buffer) => void }[] = [];
  let holding = false;

  await page.routeWebSocket(/\/api\/sync\//, (socket) => {
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
      held.splice(0).forEach((frame) => servers[0].send(frame));
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

test("the status note makes room for the alarm beside it", async ({
  page,
}) => {
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
  const stored = await (await page.request.get(`${API}/scenes/${written}`)).json();
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
  const stored = await (await page.request.get(`${API}/scenes/${begun.id}`)).json();
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
  await expect(later).toBeFocused();
  await page.keyboard.type("X");
  await expect(later).toHaveText("AlphaX Beta");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
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
