import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { aBookInTheWritingView, cataloguedOpening, flow, heading, label, openTheWritingView, quotationMarks, saveState, scene, shown, statusNote, theBookIsIn } from "./support/writing";

const writingSettings = (page: Page) =>
  page.getByRole("dialog", { name: "Writing settings" });

const checkedForSpelling = (page: Page) =>
  flow(page)
    .locator(".scene .ProseMirror, .heading")
    .evaluateAll((all) => all.map((one) => (one as HTMLElement).spellcheck));

async function quoting(page: Page, named: string, language: string | null) {
  await aNewProject(page, named);
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await theBookIsIn(page, language);
  await openTheWritingView(page);
  await scene(page, only).click();

  return only;
}

const LONG_PARAGRAPH =
  "Am Morgen lag Nebel über dem Fluss, und die Fährleute warteten schweigend am Ufer, bis die Glocke vom Kirchturm herüberklang und der erste Kahn sich langsam vom Steg löste, beladen mit Körben voller Äpfel und Tuchballen für den Markt.";

const howTheTextSits = (page: Page, id: string) =>
  scene(page, id)
    .locator("p")
    .first()
    .evaluate((paragraph) => {
      const style = getComputedStyle(paragraph);
      const range = document.createRange();
      range.selectNodeContents(paragraph);
      const firstLine = range.getClientRects()[0];
      const inside =
        paragraph.getBoundingClientRect().right - parseFloat(style.paddingRight);

      return {
        align: style.textAlign,
        hyphens: style.hyphens,
        wrap: style.textWrapStyle,
        flush: Math.abs(inside - firstLine.right) < 1,
      };
    });

test("scenes are checked for spelling by default, headings never", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "Spelling", { "Part One": 2 });
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
  await aBookInTheWritingView(page, "SpellingOff", { "Part One": 2 });
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
  await page.reload();
  await expect(flow(page).locator(".scene .ProseMirror")).toHaveCount(2);
  expect(
    await checkedForSpelling(page),
    "switched back on, spelling stays on after a reload too",
  ).toEqual([false, true, true]);
});

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

test("a typed \" begins what is said and another ends it, drawn in the book's marks", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingDe", "de");

  await page.keyboard.type('"Komm her," sagte sie. ("Bitte.")');

  await expect
    .poll(() => shown(page, only))
    .toBe("»Komm her,« sagte sie. (»Bitte.«)");
  await expect(quotationMarks(page, only)).toHaveText(["»", "«", "»", "«"]);
});

test("the opening mark is there as soon as \" is typed, the closing one once it is typed again", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingBegun", "de");

  await page.keyboard.type('Sie sagte: "');

  await expect(scene(page, only)).toHaveText("Sie sagte: »");
  await page.keyboard.type("Ja.");
  await expect(scene(page, only)).toHaveText("Sie sagte: »Ja.");
  await page.keyboard.type('"');
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

test("a mark is a letter the cursor steps over like any other", async ({ page }) => {
  const only = await quoting(page, "QuotingStepped", "de");
  await page.keyboard.type('"Komm her"');

  await page.keyboard.press("ArrowLeft");
  await page.keyboard.type(",");
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowRight");
  await page.keyboard.type("K");

  await expect.poll(() => shown(page, only)).toBe("»KKomm her,«");
});

test("a style chosen while writing is not taken back by undoing what was typed", async ({
  page,
}) => {
  const only = await quoting(page, "QuotingUndone", "de");
  await page.keyboard.type('"Eins" ');
  await page.waitForTimeout(700);
  await page.keyboard.type("zwei");
  await expect.poll(() => shown(page, only)).toBe("»Eins« zwei");
  await page.getByRole("button", { name: "Project details" }).click();
  const details = page.getByRole("dialog", { name: "Project details" });
  await details.getByLabel("Quotation marks").selectOption({ label: "„…“" });
  await details.getByRole("button", { name: "Save" }).click();
  await expect.poll(() => shown(page, only)).toBe("„Eins“ zwei");
  await scene(page, only).click();

  await page.keyboard.press("ControlOrMeta+Z");

  await expect.poll(() => shown(page, only)).toBe("„Eins“ ");
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

  await expect(quotationMarks(page, only)).toHaveText(["»", "«"]);
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

  await expect(
    quotationMarks(page, only),
    "pasted, the marks stay quotation marks a later style can change",
  ).toHaveCount(4);
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
  await expect(quotationMarks(page, only)).toHaveText(["»", "«"]);
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
  await expect(quotationMarks(page, only)).toHaveCount(0);
});

test("the text sits ragged right by default, its paragraphs ending without a lone word", async ({
  page,
}) => {
  const only = await quoting(page, "RaggedRight", "de");

  await page.keyboard.insertText(LONG_PARAGRAPH);

  await expect.poll(() => howTheTextSits(page, only)).toEqual({
    align: "start",
    hyphens: "manual",
    wrap: "pretty",
    flush: false,
  });
});

test("justified text in the writing settings sets lines flush and hyphenated, across a reload", async ({
  page,
}) => {
  const only = await quoting(page, "Justified", "de");
  await page.keyboard.insertText(LONG_PARAGRAPH);

  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await writingSettings(page).getByLabel("Justified text").check();
  await page.keyboard.press("Escape");

  const justified = {
    align: "justify",
    hyphens: "auto",
    wrap: "pretty",
    flush: true,
  };
  await expect.poll(() => howTheTextSits(page, only)).toEqual(justified);
  await page.reload();
  await expect(scene(page, only)).toContainText("Am Morgen");
  await expect.poll(() => howTheTextSits(page, only)).toEqual(justified);

  await statusNote(page)
    .getByRole("button", { name: "Writing settings" })
    .click();
  await expect(writingSettings(page).getByLabel("Justified text")).toBeChecked();
  await writingSettings(page).getByLabel("Justified text").uncheck();
  await page.keyboard.press("Escape");

  await expect
    .poll(() => howTheTextSits(page, only))
    .toMatchObject({ align: "start", flush: false });
  await page.reload();
  await expect(scene(page, only)).toContainText("Am Morgen");
  await expect
    .poll(() => howTheTextSits(page, only), "switched back, the text stays ragged after a reload")
    .toMatchObject({ align: "start", flush: false });
});

test("what is said and set in italics opens a scene's catalogued name with its marks and without tags", async ({
  page,
}) => {
  const only = await quoting(page, "PlainOpening", "de");

  await page.keyboard.type('"Was soll ich tun?" fragte sie ');
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type("leise");

  await expect
    .poll(() => cataloguedOpening(page, only))
    .toBe("»Was soll ich tun?« fragte sie leise");
});
