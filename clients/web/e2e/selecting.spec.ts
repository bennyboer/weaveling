import { test, expect, type Page } from "@playwright/test";

import { aLooseScene, aNewProject, API, aSceneMade, scenesCatalogued, views } from "./support/shell";
import { aBookShaped, aBookWithScenes } from "./support/book";
import { aBookInTheWritingView, flow, scene, theBook } from "./support/writing";

const chosen = (page: Page) =>
  flow(page)
    .locator(".scene-slot.chosen")
    .evaluateAll((slots) =>
      slots.map((slot) => slot.getAttribute("data-place-of")),
    );

async function aBook(page: Page, named: string, scenes: number) {
  const made = await aBookInTheWritingView(page, named, { "Part One": scenes });

  return made["Part One"];
}

async function written(page: Page, id: string, text: string) {
  await scene(page, id).click();
  await page.keyboard.type(text);
}

test("Shift+Down at a scene's end chooses the scene, then takes in the next, and Shift+Up gives it back", async ({
  page,
}) => {
  // given: a book of three scenes, the cursor at the end of the first
  const [first, second, third] = await aBook(page, "Down", 3);
  await written(page, first, "The first scene.");

  // when: Shift+Down is pressed
  await page.keyboard.press("Shift+ArrowDown");
  // then: the scene is chosen whole
  await expect.poll(() => chosen(page)).toEqual([first]);
  await expect(
    scene(page, first),
    "the scene is chosen whole, not edited",
  ).not.toBeFocused();

  // when: Shift+Down is pressed again and again
  await page.keyboard.press("Shift+ArrowDown");
  // then: each press takes in the next scene
  await expect.poll(() => chosen(page)).toEqual([first, second]);
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second, third]);

  // when: Shift+Up is pressed
  await page.keyboard.press("Shift+ArrowUp");
  // then: the last scene taken in is given back
  await expect.poll(() => chosen(page)).toEqual([first, second]);
});

test("Shift+Up at a scene's top chooses the scene and takes in those above", async ({
  page,
}) => {
  // given: a book of three scenes, the cursor at the top of the last
  const [first, second, third] = await aBook(page, "Up", 3);
  await written(page, third, "The third scene.");
  await page.keyboard.press("ControlOrMeta+Home");

  // when: Shift+Up is pressed
  await page.keyboard.press("Shift+ArrowUp");
  // then: the scene is chosen whole
  await expect.poll(() => chosen(page)).toEqual([third]);
  // when: Shift+Up is pressed twice more
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");

  // then: the scenes above are taken in
  await expect.poll(() => chosen(page)).toEqual([first, second, third]);
});

test("inside a scene, Shift+Down still selects text as it always did", async ({
  page,
}) => {
  // given: a scene of two lines, the cursor at its top
  const [first] = await aBook(page, "Inside", 2);
  await written(page, first, "One line.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Another line.");
  await page.keyboard.press("ControlOrMeta+Home");

  // when: Shift+Down is pressed
  await page.keyboard.press("Shift+ArrowDown");

  // then: no scene is chosen
  await expect.poll(() => chosen(page)).toEqual([]);
  // and: the text of the first line is selected
  await expect(scene(page, first)).toBeFocused();
  expect(await page.evaluate(() => getSelection()!.toString())).toContain(
    "One line.",
  );
});

test("Ctrl+A chooses the scene's text, and pressed again the whole book", async ({
  page,
}) => {
  // given: the cursor in a scene of a book
  const scenes = await aBook(page, "Everything", 3);
  await written(page, scenes[1], "The middle scene.");

  // when: Ctrl+A is pressed
  await page.keyboard.press("ControlOrMeta+A");
  // then: the scene's text is selected
  await expect.poll(() => chosen(page)).toEqual([]);
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "The middle scene.",
  );

  // when: Ctrl+A is pressed again
  await page.keyboard.press("ControlOrMeta+A");

  // then: the whole book is chosen
  await expect.poll(() => chosen(page)).toEqual(scenes);
});

