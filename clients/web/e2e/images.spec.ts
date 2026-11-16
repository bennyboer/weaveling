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

  // when: the + is opened, Image… chosen, a new file uploaded, and then chosen
  await uploadedAndChosen(page, { name: "chosen.png", buffer: A_PIXEL });

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

/// Picks files in the dialog for an image, opened from the "+" on the scene's empty line, to upload them anew.
async function picked(page: Page, ...files: { name: string; buffer: Buffer }[]) {
  await theImageDialog(page);
  const chooser = page.waitForEvent("filechooser");
  await choosing(page).getByRole("button", { name: "Upload new…" }).click();
  await (await chooser).setFiles(files.map((file) => ({ ...file, mimeType: "image/png" })));
}

/// Uploads a file from the dialog for an image, and chooses it once it is uploaded.
async function uploadedAndChosen(page: Page, file: { name: string; buffer: Buffer }) {
  await picked(page, file);
  await expect(choosing(page).locator(".image-sending")).toHaveCount(0);
  await choosing(page).locator(".image-offered").first().click();
}

const sendingIn = (page: Page) => choosing(page).locator(".image-sending");

test("an image uploaded from the dialog shows there how far it got, then is offered to be chosen", async ({
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

  // when: an image is uploaded from the dialog
  await picked(page, { name: "slow.png", buffer: A_PIXEL });

  // then: the dialog shows it uploading first in its grid, with a progress bar
  const tiles = choosing(page).locator(".image-grid > li");
  await expect(tiles.nth(1)).toContainText("slow.png");
  await expect(
    tiles.nth(1).getByRole("progressbar", { name: "Uploading slow.png…" }),
  ).toBeVisible();

  // when: the upload goes through
  letGo();

  // then: it is offered first in the grid, the dialog still open, the scene still without an image
  await expect(sendingIn(page)).toHaveCount(0);
  await expect(tiles.nth(1).locator(".image-offered")).toHaveAccessibleName(/^Image uploaded on /);
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);

  // when: it is chosen
  await tiles.nth(1).locator(".image-offered").click();

  // then: the dialog is gone, and the scene holds it
  await expect(choosing(page)).toHaveCount(0);
  await expect(scene(page, only).locator("figure.figure img")).toHaveAttribute("src", /width=1536/);
});

test("an image larger than an upload may be is refused in the dialog, without being sent", async ({
  page,
}) => {
  // given: a scene to write in, every upload counted
  await aSceneToWriteIn(page, "TooLarge");
  const sent: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/api/assets")) {
      sent.push(request.url());
    }
  });

  // when: a file of 21 MB is uploaded from the dialog
  await picked(page, { name: "huge.png", buffer: Buffer.alloc(21 * 1024 * 1024, 7) });

  // then: its tile says it is too large, and nothing was sent
  const refused = sendingIn(page).and(choosing(page).locator(".refused"));
  await expect(refused).toContainText("huge.png");
  await expect(refused).toContainText("An image can be at most 20\u00a0MB large.");
  await expect(refused.getByRole("progressbar")).toHaveCount(0);
  expect(sent).toHaveLength(0);

  // when: the refusal is dismissed
  await refused.getByRole("button", { name: "Dismiss" }).click();

  // then: it is gone
  await expect(sendingIn(page)).toHaveCount(0);
});

test("a file the server refuses says why in the dialog, and the others are still uploaded", async ({
  page,
}) => {
  // given: a scene to write in
  const only = await aSceneToWriteIn(page, "Refused");

  // when: a file that is no image and an image are uploaded together
  await picked(
    page,
    { name: "notes.png", buffer: Buffer.from("not an image at all") },
    { name: "fine.png", buffer: A_PIXEL },
  );

  // then: the image is offered, and the other file says it is no image
  await expect(choosing(page).locator(".image-offered")).toHaveCount(1);
  await expect(sendingIn(page)).toHaveCount(1);
  await expect(sendingIn(page)).toContainText("notes.png");
  await expect(sendingIn(page)).toContainText("This file is not an image Weaveling can use.");
  // and: nothing went into the text
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
});

