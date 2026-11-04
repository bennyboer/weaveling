import { test, expect } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject, API } from "./support/shell";
import { aBookInTheWritingView, addHere, asked, flow, gapBelow, heading, label, menuItem, offered, openTheWritingView, savedTitles, scene, sceneActions, sceneOnServer, sectionActions, shown, theBook, titled } from "./support/writing";

test("a scene's title stands in the margin, level with the scene", async ({
  page,
}) => {
  // given: a named scene
  await aNewProject(page, "Margin");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  // when: the writing view is opened
  await openTheWritingView(page);

  // then: its title stands in the margin
  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  await expect(label(page, scenes["Part One"][0])).not.toHaveClass(/untitled/);
  // and: level with the scene, clear of the text
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
  // given: an untitled scene open in the writing view
  const scenes = await aBookInTheWritingView(page, "Opening", { "Part One": 1 });
  // then: the margin shows nothing for it
  const shown = label(page, scenes["Part One"][0]);
  await expect(shown).toHaveText("");

  // when: it is written in
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");

  // then: the margin shows its opening words
  await expect(shown).toHaveText("The loom stood silent.");
  await expect(shown).toHaveClass(/untitled/);
  // when: more is written
  await page.keyboard.type(" It had stood so since spring, under dust.");
  // then: the margin shows only the first few
  await expect(shown, "a margin holds a few words, not the scene").toHaveText(
    "The loom stood silent. It had stood so s…",
  );
});

test("on a narrow window the margin folds above its scene", async ({
  page,
}) => {
  // given: a narrow window
  await page.setViewportSize({ width: 800, height: 900 });
  // and: a named scene
  await aNewProject(page, "Narrow");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  // when: the writing view is opened
  await openTheWritingView(page);

  // then: its title stands above the scene
  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  const shown = (await label(page, scenes["Part One"][0]).boundingBox())!;
  const written = (await scene(page, scenes["Part One"][0]).boundingBox())!;
  expect(shown.y + shown.height).toBeLessThanOrEqual(written.y + 1);
  expect(Math.abs(shown.x - written.x)).toBeLessThanOrEqual(2);
});

test("a scene's title in the margin is only to be read", async ({ page }) => {
  // given: a named scene open in the writing view
  await aNewProject(page, "ReadOnly");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await titled(page, scenes["Part One"][0], "The arrival");
  await openTheWritingView(page);

  // when: its title in the margin is clicked and typed into
  await label(page, scenes["Part One"][0]).click();
  await page.keyboard.type("x");

  // then: the title stays as it was
  await expect(label(page, scenes["Part One"][0])).toHaveText("The arrival");
  // and: it cannot be edited
  await expect(label(page, scenes["Part One"][0])).not.toHaveAttribute(
    "contenteditable",
  );
});

test("a scene is renamed from its menu, right where its title stands", async ({
  page,
}) => {
  // given: a named scene open in the writing view
  await aNewProject(page, "Renaming");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await titled(page, arrival, "The arrival");
  await openTheWritingView(page);
  await expect(label(page, arrival)).toHaveText("The arrival");

  // when: renaming is chosen from its menu
  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  // then: its title can be changed where it stands
  const field = flow(page).getByRole("textbox", { name: "Scene title" });
  await expect(field).toBeFocused();
  await expect(field).toHaveValue("The arrival");
  // when: a new title is entered
  await field.fill("The departure");
  await page.keyboard.press("Enter");

  // then: the margin shows it
  await expect(label(page, arrival)).toHaveText("The departure");
  await expect(field).toHaveCount(0);
  // and: it is saved
  await expect
    .poll(async () => (await (await sceneOnServer(page, arrival)).json()).title)
    .toBe("The departure");
});

test("Escape leaves a scene's title as it was", async ({ page }) => {
  // given: a named scene open in the writing view
  await aNewProject(page, "KeepTitle");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await titled(page, arrival, "The arrival");
  await openTheWritingView(page);

  // when: it is renamed and Escape pressed
  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Something else");
  await page.keyboard.press("Escape");

  // then: the margin shows the old title
  await expect(label(page, arrival)).toHaveText("The arrival");
  // and: the old title is still saved
  expect((await (await sceneOnServer(page, arrival)).json()).title).toBe(
    "The arrival",
  );
});

