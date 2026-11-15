import { test, expect, type Locator, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject } from "./support/shell";
import { openTheWritingView, scene, wordCount } from "./support/writing";

const A_PIXEL = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

const figureBar = (page: Page) => page.getByRole("toolbar", { name: "Image" });

/// Opens a new book with one scene in the writing view, and puts the cursor into the scene.
async function aSceneToWriteIn(page: Page, named: string): Promise<string> {
  await aNewProject(page, named);
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();

  return only;
}

/// Pastes into the scene as the browser would, with the given files or HTML on the clipboard.
async function pasted(into: Locator, given: { image?: Buffer; html?: string }) {
  await into.evaluate(
    (editor, { image, html }) => {
      const clipboard = new DataTransfer();
      if (image) {
        clipboard.items.add(
          new File([Uint8Array.from(image)], "pasted.png", { type: "image/png" }),
        );
      }
      if (html) {
        clipboard.setData("text/html", html);
        clipboard.setData("text/plain", html.replace(/<[^>]*>/g, ""));
      }
      editor.dispatchEvent(
        new ClipboardEvent("paste", { clipboardData: clipboard, bubbles: true, cancelable: true }),
      );
    },
    { image: given.image ? [...given.image] : undefined, html: given.html },
  );
}

test("a pasted image is uploaded into the project and shown in the scene, its caption not counted as words", async ({
  page,
}) => {
  // given: a scene with a sentence and the cursor on a new line
  const only = await aSceneToWriteIn(page, "Pasting");
  await page.keyboard.type("She never returned.");
  await page.keyboard.press("Enter");

  // when: an image is pasted
  await pasted(scene(page, only), { image: A_PIXEL });

  // then: the scene shows it at full width, loaded from the project's assets
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveCount(1);
  await expect(figure).toHaveAttribute("data-size", "full");
  await expect(figure.locator("img")).toHaveAttribute(
    "src",
    /^\/api\/assets\/[A-Za-z0-9_]+\/content\?width=1536$/,
  );
  await expect(figure).not.toHaveClass(/missing/);

  // when: a caption is written below it
  await figure.locator("figcaption").click();
  await page.keyboard.type("The harbour at dawn");

  // then: the caption is there, and only the sentence counts
  await expect(figure.locator("figcaption")).toHaveText("The harbour at dawn");
  await expect(wordCount(page)).toHaveText("3 words in “Part One”", { timeout: 10_000 });
});

test("an image chosen from the + on an empty line goes where the line was", async ({ page }) => {
  // given: a scene with the cursor on an empty line between two paragraphs
  const only = await aSceneToWriteIn(page, "Choosing");
  await page.keyboard.type("Before.");
  await page.keyboard.press("Enter");
  await page.keyboard.type("After.");
  await page.keyboard.press("Home");
  await page.keyboard.press("Enter");
  await page.keyboard.press("ArrowUp");

  // when: the + is opened and Image… chosen, and a file picked
  await page.getByRole("button", { name: "Insert" }).click();
  const chooser = page.waitForEvent("filechooser");
  await page.getByRole("menuitem", { name: "Image…" }).click();
  await (await chooser).setFiles({ name: "chosen.png", mimeType: "image/png", buffer: A_PIXEL });

  // then: the image takes the place of the empty line
  const blocks = scene(page, only).locator(":scope > *");
  await expect(blocks).toHaveCount(3);
  await expect(blocks.nth(0)).toHaveText("Before.");
  await expect(blocks.nth(1)).toHaveClass(/figure/);
  await expect(blocks.nth(2)).toHaveText("After.");
});

