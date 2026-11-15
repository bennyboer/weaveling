import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { openTheWritingView, scene } from "./support/writing";

const toolbar = (page: Page) => page.getByRole("toolbar", { name: "Formatting" });

/// Writes a sentence into a scene of a new book, and selects its first word with the keyboard.
async function aWordSelected(page: Page, named: string): Promise<string> {
  await aNewProject(page, named);
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await page.keyboard.type("She never returned.");
  await page.keyboard.press("Home");
  for (let step = 0; step < "She".length; step++) {
    await page.keyboard.press("Shift+ArrowRight");
  }

  return only;
}

test("selected text shows the formatting toolbar, which sets it in bold and takes the formatting off again", async ({
  page,
}) => {
  // given: the first word of a scene's text selected
  const only = await aWordSelected(page, "Toolbar");

  // then: the toolbar shows above the selection, nothing pressed
  await expect(toolbar(page)).toBeVisible();
  await expect(toolbar(page).getByRole("button", { name: "Bold" })).toHaveAttribute("aria-pressed", "false");

  // when: bold is chosen in it
  await toolbar(page).getByRole("button", { name: "Bold" }).click();

  // then: the word is in bold, the button pressed, and the text keeps the focus
  await expect(scene(page, only).locator("strong")).toHaveText("She");
  await expect(toolbar(page).getByRole("button", { name: "Bold" })).toHaveAttribute("aria-pressed", "true");
  await expect(scene(page, only)).toBeFocused();

  // when: italic is chosen too, and then the formatting is taken off
  await toolbar(page).getByRole("button", { name: "Italic" }).click();
  await expect(scene(page, only).locator("em")).toHaveText("She");
  await toolbar(page).getByRole("button", { name: "Remove formatting" }).click();

  // then: the word is plain again
  await expect(scene(page, only).locator("strong, em")).toHaveCount(0);
  await expect(scene(page, only)).toContainText("She never returned.");
});

test("the toolbar is reached with Alt+F10, walked with the arrows, and left with Escape", async ({
  page,
}) => {
  // given: the first word of a scene's text selected
  const only = await aWordSelected(page, "Keys");

  // when: Alt+F10 is pressed
  await page.keyboard.press("Alt+F10");

  // then: the first button of the toolbar has the focus
  await expect(toolbar(page).getByRole("button", { name: "Bold" })).toBeFocused();

  // when: the next button is chosen with the arrow and pressed
  await page.keyboard.press("ArrowRight");
  await expect(toolbar(page).getByRole("button", { name: "Italic" })).toBeFocused();
  await page.keyboard.press("Enter");

  // then: the word is in italics, and the toolbar keeps the focus
  await expect(scene(page, only).locator("em")).toHaveText("She");
  await expect(toolbar(page).getByRole("button", { name: "Italic" })).toHaveAttribute("aria-pressed", "true");

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the text has the focus again, the word still selected
  await expect(scene(page, only)).toBeFocused();
  await expect(toolbar(page)).toBeVisible();
});

test("the toolbar sets the selection in the book's quotation marks, and goes when nothing is selected", async ({
  page,
}) => {
  // given: the first word of a scene's text selected
  const only = await aWordSelected(page, "Quoted");

  // when: the quotation marks are chosen in the toolbar
  await toolbar(page).getByRole("button", { name: "Quotation marks" }).click();

  // then: the word stands in quotation marks
  await expect(scene(page, only)).toContainText("“She” never returned.");

  // when: the selection is given up
  await page.keyboard.press("End");

  // then: the toolbar is gone
  await expect(toolbar(page)).toHaveCount(0);
});