test("leaving the dialog stops what it still uploads", async ({ page }) => {
  // given: a scene to write in, and uploads held back until let go
  await aSceneToWriteIn(page, "Stopped");
  const project = projectOf(page)!;
  let letGo = () => {};
  const held = new Promise<void>((resolve) => (letGo = resolve));
  await page.route(/\/api\/assets\?project=/, async (route) => {
    await held;
    await route.continue().catch(() => {});
  });

  // when: an image is uploaded from the dialog, and the dialog left before it went through
  await picked(page, { name: "left.png", buffer: A_PIXEL });
  await expect(sendingIn(page).getByRole("progressbar")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(choosing(page)).toHaveCount(0);
  letGo();

  // then: the project holds no image
  await expect
    .poll(async () => (await (await page.request.get(`${API}/assets/projects/${project}`)).json()).assets.length)
    .toBe(0);
});

test("an image pasted shows in its place how far it got, then becomes the image", async ({
  page,
}) => {
  // given: a scene to write in, and uploads held back until let go
  const only = await aSceneToWriteIn(page, "PastedProgress");
  let letGo = () => {};
  const held = new Promise<void>((resolve) => (letGo = resolve));
  await page.route(/\/api\/assets\?project=/, async (route) => {
    await held;
    await route.continue();
  });

  // when: an image is pasted
  await pasted(scene(page, only), { image: A_PIXEL });

  // then: its place shows it uploading, with a progress bar
  const uploading = scene(page, only).locator(".uploading");
  await expect(uploading).toContainText("Uploading pasted.png…");
  await expect(uploading.getByRole("progressbar", { name: "Uploading pasted.png…" })).toBeVisible();

  // when: the upload goes through
  letGo();

  // then: the image takes its place
  await expect(scene(page, only).locator("figure.figure img")).toHaveAttribute("src", /width=1536/);
  await expect(uploading).toHaveCount(0);
});

test("an image pasted that is larger than an upload may be is refused in its place, without being sent", async ({
  page,
}) => {
  // given: a scene to write in, uploads of at most a few bytes, and every upload counted
  const only = await aSceneToWriteIn(page, "PastedTooLarge");
  await page.route(/\/api\/assets\/limits$/, (route) => route.fulfill({ json: { each_upload: 8 } }));
  const sent: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/api/assets")) {
      sent.push(request.url());
    }
  });

  // when: an image larger than that is pasted
  await pasted(scene(page, only), { image: A_PIXEL });

  // then: the text says in its place that it is too large, and nothing was sent
  const refused = scene(page, only).locator(".uploading.refused");
  await expect(refused).toContainText("pasted.png");
  await expect(refused).toContainText("An image can be at most 8\u00a0B large.");
  await expect(refused).not.toContainText("Uploading");
  expect(sent).toHaveLength(0);

  // when: the refusal is dismissed
  await refused.getByRole("button", { name: "Dismiss" }).click();

  // then: it is gone, and the scene holds no image
  await expect(refused).toHaveCount(0);
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
});

test("a pasted file the server refuses says why in its place", async ({ page }) => {
  // given: a scene to write in
  const only = await aSceneToWriteIn(page, "PastedRefused");

  // when: a file that is no image is pasted as one
  await pasted(scene(page, only), { image: Buffer.from("not an image at all") });

  // then: it says in its place that it is no image, and the scene holds no image
  const refused = scene(page, only).locator(".uploading.refused");
  await expect(refused).toContainText("pasted.png");
  await expect(refused).toContainText("This file is not an image Weaveling can use.");
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
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
  const tiles = choosing(page).locator(".image-grid > li > button:not(.tile-remove)");
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
  let letGo = () => {};
  const held = new Promise<void>((resolve) => (letGo = resolve));
  await page.route(/\/copies\?project=/, async (route) => {
    await held;
    await route.continue();
  });
  await tile.click();

  // then: while it is copied, its tile says so, the dialog still open
  await expect(tile).toContainText("Adding the image…");
  await expect(tile).toHaveAttribute("aria-busy", "true");
  await expect(choosing(page)).toBeVisible();

  // when: the copy is made
  letGo();

  // then: the dialog is gone, and the scene holds a copy of it, the project's own
  await expect(choosing(page)).toHaveCount(0);
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  const copy = await figure.getAttribute("data-asset");
  expect(copy).not.toBe(image);
  await expect(figure).not.toHaveClass(/missing/);
});

