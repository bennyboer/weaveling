import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject, API, openTheOutline, sceneCatalogued } from "./support/shell";
import { flow, heading, label, openTheWritingView, projectOf, scene, sceneTab, sceneTray, shown, statusNote, titled, trayTitle } from "./support/writing";

async function aTitledPair(page: Page) {
  await aNewProject(page, "Tray");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await titled(page, first, "The arrival");
  await titled(page, second, "The loom");
  await openTheWritingView(page);

  return [first, second];
}

async function anIdea(page: Page, title: string) {
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: projectOf(page), title },
  });
  expect(captured.status()).toBe(201);

  return (await captured.json()).id as string;
}

async function anIdeaLinkedTo(page: Page, scene: string, title: string) {
  const id = await anIdea(page, title);
  const linked = await page.request.post(`${API}/scenes/${scene}/ideas`, {
    data: { idea: id },
  });
  expect(linked.ok()).toBe(true);

  return id as string;
}

const drawnFrom = (page: Page) =>
  sceneTray(page).getByRole("list", { name: "Drawn from" });

const linkDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Link ideas" });

test("the tray is shut at first and opens from its tab on the scene the cursor is in", async ({
  page,
}) => {
  const [, second] = await aTitledPair(page);
  await expect(sceneTray(page)).toBeHidden();

  await scene(page, second).click();
  await sceneTab(page).click();

  await expect(sceneTab(page)).toHaveAttribute("aria-expanded", "true");
  await expect(trayTitle(page)).toHaveValue("The loom");
});

test("the tray follows the cursor, and keeps the last scene when the cursor leaves the scenes", async ({
  page,
}) => {
  const [first, second] = await aTitledPair(page);
  await sceneTab(page).click();

  await scene(page, first).click();
  await expect(trayTitle(page)).toHaveValue("The arrival");
  await scene(page, second).click();
  await expect(trayTitle(page)).toHaveValue("The loom");
  await flow(page).getByRole("heading", { name: "Part One" }).click();

  await expect(
    trayTitle(page),
    "reaching for the tray must not empty it",
  ).toHaveValue("The loom");
});

test("an untitled scene is named in the tray by its opening words", async ({
  page,
}) => {
  await aNewProject(page, "TrayUntitled");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await sceneTab(page).click();
  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "An empty scene",
  );

  await scene(page, only).click();
  await page.keyboard.type("The loom stood silent.");

  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
  await expect(trayTitle(page)).toHaveValue("");
});

test("the tray stays open or shut across a reload, and Escape in it shuts it", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await page.reload();
  await expect(sceneTray(page)).toBeVisible();

  await sceneTray(page).getByRole("button", { name: "Close the tray" }).focus();
  await page.keyboard.press("Escape");
  await expect(sceneTray(page)).toBeHidden();
  await page.reload();
  await expect(flow(page)).toBeVisible();
  await expect(sceneTray(page)).toBeHidden();
});

test("Escape while writing leaves the tray open", async ({ page }) => {
  const [first] = await aTitledPair(page);
  await sceneTab(page).click();
  await scene(page, first).click();

  await page.keyboard.press("Escape");

  await expect(
    sceneTray(page),
    "Escape in the prose belongs to the margin, not the tray",
  ).toBeVisible();
});

test("on a wide window the open tray sits beside the column and ends above the status bar", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  const noteBefore = (await statusNote(page).boundingBox())!;

  await sceneTab(page).click();

  const tray = (await sceneTray(page).locator(".inside").boundingBox())!;
  const written = (await scene(page, first).boundingBox())!;
  const note = (await statusNote(page).boundingBox())!;
  expect(written.x + written.width).toBeLessThanOrEqual(tray.x);
  expect(
    tray.y + tray.height,
    "the tray covers nothing of the status bar",
  ).toBeLessThanOrEqual(note.y + 1);
  expect(note.x, "the status bar keeps its place").toBe(noteBefore.x);
  expect(
    await statusNote(page).evaluate((shown) => {
      const at = shown.getBoundingClientRect();

      return shown.contains(
        document.elementFromPoint(at.x + at.width / 2, at.y + at.height / 2),
      );
    }),
    "nothing covers the status note",
  ).toBe(true);
});

