import { test, expect, type Page } from "@playwright/test";

import { aLooseScene, aNewProject, views } from "./support/shell";

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