test("an image in use is found from the storage, and its scene opens with the image chosen", async ({
  page,
}) => {
  test.skip(!withAccounts, "the storage belongs to a signed-in account");

  // given: an image put into a scene
  const only = await aSceneToWriteIn(page, "Found");
  await uploadedAndChosen(page, { name: "map.png", buffer: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  const image = (await figure.getAttribute("data-asset"))!;
  // and: the server told that the scene uses it
  await expect
    .poll(
      async () => ((await (await page.request.get(`${API}/assets/${image}`)).json()).uses ?? []).length,
      { timeout: 15_000 },
    )
    .toBe(1);

  // when: her storage is opened, the image's mark pressed, and the scene it is used in followed
  await page.goto("/account/storage");
  const row = page
    .getByRole("main")
    .getByRole("listitem")
    .filter({ has: page.locator(`img[src*="${image}"]`) });
  await row.getByRole("button", { name: "in use" }).click();
  await page.getByRole("dialog").locator(".used-in").getByRole("link").click();

  // then: the writing view opens at that scene, the image chosen
  await expect(page).toHaveURL(/\/write/);
  await expect(scene(page, only).locator("figure.figure")).toHaveAttribute("data-asset", image);
  await expect(figureBar(page)).toBeVisible();
});

test("an image of one's own that cannot be copied says why on its tile, the dialog staying open", async ({
  page,
}) => {
  test.skip(!withAccounts, "one's own images are only offered to a signed-in account");

  // given: an image in another project of hers, a scene to write in in a new one, and copies refused
  const other = { name: await aNewProject(page, "Kept"), id: projectOf(page)! };
  const image = await anImageIn(page, other.id);
  const only = await aSceneToWriteIn(page, "NoCopy");
  await page.route(/\/copies\?project=/, (route) =>
    route.fulfill({
      status: 403,
      contentType: "application/problem+json",
      json: { type: "not-an-author", title: "Forbidden", status: 403, detail: "" },
    }),
  );

  // when: her own images are shown, and that one chosen
  await theImageDialog(page);
  await choosing(page).getByRole("group", { name: "Whose images" }).getByRole("button", { name: "My images" }).click();
  const tile = choosing(page).locator(".image-offered").filter({ has: page.locator(`img[src*="${image}"]`) });
  await tile.click();

  // then: its tile says why it could not be copied, and the dialog stays open
  await expect(tile.getByRole("alert")).toBeVisible();
  await expect(tile).toHaveAttribute("aria-busy", "false");
  await expect(choosing(page)).toBeVisible();
  // and: nothing went into the text
  await expect(scene(page, only).locator("figure.figure")).toHaveCount(0);
});

test("an image of the project is deleted from its tile, after asking", async ({ page }) => {
  // given: a scene to write in, and an image the project holds
  await aSceneToWriteIn(page, "Deleting");
  const project = projectOf(page)!;
  const image = await anImageIn(page, project);

  // when: the dialog is opened, and the image's tile pointed at
  await theImageDialog(page);
  const tile = choosing(page).locator(".offered").filter({ has: page.locator(`img[src*="${image}"]`) });
  await tile.hover();

  // then: a button to delete it shows
  const remove = tile.getByRole("button", { name: "Delete this image" });
  await expect(remove).toBeVisible();

  // when: it is pressed, and the image kept after all
  await remove.click();
  const asking = tile.getByRole("group", { name: "Delete this image?" });
  await asking.getByRole("button", { name: "Keep" }).click();

  // then: the question is gone, and the image still offered
  await expect(asking).toHaveCount(0);
  await expect(tile).toHaveCount(1);

  // when: it is asked again, and the deleting confirmed
  await tile.hover();
  await remove.click();
  await asking.getByRole("button", { name: "Delete" }).click();

  // then: the tile is gone, the dialog still open, and the project holds no image
  await expect(tile).toHaveCount(0);
  await expect(choosing(page)).toBeVisible();
  await expect
    .poll(async () => (await (await page.request.get(`${API}/assets/projects/${project}`)).json()).assets.length)
    .toBe(0);
});

test("an image in use is not deleted, and the dialog names where, leading there", async ({ page }) => {
  // given: an image pasted into a scene, and a line after it to write on
  const only = await aSceneToWriteIn(page, "StillUsed");
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  const image = (await figure.getAttribute("data-asset"))!;
  await figure.locator("figcaption").click();
  await page.keyboard.press("Enter");
  // and: the server told that the scene uses it
  await expect
    .poll(
      async () => ((await (await page.request.get(`${API}/assets/${image}`)).json()).uses ?? []).length,
      { timeout: 15_000 },
    )
    .toBe(1);

  // when: the dialog is opened from the empty line, and deleting the image confirmed
  await theImageDialog(page);
  const tile = choosing(page).locator(".offered").filter({ has: page.locator(`img[src*="${image}"]`) });
  await tile.hover();
  await tile.getByRole("button", { name: "Delete this image" }).click();
  await tile.getByRole("group", { name: "Delete this image?" }).getByRole("button", { name: "Delete" }).click();

  // then: the dialog says where it is still used, and to remove it there first
  const told = choosing(page).getByRole("alert").filter({ hasText: "This image is still used in:" });
  await expect(told).toContainText("Remove it there first, then delete it.");
  const there = told.getByRole("link");
  await expect(there).toHaveCount(1);
  // and: the image is still offered
  await expect(tile).toHaveCount(1);

  // when: the scene it is used in is followed
  await there.click();

  // then: the dialog is gone, and the image is chosen in its scene
  await expect(choosing(page)).toHaveCount(0);
  await expect(figureBar(page)).toBeVisible();
  await expect(figure).toHaveAttribute("data-asset", image);
});

/// Ends a scene with a pasted image, and goes to the line after it that begins a new scene.
async function onTheLineAfterAnImage(page: Page, named: string): Promise<string> {
  const only = await aSceneToWriteIn(page, named);
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  await figure.locator("figcaption").click();
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await expect(page.locator(".opening")).toBeFocused();

  return (await figure.getAttribute("data-asset"))!;
}

const sceneRows = (page: Page) => page.locator("[data-scene-row]");

test("an image pasted on the line a new scene begins on begins the scene, holding the image", async ({
  page,
}) => {
  // given: the caret on the line after a scene ending in an image
  await onTheLineAfterAnImage(page, "PastedOnTheLine");

  // when: an image is pasted there
  await pasted(page.locator(".opening"), { image: A_PIXEL });

  // then: a new scene is begun after the first, holding the image
  await expect(sceneRows(page)).toHaveCount(2);
  await expect(sceneRows(page).nth(1).locator("figure.figure img")).toHaveAttribute("src", /width=1536/);
});

test("the + on the line a new scene begins on offers an image to begin the scene with", async ({
  page,
}) => {
  // given: the caret on the line after a scene ending in an image
  const image = await onTheLineAfterAnImage(page, "AddingOnTheLine");

  // then: the line shows the +
  const add = page.locator(".opening-line").getByRole("button", { name: "Insert" });
  await expect(add).toBeVisible();

  // when: Image… is chosen from it, and the project's image chosen in the dialog
  await add.click();
  await page.locator(".opening-line").getByRole("menuitem", { name: "Image…" }).click();
  await expect(choosing(page)).toBeVisible();
  await choosing(page).locator(".image-offered").filter({ has: page.locator(`img[src*="${image}"]`) }).click();

  // then: a new scene is begun after the first, holding that image
  await expect(choosing(page)).toHaveCount(0);
  await expect(sceneRows(page)).toHaveCount(2);
  await expect(sceneRows(page).nth(1).locator("figure.figure")).toHaveAttribute("data-asset", image);
});

test("the + on the line a new scene begins on goes once something is typed there", async ({ page }) => {
  // given: the caret on the line after a scene ending in an image, showing the +
  await onTheLineAfterAnImage(page, "TypingOnTheLine");
  await expect(page.locator(".opening-line .adding")).toBeVisible();

  // when: a word is typed there
  await page.keyboard.type("Then");

  // then: the + is gone, and the word begins a new scene
  await expect(page.locator(".opening-line .adding")).toHaveCount(0);
  await expect(sceneRows(page)).toHaveCount(2);
  await expect(sceneRows(page).nth(1)).toContainText("Then");
});

test("a chosen image is copied as a picture, and pasted back it is the same image, not uploaded again", async ({
  page,
  context,
}) => {
  // given: a scene holding a pasted image with a caption, and the clipboard open to the page
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  const only = await aSceneToWriteIn(page, "CopiedOut");
  await pasted(scene(page, only), { image: A_PIXEL });
  const figure = scene(page, only).locator("figure.figure");
  await expect(figure).toHaveAttribute("data-asset", /^asset_/);
  const image = (await figure.getAttribute("data-asset"))!;
  await figure.locator("figcaption").click();
  await page.keyboard.type("A map");

  // when: the image is chosen and copied
  await figure.locator("img").click();
  await expect(figureBar(page)).toBeVisible();
  await page.keyboard.press("Control+c");

  // then: the clipboard holds the picture, the figure with the picture in it, and the caption as text
  await expect
    .poll(() => page.evaluate(async () => (await navigator.clipboard.read())[0]?.types ?? []))
    .toEqual(expect.arrayContaining(["image/png", "text/html", "text/plain"]));
  const told = await page.evaluate(async () => {
    const [item] = await navigator.clipboard.read();
    const picture = await item.getType("image/png");

    return {
      html: await (await item.getType("text/html")).text(),
      text: await (await item.getType("text/plain")).text(),
      picture: picture.size,
    };
  });
  expect(told.html).toContain(`data-asset="${image}"`);
  expect(told.html).toMatch(/<img[^>]*src="data:image\/png;base64,/);
  expect(told.text).toBe("A map");
  expect(told.picture).toBeGreaterThan(0);

  // when: it is pasted on a new line after the image, every upload counted
  const sent: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "POST" && request.url().includes("/api/assets")) {
      sent.push(request.url());
    }
  });
  await figure.locator("figcaption").click();
  await page.keyboard.press("End");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Control+v");

  // then: the scene holds the same image twice, and nothing was uploaded
  await expect(figure).toHaveCount(2);
  await expect(figure.nth(1)).toHaveAttribute("data-asset", image);
  expect(sent).toHaveLength(0);
});