test("Escape lets the scenes go and puts the cursor back where it was", async ({
  page,
}) => {
  // given: two scenes chosen from the cursor in the first
  const [first, second] = await aBook(page, "Escaping", 2);
  await written(page, first, "Back here");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first, second]);

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: no scene is chosen
  await expect.poll(() => chosen(page)).toEqual([]);
  // and: the cursor is back in the scene the choosing ended in
  await expect(
    scene(page, second),
    "the cursor goes back into the scene the choosing ended in",
  ).toBeFocused();
});

test("an arrow without Shift lets the scenes go too", async ({ page }) => {
  // given: a scene chosen from the cursor in it
  const [first] = await aBook(page, "Arrow", 2);
  await written(page, first, "Here.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  // when: an arrow is pressed without Shift
  await page.keyboard.press("ArrowDown");

  // then: no scene is chosen
  await expect.poll(() => chosen(page)).toEqual([]);
  // and: the cursor is back in the scene, where typing goes on
  await expect(scene(page, first)).toBeFocused();
  await page.keyboard.type(" More.");
  await expect(scene(page, first)).toHaveText("Here. More.");
});

test("clicking into a scene lets the chosen ones go", async ({ page }) => {
  // given: a scene chosen
  const [first, second] = await aBook(page, "Clicking", 2);
  await written(page, first, "Chosen.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  // when: another scene is clicked into
  await scene(page, second).click();

  // then: no scene is chosen
  await expect.poll(() => chosen(page)).toEqual([]);
});

test("typing while scenes are chosen changes nothing in them", async ({
  page,
}) => {
  // given: a scene chosen
  const [first] = await aBook(page, "Typing", 2);
  await written(page, first, "Untouched.");
  await page.keyboard.press("Shift+ArrowDown");
  await expect.poll(() => chosen(page)).toEqual([first]);

  // when: something is typed
  await page.keyboard.type("xyz");

  // then: the scene's text is unchanged
  await expect(scene(page, first)).toHaveText("Untouched.");
});

test("taking in scenes reaches down the book, bringing each into sight", async ({
  page,
}) => {
  // given: a book of fourteen scenes, the cursor in the first
  const scenes = await aBook(page, "FarDown", 14);
  await written(page, scenes[0], "The top.");

  // when: Shift+Down is pressed fourteen times
  for (let step = 0; step < 14; step++) {
    await page.keyboard.press("Shift+ArrowDown");
  }

  // then: every scene is chosen
  await expect.poll(() => chosen(page)).toEqual(scenes);
  // and: the last of them is in sight
  await expect(
    flow(page).locator(`[data-place-of="${scenes[13]}"]`),
  ).toBeInViewport();
});

test("scenes outside the book are chosen the same way", async ({ page }) => {
  // given: two scenes outside the book, the cursor in the first
  await aNewProject(page, "Outside");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  const second = await aLooseScene(page);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(page).toHaveURL(/\/write$/);
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));
  await expect(scene(page, first)).toBeVisible();
  await written(page, first, "Loose.");

  // when: Shift+Down is pressed twice
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  // then: both are chosen
  await expect.poll(() => chosen(page)).toEqual([first, second]);
});

const spannedBySelection = (page: Page) =>
  page.evaluate(() => {
    const shown = getSelection()!;
    if (shown.rangeCount === 0) {
      return [];
    }
    const range = shown.getRangeAt(0);

    return [...document.querySelectorAll(".scene[data-scene]")]
      .filter((scene) => range.intersectsNode(scene))
      .map((scene) => scene.getAttribute("data-scene"));
  });

test("chosen scenes are shown as the browser's own selection, from the first to the last", async ({
  page,
}) => {
  // given: a book of three scenes, with some text selected in the first
  const [first, second, third] = await aBook(page, "Shown", 3);
  await written(page, first, "Some words");
  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowLeft");

  // when: two scenes are chosen from the end of the first
  await page.keyboard.press("End");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  // then: the browser's selection spans them
  await expect.poll(() => spannedBySelection(page)).toEqual([first, second]);
  // and: the book keeps the keys
  await expect(
    flow(page),
    "the keys stay with the book while scenes are chosen",
  ).toBeFocused();

  // when: the next scene is taken in
  await page.keyboard.press("Shift+ArrowDown");
  // then: the browser's selection spans it too
  await expect
    .poll(() => spannedBySelection(page))
    .toEqual([first, second, third]);
});