test("a scene taken out of the book leaves the flow but is kept", async ({
  page,
}) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "TakeOut");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();

  // when: the first is taken out of the book
  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();

  // then: it leaves the flow
  await expect(scene(page, first)).toHaveCount(0);
  await expect(scene(page, second)).toBeVisible();
  // and: the book is saved without it
  await expect
    .poll(() => theBook(page), {
      message: "the flow lets go of the scene before the server has it out",
    })
    .toEqual([["Part One", [second]]]);
  // and: the scene is kept
  expect((await sceneOnServer(page, first)).status()).toBe(200);
});

test("deleting a scene asks first, and Cancel keeps it", async ({ page }) => {
  // given: a named scene open in the writing view
  await aNewProject(page, "DeleteCancel");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await titled(page, only, "The arrival");
  await openTheWritingView(page);

  // when: deleting it is chosen
  await sceneActions(page, only).click();
  await menuItem(page, "Delete scene").click();
  // then: the question names it
  await expect(asked(page)).toContainText("“The arrival”");
  // when: it is cancelled
  await asked(page).getByRole("button", { name: "Cancel" }).click();

  // then: the question goes
  await expect(asked(page)).toHaveCount(0);
  // and: the scene is kept
  await expect(scene(page, only)).toBeVisible();
  expect((await sceneOnServer(page, only)).status()).toBe(200);
});

test("a deleted scene is gone from the book and for good", async ({ page }) => {
  // given: a book of two scenes open in the writing view
  await aNewProject(page, "DeleteScene");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();

  // when: the first is deleted
  await sceneActions(page, first).click();
  await menuItem(page, "Delete scene").click();
  await asked(page).getByRole("button", { name: "Delete" }).click();

  // then: it is gone from the book
  await expect(scene(page, first)).toHaveCount(0);
  await expect(scene(page, second)).toBeVisible();
  expect(await theBook(page)).toEqual([["Part One", [second]]]);
  // and: gone for good
  expect((await sceneOnServer(page, first)).status()).toBe(404);
});

test("an empty section is removed at once", async ({ page }) => {
  // given: two empty sections open in the writing view
  await aNewProject(page, "RemoveEmpty");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);
  await expect(heading(page, "Part Two")).toBeVisible();

  // when: removing the second is chosen
  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();

  // then: it is removed without asking
  await expect(heading(page, "Part Two")).toHaveCount(0);
  await expect(asked(page)).toHaveCount(0);
  expect(await savedTitles(page)).toEqual(["Part One"]);
});

test("removing a section that holds scenes asks first, and keeps the scenes", async ({
  page,
}) => {
  // given: two sections with a scene each, open in the writing view
  await aNewProject(page, "RemoveHolding");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [kept] = scenes["Part Two"];
  await openTheWritingView(page);
  await expect(scene(page, kept)).toBeVisible();

  // when: removing the second is chosen
  await sectionActions(page, "Part Two").click();
  await menuItem(page, "Remove section").click();
  // then: it asks first
  await expect(asked(page)).toContainText("Remove this section?");
  // when: it is removed
  await asked(page).getByRole("button", { name: "Remove" }).click();

  // then: it and its scene leave the book
  await expect(heading(page, "Part Two")).toHaveCount(0);
  await expect(scene(page, kept)).toHaveCount(0);
  expect(await savedTitles(page)).toEqual(["Part One"]);
  // and: the scene is kept
  expect((await sceneOnServer(page, kept)).status()).toBe(200);
});

test("a margin menu closes on Escape and on a click elsewhere", async ({
  page,
}) => {
  // given: a scene open in the writing view
  await aNewProject(page, "MenuCloses");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  // when: its menu is opened and Escape pressed
  await sceneActions(page, only).click();
  await expect(menuItem(page, "Rename")).toBeVisible();
  await page.keyboard.press("Escape");
  // then: the menu closes
  await expect(menuItem(page, "Rename")).toHaveCount(0);

  // when: its menu is opened and a heading clicked
  await sceneActions(page, only).click();
  await expect(menuItem(page, "Rename")).toBeVisible();
  await heading(page, "Part One").click();
  // then: the menu closes
  await expect(menuItem(page, "Rename")).toHaveCount(0);
  // and: its button says so
  await expect(sceneActions(page, only)).toHaveAttribute(
    "aria-expanded",
    "false",
  );
});

test("Shift+Tab from a scene goes back to its heading, past the margin", async ({
  page,
}) => {
  // given: a scene open in the writing view
  const scenes = await aBookInTheWritingView(page, "TabBack", { "Part One": 1 });

  // when: Shift+Tab is pressed in it
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.press("Shift+Tab");

  // then: its heading has the cursor
  await expect(heading(page, "Part One")).toBeFocused();
});

