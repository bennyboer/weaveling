import { test, expect, type Locator, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { API, aNewProject } from "./support/shell";
import { withAccounts } from "./support/signed-in";
import { openTheWritingView, projectOf, scene, wordCount } from "./support/writing";

const A_PIXEL = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

const figureBar = (page: Page) => page.getByRole("toolbar", { name: "Image" });
const choosing = (page: Page) => page.getByRole("dialog", { name: "Insert an image" });

/// Opens the dialog for choosing an image from the "+" on the scene's empty line.
async function theImageDialog(page: Page) {
  await page.getByRole("button", { name: "Insert" }).click();
  await page.getByRole("menuitem", { name: "Image…" }).click();
  await expect(choosing(page)).toBeVisible();
}

/// Uploads an image straight into a project, as if it had been put into one of its scenes before.
async function anImageIn(page: Page, project: string): Promise<string> {
  const uploaded = await page.request.post(`${API}/assets?project=${project}`, {
    data: A_PIXEL,
    headers: { "content-type": "application/octet-stream" },
  });
  expect(uploaded.status(), await uploaded.text()).toBe(201);

  return (await uploaded.json()).id;
}

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

  // when: the + is opened, Image… chosen, and a new file uploaded
  await theImageDialog(page);
  const chooser = page.waitForEvent("filechooser");
  await choosing(page).getByRole("button", { name: "Upload new…" }).click();
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

/// Picks files for an image from the "+" on the scene's empty line, uploading them anew.
async function picked(page: Page, ...files: { name: string; buffer: Buffer }[]) {
  await theImageDialog(page);
  const chooser = page.waitForEvent("filechooser");
  await choosing(page).getByRole("button", { name: "Upload new…" }).click();
  await (await chooser).setFiles(files.map((file) => ({ ...file, mimeType: "image/png" })));
}

test("an image larger than an upload may be is refused in its place, without being sent", async ({
  page,
}) => {
  // given: a scene to write in, every upload counted
  const only = await aSceneToWriteIn(page, "TooLarge");
  const sent: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/api/assets")) {
      sent.push(request.url());
    }
  });

  // when: a file of 21 MB is chosen
  await picked(page, { name: "huge.png", buffer: Buffer.alloc(21 * 1024 * 1024, 7) });

  // then: the text says in its place that it is too large, and nothing was sent
  const refused = scene(page, only).locator(".uploading.refused");
  await expect(refused).toContainText("huge.png");
  await expect(refused).toContainText("An image can be at most 20\u00a0MB large.");
  await expect(refused).not.toContainText("Uploading");
  expect(sent).toHaveLength(0);

  // when: the refusal is dismissed
  await refused.getByRole("button", { name: "Dismiss" }).click();

  // then: it is gone, and the scene holds no image
  await expect(refused).toHaveCount(0);
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
});

test("an image being uploaded shows in its place how far it got, then becomes the image", async ({
  page,
}) => {
  // given: a scene to write in, and uploads held back until let go
  const only = await aSceneToWriteIn(page, "Progress");
  let letGo = () => {};
  const held = new Promise<void>((resolve) => (letGo = resolve));
  await page.route(/\/api\/assets\?project=/, async (route) => {
    await held;
    await route.continue();
  });

  // when: an image is chosen
  await picked(page, { name: "slow.png", buffer: A_PIXEL });

  // then: its place shows it uploading, with a progress bar
  const uploading = scene(page, only).locator(".uploading");
  await expect(uploading).toContainText("Uploading slow.png…");
  await expect(uploading.getByRole("progressbar", { name: "Uploading slow.png…" })).toBeVisible();

  // when: the upload goes through
  letGo();

  // then: the image takes its place
  await expect(scene(page, only).locator("figure.figure img")).toHaveAttribute("src", /width=1536/);
  await expect(uploading).toHaveCount(0);
});

test("an image the server refuses says why in its place, and the others still go in", async ({
  page,
}) => {
  // given: a scene to write in
  const only = await aSceneToWriteIn(page, "Refused");

  // when: a file that is no image and an image are chosen together
  await picked(
    page,
    { name: "notes.png", buffer: Buffer.from("not an image at all") },
    { name: "fine.png", buffer: A_PIXEL },
  );

  // then: the image goes in, and the other file says it is no image
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(1);
  const refused = scene(page, only).locator(".uploading.refused");
  await expect(refused).toContainText("notes.png");
  await expect(refused).toContainText("This file is not an image Weaveling can use.");
});