const headingsChosen = (page: Page) =>
  page.evaluate(() => {
    const shown = getSelection()!;
    if (shown.rangeCount === 0) {
      return [];
    }
    const range = shown.getRangeAt(0);

    return [...document.querySelectorAll(".writing .heading")]
      .filter((heading) => range.intersectsNode(heading))
      .map((heading) => heading.textContent);
  });

test("a heading is a step of its own when choosing across sections", async ({
  page,
}) => {
  // given: a book of two sections of one scene each, the cursor at the top of the second
  await aNewProject(page, "Headings");
  const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = made["Part One"];
  const [second] = made["Part Two"];
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await written(page, second, "In part two.");
  await page.keyboard.press("ControlOrMeta+Home");

  // when: Shift+Up is pressed
  await page.keyboard.press("Shift+ArrowUp");
  // then: the scene is chosen, without the heading above it
  await expect.poll(() => spannedBySelection(page)).toEqual([second]);
  await expect.poll(() => headingsChosen(page)).toEqual([]);

  // when: Shift+Up is pressed again
  await page.keyboard.press("Shift+ArrowUp");
  // then: the heading is taken in
  await expect.poll(() => headingsChosen(page)).toEqual(["Part Two"]);
  await expect.poll(() => spannedBySelection(page)).toEqual([second]);

  // when: Shift+Up is pressed again
  await page.keyboard.press("Shift+ArrowUp");
  // then: the scene above the heading is taken in
  await expect.poll(() => spannedBySelection(page)).toEqual([first, second]);
  await expect.poll(() => headingsChosen(page)).toEqual(["Part Two"]);
});

test("a scene already selected whole as text takes in the next one at the first press beyond it", async ({
  page,
}) => {
  // given: a book of three scenes, the cursor in the last
  const [first, second, third] = await aBook(page, "Whole", 3);
  await written(page, third, "The last scene.");

  // when: Control+Shift+Up is pressed
  await page.keyboard.press("Control+Shift+ArrowUp");
  // then: its text is selected
  expect(await page.evaluate(() => getSelection()!.toString())).toBe(
    "The last scene.",
  );

  // when: Control+Shift+Up is pressed again
  await page.keyboard.press("Control+Shift+ArrowUp");

  // then: the scene and the one above are chosen
  await expect.poll(() => chosen(page)).toEqual([second, third]);
  // when: Control+Shift+Up is pressed again
  await page.keyboard.press("Control+Shift+ArrowUp");
  // then: the first scene is taken in
  await expect.poll(() => chosen(page)).toEqual([first, second, third]);
});

test("ending a choosing on a heading puts the cursor in that heading", async ({
  page,
}) => {
  // given: a book of two sections, with the second's heading and scene chosen
  await aNewProject(page, "EndOnHeading");
  const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [second] = made["Part Two"];
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await written(page, second, "Text.");
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the cursor is in the heading
  await expect(
    flow(page).getByRole("heading", { name: "Part Two" }),
  ).toBeFocused();
});

test("in an empty scene, Shift+Down chooses only that scene", async ({
  page,
}) => {
  // given: the cursor in an empty scene
  const [first, second] = await aBook(page, "EmptyScene", 3);
  await scene(page, first).click();

  // when: Shift+Down is pressed
  await page.keyboard.press("Shift+ArrowDown");

  // then: only that scene is chosen
  await expect.poll(() => chosen(page)).toEqual([first]);
  // when: Shift+Down is pressed again
  await page.keyboard.press("Shift+ArrowDown");
  // then: the next scene is taken in
  await expect.poll(() => chosen(page)).toEqual([first, second]);
});