test("Shift+F10 opens the menu of the scene being written, and Escape goes back to it", async ({
  page,
}) => {
  // given: a scene open in the writing view
  await aNewProject(page, "MenuKeys");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  // when: Shift+F10 is pressed in it
  await scene(page, only).click();
  await page.keyboard.press("Shift+F10");
  // then: its menu opens, the first item chosen
  await expect(menuItem(page, "Rename")).toBeFocused();
  // and: the arrows move through it
  await page.keyboard.press("ArrowDown");
  await expect(menuItem(page, "Take out of the book")).toBeFocused();
  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the menu closes
  await expect(menuItem(page, "Rename")).toHaveCount(0);
  // and: the scene has the cursor again
  await expect(scene(page, only)).toBeFocused();
});

test("the scene's and the section's menu buttons name Shift+F10, the add menus do not", async ({
  page,
}) => {
  // given: a scene open in the writing view
  await aNewProject(page, "MenuNamesItsKey");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  // then: the scene's menu button names Shift+F10
  await expect(sceneActions(page, only)).toHaveAttribute(
    "aria-keyshortcuts",
    "Shift+F10",
  );
  await expect(sceneActions(page, only)).toHaveAttribute(
    "title",
    "Scene actions (Shift+F10)",
  );
  // and: the section's does
  await expect(sectionActions(page, "Part One")).toHaveAttribute(
    "title",
    "Section actions (Shift+F10)",
  );
  // and: the add menus do not
  await expect(
    addHere(gapBelow(page, only)),
    "Shift+F10 does not open the menus that add",
  ).not.toHaveAttribute("aria-keyshortcuts");
});

test("Shift+F10 in a heading opens its section's menu", async ({ page }) => {
  // given: a section open in the writing view
  await aNewProject(page, "HeadingKeys");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  // when: Shift+F10 is pressed in its heading
  await heading(page, "Part One").click();
  await page.keyboard.press("Shift+F10");

  // then: its menu opens
  await expect(menuItem(page, "Remove section")).toBeFocused();
});

test("a scene renamed from the keyboard hands the cursor back to the scene", async ({
  page,
}) => {
  // given: a scene open in the writing view
  await aNewProject(page, "RenameKeys");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = scenes["Part One"];
  await openTheWritingView(page);

  // when: it is renamed from the keyboard
  await scene(page, only).click();
  await page.keyboard.press("Shift+F10");
  await page.keyboard.press("Enter");
  await page.keyboard.type("The arrival");
  await page.keyboard.press("Enter");

  // then: the margin shows the new name
  await expect(label(page, only)).toHaveText("The arrival");
  // and: the scene has the cursor again
  await expect(scene(page, only)).toBeFocused();
});

test("the margin lines up with the letters beside it", async ({ page }) => {
  // given: nested sections and a named scene being written in
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

  // when: the margin and the text beside it are measured
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

  // then: a heading's menu button and bar sit on its capitals
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
  // and: a scene's title stands on the baseline of its first line, its menu button on its capitals
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
  // given: a scene with a long title open in the writing view
  await aNewProject(page, "OnTop");
  const scenes = await aBookWithScenes(page, { Einleitung: 1 });
  const [only] = scenes["Einleitung"];
  await titled(page, only, "A title long enough to reach under the menu");
  await openTheWritingView(page);
  await expect(label(page, only)).toContainText("A title long enough");

  // when: a section's menu is opened
  await sectionActions(page, "Einleitung").click();
  const offered = menuItem(page, "Remove section");
  await expect(offered).toBeVisible();

  // then: nothing draws over it
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
  // given: a scene open in the writing view
  const scenes = await aBookInTheWritingView(page, "SameType", { "Part One": 1 });

  // when: deleting it is asked
  await sceneActions(page, scenes["Part One"][0]).click();
  await menuItem(page, "Delete scene").click();
  const told = asked(page).locator("p");
  await expect(told).toBeVisible();

  // then: the question is in the same type as every other dialog
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

test("on a phone, the menu of a place to add opens where it can be seen", async ({
  page,
}) => {
  // given: a scene open in the writing view on a phone
  await page.setViewportSize({ width: 375, height: 700 });
  const scenes = await aBookInTheWritingView(page, "PhoneMenu", { "Part One": 1 });

  // when: something is to be added below it
  await addHere(gapBelow(page, scenes["Part One"][0])).click();

  // then: the menu opens within the window
  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const seen = (await menu.boundingBox())!;
  expect(seen.x).toBeGreaterThanOrEqual(0);
  expect(seen.x + seen.width).toBeLessThanOrEqual(375);
});
