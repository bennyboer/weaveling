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

test("the toolbar sets the selection in the work's quotation marks, and goes when nothing is selected", async ({
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

test("a link is set from the toolbar, opens with Ctrl and a click, and is taken off again", async ({
  page,
  context,
}) => {
  // given: the first word of a scene's text selected
  const only = await aWordSelected(page, "Linked");
  await context.route("https://example.com/**", (route) =>
    route.fulfill({ contentType: "text/html", body: "<title>Elsewhere</title>" }),
  );

  // when: a link is chosen in the toolbar, and an address without a scheme typed
  await toolbar(page).getByRole("button", { name: "Link" }).click();
  await expect(toolbar(page).getByLabel("Address of the link")).toBeFocused();
  await page.keyboard.type("example.com");
  await page.keyboard.press("Enter");

  // then: the word links to the web address, and the text has the focus again
  await expect(scene(page, only).locator("a")).toHaveText("She");
  await expect(scene(page, only).locator("a")).toHaveAttribute("href", "https://example.com");
  await expect(scene(page, only)).toBeFocused();
  await expect(toolbar(page).getByRole("button", { name: "Link" })).toHaveAttribute("aria-pressed", "true");

  // when: the link is clicked with Ctrl held
  const opened = context.waitForEvent("page");
  await scene(page, only).locator("a").click({ modifiers: ["ControlOrMeta"] });

  // then: the address opens in a tab of its own
  expect((await opened).url()).toBe("https://example.com/");

  // when: the link is taken off from the toolbar
  await scene(page, only).click();
  await page.keyboard.press("Home");
  for (let step = 0; step < "She".length; step++) {
    await page.keyboard.press("Shift+ArrowRight");
  }
  await toolbar(page).getByRole("button", { name: "Link" }).click();
  await toolbar(page).getByRole("button", { name: "Remove link" }).click();

  // then: the word is plain text again
  await expect(scene(page, only).locator("a")).toHaveCount(0);
  await expect(scene(page, only)).toContainText("She never returned.");
});

test("an address that is no web or email address is refused as a link", async ({ page }) => {
  // given: the first word of a scene's text selected, and the field for a link open
  const only = await aWordSelected(page, "Refused");
  await toolbar(page).getByRole("button", { name: "Link" }).click();

  // when: a script is given as the address
  await page.keyboard.type("javascript:alert(1)");
  await page.keyboard.press("Enter");

  // then: it is refused, saying why, and nothing links
  await expect(toolbar(page).getByRole("alert")).toHaveText(
    "Only web addresses and email addresses can be links.",
  );
  await expect(scene(page, only).locator("a")).toHaveCount(0);
});

test("Ctrl+K asks for a link, and an email address links to writing to it", async ({ page }) => {
  // given: the first word of a scene's text selected
  const only = await aWordSelected(page, "Mailed");

  // when: Ctrl+K is pressed and an email address given
  await page.keyboard.press("ControlOrMeta+K");
  await expect(toolbar(page).getByLabel("Address of the link")).toBeFocused();
  await page.keyboard.type("ada@example.com");
  await page.keyboard.press("Enter");

  // then: the word links to writing to that address
  await expect(scene(page, only).locator("a")).toHaveAttribute("href", "mailto:ada@example.com");
});

test("what is pasted keeps its web links, and a link that is no web address stays plain text", async ({
  page,
}) => {
  // given: a scene with the cursor at the end of its text
  const only = await aWordSelected(page, "Pasted");
  await page.keyboard.press("End");

  // when: text with a web link and a script link is pasted
  await scene(page, only).evaluate((prose) => {
    const pasted = new DataTransfer();
    pasted.setData(
      "text/html",
      ' <a href="https://example.com/good">good</a> and <a href="javascript:alert(1)">bad</a>',
    );
    pasted.setData("text/plain", " good and bad");
    prose.dispatchEvent(
      new ClipboardEvent("paste", { clipboardData: pasted, bubbles: true, cancelable: true }),
    );
  });

  // then: the web link is kept, and the script is plain text
  await expect(scene(page, only)).toContainText("good and bad");
  await expect(scene(page, only).locator("a")).toHaveCount(1);
  await expect(scene(page, only).locator("a")).toHaveAttribute("href", "https://example.com/good");
});