const sceneExists = async (page: Page, id: string) =>
  (await page.request.get(`${API}/scenes/${id}`)).ok();

const removing = (page: Page) =>
  page.getByRole("dialog", { name: "Remove what is chosen?" });

test("Delete asks first, naming what is chosen, and Cancel keeps it chosen", async ({
  page,
}) => {
  // given: two scenes chosen
  const [first, second] = await aBook(page, "Asking", 3);
  await written(page, first, "One.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");

  // when: Delete is pressed
  await page.keyboard.press("Delete");

  // then: the question names what is chosen and how it can go
  await expect(removing(page)).toContainText("2 scenes are chosen.");
  await expect(removing(page).getByRole("button")).toHaveText([
    "Cancel",
    "Take out of the book",
    "Delete for good",
  ]);
  // when: the question is cancelled
  await removing(page).getByRole("button", { name: "Cancel" }).click();

  // then: it closes
  await expect(removing(page)).toHaveCount(0);
  // and: the scenes stay chosen
  await expect.poll(() => chosen(page)).toEqual([first, second]);
  await page.keyboard.press("Shift+ArrowUp");
  await expect.poll(() => chosen(page)).toEqual([first]);
});

test("Escape closes the question and leaves the scenes chosen", async ({
  page,
}) => {
  // given: a scene chosen, with the question of removing it open
  const [first] = await aBook(page, "EscapeAsking", 2);
  await written(page, first, "One.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Backspace");
  await expect(removing(page)).toBeVisible();

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the question closes
  await expect(removing(page)).toHaveCount(0);
  // and: the scene stays chosen
  await expect.poll(() => chosen(page)).toEqual([first]);
});

test("taken out of the book, the chosen scenes wait among those not in it", async ({
  page,
}) => {
  // given: two of three scenes chosen and asked about
  const [first, second, third] = await aBook(page, "TakenOut", 3);
  await written(page, first, "One.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Delete");

  // when: they are taken out of the book
  await removing(page)
    .getByRole("button", { name: "Take out of the book" })
    .click();

  // then: the book keeps only the third
  await expect.poll(() => theBook(page)).toEqual([["Part One", [third]]]);
  // and: the two scenes still exist
  expect(await sceneExists(page, first)).toBe(true);
  expect(await sceneExists(page, second)).toBe(true);
  // and: they wait among those not in the book
  await expect(flow(page).locator(`[data-scene="${first}"]`)).toHaveCount(0);
  await expect(
    page.getByRole("link", { name: /Not in the book/ }),
  ).toContainText("2");
});

test("deleted for good, the chosen scenes are gone", async ({ page }) => {
  // given: two of three scenes chosen and asked about
  const [first, second, third] = await aBook(page, "ForGood", 3);
  await written(page, second, "Two.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Delete");

  // when: they are deleted for good
  await removing(page).getByRole("button", { name: "Delete for good" }).click();

  // then: the book keeps only the first
  await expect.poll(() => theBook(page)).toEqual([["Part One", [first]]]);
  // and: the two scenes are gone
  await expect.poll(() => sceneExists(page, second)).toBe(false);
  await expect.poll(() => sceneExists(page, third)).toBe(false);
});