test("the + on an empty line is reached with Alt+F10 and left with Escape, not with Tab", async ({
  page,
}) => {
  // given: the cursor on an empty line of a scene
  const only = await aSceneToWriteIn(page, "Keyboard");
  await expect(page.getByRole("button", { name: "Insert" })).toBeVisible();

  // when: Alt+F10 is pressed
  await page.keyboard.press("Alt+F10");

  // then: the + has the focus
  await expect(page.getByRole("button", { name: "Insert" })).toBeFocused();

  // when: it is opened with Enter, and Escape pressed twice
  await page.keyboard.press("Enter");
  await expect(page.getByRole("menuitem", { name: "Image…" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("menuitem", { name: "Image…" })).toHaveCount(0);
  await page.keyboard.press("Escape");

  // then: the text has the focus again
  await expect(scene(page, only)).toBeFocused();

  // and: the + stays out of the Tab order
  await expect(page.getByRole("button", { name: "Insert" })).toHaveAttribute("tabindex", "-1");
});

test("a chosen image is resized and removed from its toolbar", async ({ page }) => {
  // given: a scene holding a pasted image
  const only = await aSceneToWriteIn(page, "Sizing");
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure.locator("img")).toHaveAttribute("src", /width=1536/);

  // when: the image is clicked
  await figure.locator("img").click();

  // then: its toolbar shows, full width pressed
  await expect(figureBar(page)).toBeVisible();
  await expect(figureBar(page).getByRole("button", { name: "Full width" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // when: small is chosen
  await figureBar(page).getByRole("button", { name: "Small" }).click();

  // then: the image is small, loaded at a smaller width, and stays chosen
  await expect(figure).toHaveAttribute("data-size", "small");
  await expect(figure.locator("img")).toHaveAttribute("src", /width=768$/);
  await expect(figureBar(page).getByRole("button", { name: "Small" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // when: it is removed
  await figureBar(page).getByRole("button", { name: "Remove image" }).click();

  // then: the scene holds no image any more
  await expect(figure).toHaveCount(0);
  await expect(figureBar(page)).toHaveCount(0);
});

test("images pasted from elsewhere are left out, their text kept", async ({ page }) => {
  // given: a scene to write in
  const only = await aSceneToWriteIn(page, "Foreign");

  // when: HTML with an image from another site is pasted
  await pasted(scene(page, only), {
    html: '<p>Seen from the cliffs</p><img src="https://elsewhere.example/tracker.png">',
  });

  // then: the text is there, and nothing is loaded from elsewhere
  await expect(scene(page, only)).toContainText("Seen from the cliffs");
  await expect(scene(page, only).locator("img")).toHaveCount(0);
});

test("a chosen image's empty caption shows a placeholder, gone once written", async ({ page }) => {
  // given: a scene holding a pasted image
  const only = await aSceneToWriteIn(page, "Placeholder");
  await pasted(scene(page, only), { image: A_PIXEL });
  const caption = scene(page, only).locator("figure.figure figcaption");
  await expect(caption).toHaveAttribute("data-placeholder", "Caption");

  // when: the caret goes into the caption
  await caption.click();

  // then: the placeholder shows
  await expect(scene(page, only).locator("figure.figure")).toHaveClass(/captioning/);
  await expect(caption).toHaveClass(/empty/);

  // when: the caption is written
  await page.keyboard.type("A map");

  // then: it is no longer empty
  await expect(caption).not.toHaveClass(/empty/);
});

test("Enter in a caption goes to the line after the image, even as the scene's last block", async ({
  page,
}) => {
  // given: a scene ending in a pasted image, the caret in its empty caption
  const only = await aSceneToWriteIn(page, "LeavingCaption");
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await figure.locator("figcaption").click();

  // when: Enter is pressed and a line written
  await page.keyboard.press("Enter");
  await page.keyboard.type("After the map.");

  // then: the image is still there, and the line follows it in the same scene
  await expect(figure).toHaveCount(1);
  await expect(figure.locator("figcaption")).toHaveText("");
  await expect(scene(page, only).locator(":scope > *").last()).toHaveText("After the map.");
});

test("Backspace at the start of a caption chooses the image instead of tearing it apart", async ({
  page,
}) => {
  // given: a pasted image with a caption, the caret at the caption's start
  const only = await aSceneToWriteIn(page, "BackCaption");
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await figure.locator("figcaption").click();
  await page.keyboard.type("A map");
  await page.keyboard.press("Home");

  // when: Backspace is pressed
  await page.keyboard.press("Backspace");

  // then: the image is chosen, whole, with its caption
  await expect(page.getByRole("toolbar", { name: "Image" })).toBeVisible();
  await expect(figure.locator("figcaption")).toHaveText("A map");
});