test("on a phone the tray slides over the text and is toggled from the status bar", async ({
  page,
}) => {
  await page.setViewportSize({ width: 400, height: 800 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  const tray = (await sceneTray(page).boundingBox())!;
  expect(tray.x).toBeGreaterThanOrEqual(0);
  expect(tray.x + tray.width).toBeLessThanOrEqual(400);
  await expect(trayTitle(page)).toHaveValue("The arrival");

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(sceneTray(page)).toBeHidden();
});

test("a scene renamed in the tray is named so in the margin, on the server and in the outline", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("Begun.");
  await sceneTab(page).click();

  await trayTitle(page).fill("The homecoming");
  await trayTitle(page).press("Enter");

  await expect(label(page, first)).toHaveText("The homecoming");
  await expect(
    scene(page, first),
    "Enter puts the cursor back where it was writing",
  ).toBeFocused();
  await page.keyboard.type(" Still.");
  await expect(scene(page, first)).toHaveText("Begun. Still.");
  await sceneCatalogued(page, first, "The homecoming");
  await openTheOutline(page);
  await expect(page.locator(".leaf .name").first()).toHaveText(
    "The homecoming",
  );
});

test("Escape in the tray's title drops what was typed and keeps the tray open", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await trayTitle(page).fill("Not this");
  await trayTitle(page).press("Escape");

  await expect(trayTitle(page)).toHaveValue("The arrival");
  await expect(sceneTray(page)).toBeVisible();
  await expect(scene(page, first)).toBeFocused();
  await expect(label(page, first)).toHaveText("The arrival");
});

test("a title cleared in the tray leaves the opening words to name the scene", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await sceneTab(page).click();

  await trayTitle(page).fill("");
  await trayTitle(page).press("Enter");

  await expect(label(page, first)).toHaveText("The loom stood silent.");
  await expect(trayTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
});

test("a scene renamed in the margin is named so in the open tray", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await flow(page)
    .locator(`.scene-row:has([data-scene="${first}"])`)
    .getByRole("button", { name: "Scene actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Renamed");
  await flow(page).getByRole("textbox", { name: "Scene title" }).press("Enter");

  await expect(trayTitle(page)).toHaveValue("Renamed");
});

test("the tray lists the ideas a scene draws on, each leading to its idea", async ({
  page,
}) => {
  await aNewProject(page, "TrayIdeas");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();

  await sceneTab(page).click();

  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
    "A war down the valley",
  ]);
  await drawnFrom(page)
    .getByRole("link", { name: "The loom remembers" })
    .click();
  await expect(page).toHaveURL(new RegExp(`/ideas/[^/]*${loom}$`));
});

test("an idea unlinked in the tray no longer counts for the scene", async ({
  page,
}) => {
  await aNewProject(page, "TrayUnlink");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  const war = await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await drawnFrom(page)
    .getByRole("button", { name: "Unlink The loom remembers" })
    .click();

  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "A war down the valley",
  ]);
  await expect
    .poll(
      async () =>
        (await (await page.request.get(`${API}/scenes/${first}`)).json()).ideas,
    )
    .toEqual([war]);
  expect(loom).not.toBe(war);
});

test("the tray's ideas follow the cursor to the next scene, and a scene without any says so", async ({
  page,
}) => {
  await aNewProject(page, "TrayIdeasFollow");
  const [first, second] = (await aBookWithScenes(page, { "Part One": 2 }))[
    "Part One"
  ];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
  ]);

  await scene(page, second).click();

  await expect(drawnFrom(page)).toHaveCount(0);
  await expect(sceneTray(page)).toContainText("No ideas linked yet.");
});

test("ideas linked from the tray are listed in the order they were picked, and kept", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinking");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  const winter = await anIdea(page, "Winter comes early");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();
  await linkDialog(page).getByLabel("Winter comes early").check();
  await linkDialog(page).getByLabel("The loom remembers").check();
  await linkDialog(page).getByRole("button", { name: "Link 2" }).click();

  await expect(linkDialog(page)).toBeHidden();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "Winter comes early",
    "The loom remembers",
  ]);
  await expect
    .poll(
      async () =>
        (await (await page.request.get(`${API}/scenes/${first}`)).json()).ideas,
    )
    .toEqual([winter, loom]);
});

test("the tray's dialog offers only ideas the scene does not draw on yet", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinkingOffers");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();

  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();

  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("A war down the valley");
});

test("Escape closes the tray's dialog and leaves the tray open", async ({
  page,
}) => {
  await aNewProject(page, "TrayLinkingEscape");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();
  await expect(linkDialog(page)).toBeVisible();

  await page.keyboard.press("Escape");

  await expect(linkDialog(page)).toBeHidden();
  await expect(sceneTray(page)).toBeVisible();
  await expect(sceneTray(page)).toContainText("No ideas linked yet.");
});

test("the tray's dialog finds ideas by name", async ({ page }) => {
  await aNewProject(page, "TrayLinkingFinds");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await sceneTab(page).click();
  await sceneTray(page).getByRole("button", { name: "Link an idea" }).click();

  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("loom");

  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("The loom remembers");
  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("nothing like it");
  await expect(linkDialog(page)).toContainText("No idea matches.");
});