test("a chosen heading removes its section with the sections inside it and all their scenes", async ({
  page,
}) => {
  // given: a book of sections, one inside another, each with a scene
  await aNewProject(page, "Sections");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "Inside Two", under: "Part Two" },
  ]);
  const scenes: Record<string, string> = {};
  for (const title of ["Part One", "Part Two", "Inside Two"]) {
    const scene = await aSceneMade(page);
    const attached = await page.request.post(
      `${API}/outlines/${outline}/attachments`,
      {
        data: {
          attachment: { kind: "scene", id: scene },
          section: ids[title],
          after: null,
        },
      },
    );
    expect(attached.ok()).toBe(true);
    scenes[title] = scene;
  }
  await scenesCatalogued(page, Object.values(scenes));
  // and: the headings and scenes of the second section chosen
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await written(page, scenes["Part Two"], "Two.");
  await page.keyboard.press("ControlOrMeta+Home");
  await page.keyboard.press("Shift+ArrowUp");
  await page.keyboard.press("Shift+ArrowUp");

  // when: Delete is pressed
  await page.keyboard.press("Delete");
  // then: the question names the scenes and sections chosen
  await expect(removing(page).locator("p")).toHaveText(
    "2 scenes and 2 sections are chosen. Each section goes with the sections inside it and all their scenes. Taken out of the book, the scenes wait among those not in it. Deleted for good, they and everything written in them are gone, and that cannot be undone.",
  );
  // when: they are taken out of the book
  await removing(page)
    .getByRole("button", { name: "Take out of the book" })
    .click();

  // then: the book keeps only the first section
  await expect
    .poll(() => theBook(page))
    .toEqual([["Part One", [scenes["Part One"]]]]);
  // and: the scene of the section inside still exists
  expect(await sceneExists(page, scenes["Inside Two"])).toBe(true);
});

test("headings chosen without a scene among them are asked about as sections alone", async ({
  page,
}) => {
  // given: a book of two sections without scenes
  await aNewProject(page, "HeadingsAlone");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  // and: both headings chosen
  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await flow(page)
    .getByRole("heading", { name: "Part Two" })
    .click({ modifiers: ["Shift"] });

  // when: Delete is pressed
  await page.keyboard.press("Delete");

  // then: the question names the sections alone
  await expect(removing(page).locator("p")).toHaveText(
    "2 sections are chosen. Each section goes with the sections inside it and all their scenes. Taken out of the book, the scenes wait among those not in it. Deleted for good, they and everything written in them are gone, and that cannot be undone.",
  );
});

test("outside the book, chosen scenes can only be deleted for good", async ({
  page,
}) => {
  // given: a scene outside the book, chosen
  await aNewProject(page, "LooseDelete");
  await aBookWithScenes(page, { "Part One": 1 });
  const first = await aLooseScene(page);
  await aLooseScene(page);
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(page).toHaveURL(/\/write$/);
  await page.goto(page.url().replace(/\/write$/, "/write/loose"));
  await written(page, first, "Loose.");
  await page.keyboard.press("Shift+ArrowDown");

  // when: Delete is pressed
  await page.keyboard.press("Delete");

  // then: the question offers only deleting for good
  await expect(removing(page).locator("p")).toHaveText(
    "1 scene is chosen. Deleted for good, they and everything written in them are gone, and that cannot be undone.",
  );
  await expect(removing(page).getByRole("button")).toHaveText([
    "Cancel",
    "Delete for good",
  ]);
  // when: it is deleted for good
  await removing(page).getByRole("button", { name: "Delete for good" }).click();
  // then: the scene is gone
  await expect.poll(() => sceneExists(page, first)).toBe(false);
});

test("scenes begun in the view, taken out of the book, show among those outside it at once, before the catalog has heard of them", async ({
  page,
}) => {
  // given: a scene begun in the view
  const [first] = await aBook(page, "BegunHere", 1);
  await written(page, first, "Existing.");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Begun right here.");
  await expect.poll(async () => (await theBook(page))[0][1].length).toBe(2);
  const [, begun] = (await theBook(page))[0][1];
  await expect(scene(page, begun)).toHaveText("Begun right here.");
  // and: the catalog not yet aware of it
  await page.keyboard.press("ControlOrMeta+End");
  await page.route("**/api/scenes?project=*", async (route) => {
    const response = await route.fetch();
    const listed = await response.json();
    await route.fulfill({
      response,
      json: {
        ...listed,
        scenes: listed.scenes.filter((held: { id: string }) => held.id !== begun),
      },
    });
  });

  // when: it is chosen and taken out of the book
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Delete");
  await removing(page)
    .getByRole("button", { name: "Take out of the book" })
    .click();

  // then: it shows among those outside the book
  const outside = page.getByRole("link", { name: /Not in the book/ });
  await expect(outside).toBeVisible();
  await outside.click();
  await expect(scene(page, begun)).toHaveText("Begun right here.");
});

