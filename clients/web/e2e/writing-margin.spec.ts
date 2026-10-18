import { test, expect } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject, API } from "./support/shell";
import { aBookInTheWritingView, addHere, asked, flow, gapBelow, heading, label, menuItem, offered, openTheWritingView, savedTitles, scene, sceneActions, sceneOnServer, sectionActions, shown, theBook, titled } from "./support/writing";

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
  const scenes = await aBookInTheWritingView(page, "Opening", { "Part One": 1 });
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
    .poll(() => theBook(page), {
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
  expect(await theBook(page)).toEqual([["Part One", [second]]]);
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
  const scenes = await aBookInTheWritingView(page, "TabBack", { "Part One": 1 });

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
  const scenes = await aBookInTheWritingView(page, "SameType", { "Part One": 1 });

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

test("on a phone, the menu of a place to add opens where it can be seen", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 700 });
  const scenes = await aBookInTheWritingView(page, "PhoneMenu", { "Part One": 1 });

  await addHere(gapBelow(page, scenes["Part One"][0])).click();

  const menu = page.getByRole("menu");
  await expect(menu).toBeVisible();
  const seen = (await menu.boundingBox())!;
  expect(seen.x).toBeGreaterThanOrEqual(0);
  expect(seen.x + seen.width).toBeLessThanOrEqual(375);
});