test("the project's images are offered the newest first, and one chosen goes in as it is", async ({
  page,
}) => {
  // given: a scene to write in, and two images the project already holds
  const only = await aSceneToWriteIn(page, "Offered");
  const project = projectOf(page)!;
  const older = await anImageIn(page, project);
  const newer = await anImageIn(page, project);
  const copied: string[] = [];
  page.on("request", (request) => {
    if (request.url().includes("/copies")) {
      copied.push(request.url());
    }
  });

  // when: the dialog for an image is opened
  await theImageDialog(page);

  // then: uploading a new one comes first, then the project's images, the newest first
  const tiles = choosing(page).locator(".image-grid button");
  await expect(tiles).toHaveCount(3);
  await expect(tiles.nth(0)).toHaveAccessibleName("Upload new…");
  await expect(tiles.nth(1).locator("img")).toHaveAttribute("src", new RegExp(newer));
  await expect(tiles.nth(2).locator("img")).toHaveAttribute("src", new RegExp(older));
  await expect(tiles.nth(1)).toHaveAccessibleName(/^Image uploaded on /);

  // when: the older one is chosen
  await tiles.nth(2).click();

  // then: the dialog is gone, and the scene holds that very image, not a copy
  await expect(choosing(page)).toHaveCount(0);
  await expect(scene(page, only).locator("figure.figure")).toHaveAttribute("data-asset", older);
  expect(copied).toHaveLength(0);
});

test("the dialog for an image is left with Cancel or Escape, and nothing goes in", async ({
  page,
}) => {
  // given: a scene to write in
  const only = await aSceneToWriteIn(page, "Cancelled");

  // when: the dialog is opened and cancelled
  await theImageDialog(page);
  await expect(choosing(page).getByText("No images yet.")).toBeVisible();
  await choosing(page).getByRole("button", { name: "Cancel" }).click();

  // then: it is gone, and the text has the focus again
  await expect(choosing(page)).toHaveCount(0);
  await expect(scene(page, only)).toBeFocused();

  // when: it is opened again and left with Escape
  await theImageDialog(page);
  await page.keyboard.press("Escape");

  // then: it is gone, and the scene holds no image
  await expect(choosing(page)).toHaveCount(0);
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
});

test("without accounts there is no filter of one's own images", async ({ page }) => {
  test.skip(withAccounts, "the filter is offered to a signed-in account");

  // given: a scene to write in
  await aSceneToWriteIn(page, "NoFilter");

  // when: the dialog for an image is opened
  await theImageDialog(page);

  // then: only the project's images are offered, with no choice of whose
  await expect(choosing(page).getByRole("group", { name: "Whose images" })).toHaveCount(0);
});

test("an image of one's own from another project is copied into this one when chosen", async ({
  page,
}) => {
  test.skip(!withAccounts, "one's own images are only offered to a signed-in account");

  // given: an image in another project of hers, and a scene to write in in a new one
  const other = { name: await aNewProject(page, "Elsewhere"), id: projectOf(page)! };
  const image = await anImageIn(page, other.id);
  const only = await aSceneToWriteIn(page, "Copying");

  // when: the dialog is opened, her own images shown, and that one chosen
  await theImageDialog(page);
  const whose = choosing(page).getByRole("group", { name: "Whose images" });
  await expect(whose.getByRole("button", { name: "This project" })).toHaveAttribute("aria-pressed", "true");
  await whose.getByRole("button", { name: "My images" }).click();
  const tile = choosing(page).locator(".image-offered").filter({ has: page.locator(`img[src*="${image}"]`) });
  await expect(tile).toHaveAccessibleName(new RegExp(`^Image in ${other.name}, uploaded on `));
  await tile.click();

  // then: the scene holds a copy of it, the project's own
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  const copy = await figure.getAttribute("data-asset");
  expect(copy).not.toBe(image);
  await expect(figure).not.toHaveClass(/missing/);
});