async function copied(page: Page) {
  await page.keyboard.press("ControlOrMeta+C");

  return (await page.evaluate(() => navigator.clipboard.readText())).replace(
    /\r\n/g,
    "\n",
  );
}

test.describe("copying what is chosen", () => {
  test.describe.configure({ mode: "serial" });

  test.beforeEach(async ({ context }) => {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  });

  test("copies the chosen scenes' text in reading order, a blank line between them", async ({
    page,
  }) => {
    // given: two scenes chosen, the second of two lines
    const [first, second] = await aBook(page, "Copying", 3);
    await written(page, second, "Second, line one.");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Second, line two.");
    await written(page, first, "First.");
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Shift+ArrowDown");
    await expect.poll(() => chosen(page)).toEqual([first, second]);
    // and: something else on the clipboard
    await page.evaluate(() => navigator.clipboard.writeText("before"));

    // expect: copying gives their text in reading order, a blank line between
    await expect
      .poll(() => copied(page))
      .toBe("First.\n\nSecond, line one.\nSecond, line two.");
  });

  test("with nothing chosen, copying text in a scene copies just that text", async ({
    page,
  }) => {
    // given: some text selected in a scene, with nothing chosen
    const [first] = await aBook(page, "OrdinaryCopy", 2);
    await written(page, first, "Keep this and not that.");
    await page.keyboard.press("Home");
    for (let step = 0; step < "Keep this".length; step++) {
      await page.keyboard.press("Shift+ArrowRight");
    }

    // expect: copying gives just that text
    await expect.poll(() => copied(page)).toBe("Keep this");
  });

  test("a chosen heading is copied as a line of its own", async ({ page }) => {
    // given: two sections of a scene each, chosen across the heading between them
    await aNewProject(page, "CopyingHeadings");
    const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
    await views(page).getByRole("link", { name: "Write", exact: true }).click();
    await written(page, made["Part One"][0], "One.");
    await written(page, made["Part Two"][0], "Two.");
    await page.keyboard.press("ControlOrMeta+Home");
    await page.keyboard.press("Shift+ArrowUp");
    await page.keyboard.press("Shift+ArrowUp");
    await page.keyboard.press("Shift+ArrowUp");

    // expect: copying gives the heading as a line of its own
    await expect.poll(() => copied(page)).toBe("One.\n\nPart Two\n\nTwo.");
  });

  test("a scene far down the book, not drawn, is copied all the same", async ({
    page,
  }) => {
    // given: the last scene of a long book written
    const scenes = await aBook(page, "CopyingFar", 60);
    const last = scenes[59];
    await flow(page)
      .locator(`[data-place-of="${last}"]`)
      .scrollIntoViewIfNeeded();
    await written(page, last, "Far down the book.");
    await expect
      .poll(
        async () =>
          (await (await page.request.get(`${API}/scenes/${last}`)).json()).text,
      )
      .toBe("Far down the book.");
    // and: the cursor in the first scene, far from the last
    await page.reload();
    await written(page, scenes[0], "At the top.");
    await expect(
      flow(page).locator(`[data-place-of="${last}"] .ProseMirror`),
      "the last scene is far enough away not to be drawn",
    ).toHaveCount(0);

    // when: the whole book is chosen
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.press("ControlOrMeta+A");
    await expect.poll(async () => (await chosen(page)).length).toBe(60);

    // then: copying gives the far scene's text too
    await expect
      .poll(async () => (await copied(page)).split("\n\n"))
      .toEqual(expect.arrayContaining(["At the top.", "Far down the book."]));
  });

  test("a far scene that cannot be read is told, rather than copied as nothing", async ({
    page,
  }) => {
    // given: the cursor in the first scene of a long book
    const scenes = await aBook(page, "CopyingRefused", 60);
    await written(page, scenes[0], "At the top.");
    // and: the last scene unreadable
    await page.route(`**/api/scenes/${scenes[59]}`, (route) =>
      route.fulfill({ status: 500, body: "{}" }),
    );
    // and: something else on the clipboard
    await page.evaluate(() => navigator.clipboard.writeText("before"));

    // when: the whole book is chosen and copied
    await page.keyboard.press("ControlOrMeta+A");
    await page.keyboard.press("ControlOrMeta+A");
    await expect.poll(async () => (await chosen(page)).length).toBe(60);
    await page.keyboard.press("ControlOrMeta+C");

    // then: an alert is shown
    await expect(flow(page).getByRole("alert")).toBeVisible();
    // and: the clipboard is left as it was
    expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
      "before",
    );
  });
});

test.describe("choosing with the mouse", () => {
  const middleOf = async (page: Page, id: string) => {
    const seen = (await scene(page, id).boundingBox())!;

    return { x: seen.x + 40, y: seen.y + seen.height / 2 };
  };

  async function dragging(page: Page, from: string, to: string[]) {
    const start = await middleOf(page, from);
    await page.mouse.move(start.x, start.y);
    await page.mouse.down();
    await page.mouse.move(start.x + 40, start.y, { steps: 3 });
    for (const over of to) {
      const next = await middleOf(page, over);
      await page.mouse.move(next.x, next.y, { steps: 8 });
    }
  }

  test("dragging from one scene into others chooses them all, and they stay chosen once let go", async ({
    page,
  }) => {
    // given: three scenes with text
    const [first, second, third] = await aBook(page, "Dragging", 3);
    await written(page, first, "First scene.");
    await written(page, second, "Second scene.");
    await written(page, third, "Third scene.");

    // when: dragging from the first into the last
    await dragging(page, first, [third]);
    // then: all three are chosen
    await expect.poll(() => chosen(page)).toEqual([first, second, third]);
    // when: the mouse is let go
    await page.mouse.up();

    // then: they stay chosen
    await expect.poll(() => chosen(page)).toEqual([first, second, third]);
    // and: the browser's selection spans them
    await expect
      .poll(() => spannedBySelection(page))
      .toEqual([first, second, third]);
    // and: the book keeps the keys
    await expect(flow(page), "the keys are with the book").toBeFocused();
    // when: Shift+Up is pressed
    await page.keyboard.press("Shift+ArrowUp");
    // then: the last is given back
    await expect.poll(() => chosen(page)).toEqual([first, second]);
  });

  test("dragging back gives scenes back", async ({ page }) => {
    // given: three scenes with text
    const [first, second, third] = await aBook(page, "DraggingBack", 3);
    await written(page, first, "First scene.");
    await written(page, second, "Second scene.");
    await written(page, third, "Third scene.");

    // when: dragging from the first to the last and back to the second
    await dragging(page, first, [third, second]);
    await page.mouse.up();

    // then: the first two are chosen
    await expect.poll(() => chosen(page)).toEqual([first, second]);
  });

  test("a drag within one scene selects its text as it always did", async ({
    page,
  }) => {
    // given: a scene with text
    const [first] = await aBook(page, "DraggingInside", 2);
    await written(page, first, "Only these words here.");
    const seen = (await scene(page, first).boundingBox())!;

    // when: dragging within it
    await page.mouse.move(seen.x + 2, seen.y + seen.height / 2);
    await page.mouse.down();
    await page.mouse.move(seen.x + 90, seen.y + seen.height / 2, {
      steps: 6,
    });
    await page.mouse.up();

    // then: no scene is chosen
    await expect.poll(() => chosen(page)).toEqual([]);
    // and: its text is selected
    await expect(scene(page, first)).toBeFocused();
    expect(
      (await page.evaluate(() => getSelection()!.toString())).length,
    ).toBeGreaterThan(0);
  });

  test("a drag across a section takes its heading in", async ({ page }) => {
    // given: two sections of a scene each
    await aNewProject(page, "DraggingHeadings");
    const made = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
    await views(page).getByRole("link", { name: "Write", exact: true }).click();
    await written(page, made["Part One"][0], "One.");
    await written(page, made["Part Two"][0], "Two.");

    // when: dragging from the first scene into the second
    await dragging(page, made["Part One"][0], [made["Part Two"][0]]);
    await page.mouse.up();

    // then: the heading between them is taken in
    await expect.poll(() => headingsChosen(page)).toEqual(["Part Two"]);
    // and: both scenes are chosen
    await expect
      .poll(() => chosen(page))
      .toEqual([made["Part One"][0], made["Part Two"][0]]);
  });

  test("once let go, moving the mouse changes nothing", async ({ page }) => {
    // given: two of three scenes chosen with the mouse, and let go
    const [first, second, third] = await aBook(page, "LetGo", 3);
    await written(page, first, "First scene.");
    await written(page, second, "Second scene.");
    await written(page, third, "Third scene.");
    await dragging(page, first, [second]);
    await page.mouse.up();
    await expect.poll(() => chosen(page)).toEqual([first, second]);

    // when: the mouse moves over the third
    const far = await middleOf(page, third);
    await page.mouse.move(far.x, far.y, { steps: 6 });

    // then: the chosen scenes are unchanged
    await expect.poll(() => chosen(page)).toEqual([first, second]);
  });

  test("moving on within the last scene keeps every chosen scene selected", async ({
    page,
  }) => {
    // given: three scenes with text
    const [first, second, third] = await aBook(page, "WithinLast", 3);
    await written(page, first, "First scene.");
    await written(page, second, "Second scene.");
    await written(page, third, "Third scene, with a few more words in it.");
    // and: dragging from the first into the last
    await dragging(page, first, [third]);

    // when: the mouse moves on within the last scene and is let go
    const last = (await scene(page, third).boundingBox())!;
    await page.mouse.move(last.x + last.width - 20, last.y + last.height / 2, {
      steps: 8,
    });
    await page.mouse.up();

    // then: the browser's selection still spans every chosen scene
    await expect
      .poll(() => spannedBySelection(page))
      .toEqual([first, second, third]);
  });

  test("Shift+click in another scene chooses from the cursor's scene to that one", async ({
    page,
  }) => {
    // given: three scenes with text, the cursor in the first
    const [first, second, third] = await aBook(page, "ShiftClick", 3);
    await written(page, first, "First scene.");
    await written(page, second, "Second scene.");
    await written(page, third, "Third scene.");
    await scene(page, first).click();

    // when: the third is Shift+clicked
    const far = await middleOf(page, third);
    await page.keyboard.down("Shift");
    await page.mouse.click(far.x, far.y);
    await page.keyboard.up("Shift");

    // then: all three are chosen
    await expect.poll(() => chosen(page)).toEqual([first, second, third]);
    // and: the browser's selection spans them
    await expect
      .poll(() => spannedBySelection(page))
      .toEqual([first, second, third]);

    // when: the second is Shift+clicked
    const back = await middleOf(page, second);
    await page.keyboard.down("Shift");
    await page.mouse.click(back.x, back.y);
    await page.keyboard.up("Shift");
    // then: the third is given back
    await expect.poll(() => chosen(page)).toEqual([first, second]);
  });

  test("Shift+click within the cursor's scene still selects its text", async ({
    page,
  }) => {
    // given: a scene with text, the cursor at its start
    const [first] = await aBook(page, "ShiftClickInside", 2);
    await written(page, first, "Only these words here.");
    await page.keyboard.press("Home");
    const seen = (await scene(page, first).boundingBox())!;

    // when: Shift+click further in it
    await page.keyboard.down("Shift");
    await page.mouse.click(seen.x + 90, seen.y + seen.height / 2);
    await page.keyboard.up("Shift");

    // then: no scene is chosen
    await expect.poll(() => chosen(page)).toEqual([]);
    // and: its text is selected
    expect(
      (await page.evaluate(() => getSelection()!.toString())).length,
    ).toBeGreaterThan(0);
  });
});
