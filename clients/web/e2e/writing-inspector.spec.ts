import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import {
  aNewProject,
  API,
  openTheOutline,
  sceneCatalogued,
} from "./support/shell";
import {
  flow,
  heading,
  label,
  openTheWritingView,
  projectOf,
  scene,
  inspectorTab,
  inspector,
  shown,
  statusNote,
  titled,
  inspectedSceneTitle,
} from "./support/writing";

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

async function anIdeaPinnedTo(
  page: Page,
  outline: string,
  section: string,
  title: string,
  after: string | null = null,
) {
  const id = await anIdea(page, title);
  const pinned = await page.request.post(
    `${API}/outlines/${outline}/attachments`,
    {
      data: {
        attachment: { kind: "idea", id },
        section,
        after: after ? { kind: "idea", id: after } : null,
      },
    },
  );
  expect(pinned.ok(), await pinned.text()).toBe(true);

  return id;
}

const inspectedSection = (page: Page) =>
  page.getByRole("complementary", { name: "This section" });

const pinnedIdeas = (page: Page) =>
  inspectedSection(page).getByRole("list", { name: "Pinned ideas" });

const drawnFrom = (page: Page) =>
  inspector(page).getByRole("list", { name: "Drawn from" });

const linkDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Link ideas" });

test("the tray is shut at first and opens from its tab on the scene the cursor is in", async ({
  page,
}) => {
  const [, second] = await aTitledPair(page);
  await expect(inspector(page)).toBeHidden();

  await scene(page, second).click();
  await inspectorTab(page).click();

  await expect(inspectorTab(page)).toHaveAttribute("aria-expanded", "true");
  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
});

test("the tray follows the cursor from scene to scene, and into a heading", async ({
  page,
}) => {
  const [first, second] = await aTitledPair(page);
  await inspectorTab(page).click();

  await scene(page, first).click();
  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");
  await scene(page, second).click();
  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
  await flow(page).getByRole("heading", { name: "Part One" }).click();

  await expect(inspectedSection(page)).toContainText("Part One");
  await expect(
    page.getByRole("button", { name: "This section", exact: true }),
    "the tab says what the tray is showing",
  ).toBeVisible();
});

test("a heading still being typed leaves the tray as it was", async ({
  page,
}) => {
  const [, second] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, second).click();

  await page.keyboard.press("ControlOrMeta+End");
  for (let nth = 0; nth < 3; nth++) await page.keyboard.press("Enter");

  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  await expect(
    inspectedSceneTitle(page),
    "the new heading is no section yet, least of all the one before it",
  ).toHaveValue("The loom");
});

test("reaching for the tray keeps what it shows", async ({ page }) => {
  const [, second] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, second).click();

  await inspectedSceneTitle(page).click();

  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
});

test("a heading's pinned ideas are shown in the tray, each leading to its idea", async ({
  page,
}) => {
  await aNewProject(page, "TrayPinned");
  const { outline, ids } = await aBookShaped(page, [{ title: "Part One" }]);
  const debt = await anIdeaPinnedTo(
    page,
    outline,
    ids["Part One"],
    "The weaver's debt",
  );
  await anIdeaPinnedTo(
    page,
    outline,
    ids["Part One"],
    "A war in the valley",
    debt,
  );
  await openTheWritingView(page);

  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.getByRole("button", { name: "This section", exact: true }).click();

  await expect(pinnedIdeas(page).getByRole("link")).toHaveText([
    "The weaver's debt",
    "A war in the valley",
  ]);
  await expect(
    pinnedIdeas(page).getByRole("link", { name: "The weaver's debt" }),
  ).toHaveAttribute("href", new RegExp(`/ideas/[^/]*${debt}$`));
});

test("a heading with nothing pinned says where ideas are pinned", async ({
  page,
}) => {
  await aTitledPair(page);
  await inspectorTab(page).click();

  await flow(page).getByRole("heading", { name: "Part One" }).click();

  await expect(inspectedSection(page)).toContainText(
    "No ideas pinned here. Pin them in the outline.",
  );
});

test("an untitled scene is named in the tray by its opening words", async ({
  page,
}) => {
  await aNewProject(page, "TrayUntitled");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, only).click();
  await inspectorTab(page).click();
  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "An empty scene",
  );

  await scene(page, only).click();
  await page.keyboard.type("The loom stood silent.");

  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
  await expect(inspectedSceneTitle(page)).toHaveValue("");
});

test("the tray stays open or shut across a reload, and Escape in it shuts it", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  await page.reload();
  await expect(inspector(page)).toBeVisible();

  await inspector(page).getByRole("button", { name: "Close the tray" }).focus();
  await page.keyboard.press("Escape");
  await expect(inspector(page)).toBeHidden();
  await page.reload();
  await expect(flow(page)).toBeVisible();
  await expect(inspector(page)).toBeHidden();
});

test("Escape while writing leaves the tray open", async ({ page }) => {
  const [first] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, first).click();

  await page.keyboard.press("Escape");

  await expect(
    inspector(page),
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

  await inspectorTab(page).click();

  const tray = (await inspector(page).locator(".inside").boundingBox())!;
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
  const tray = (await inspector(page).boundingBox())!;
  expect(tray.x).toBeGreaterThanOrEqual(0);
  expect(tray.x + tray.width).toBeLessThanOrEqual(400);
  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");

  await page.getByRole("button", { name: "This scene", exact: true }).click();
  await expect(inspector(page)).toBeHidden();
});

test("a scene renamed in the tray is named so in the margin, on the server and in the outline", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("Begun.");
  await inspectorTab(page).click();

  await inspectedSceneTitle(page).fill("The homecoming");
  await inspectedSceneTitle(page).press("Enter");

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
  await inspectorTab(page).click();

  await inspectedSceneTitle(page).fill("Not this");
  await inspectedSceneTitle(page).press("Escape");

  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");
  await expect(inspector(page)).toBeVisible();
  await expect(scene(page, first)).toBeFocused();
  await expect(label(page, first)).toHaveText("The arrival");
});

test("a title cleared in the tray leaves the opening words to name the scene", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await inspectorTab(page).click();

  await inspectedSceneTitle(page).fill("");
  await inspectedSceneTitle(page).press("Enter");

  await expect(label(page, first)).toHaveText("The loom stood silent.");
  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
});

test("a scene renamed in the margin is named so in the open tray", async ({
  page,
}) => {
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  await flow(page)
    .locator(`.scene-row:has([data-scene="${first}"])`)
    .getByRole("button", { name: "Scene actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Renamed");
  await flow(page).getByRole("textbox", { name: "Scene title" }).press("Enter");

  await expect(inspectedSceneTitle(page)).toHaveValue("Renamed");
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

  await inspectorTab(page).click();

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
  await inspectorTab(page).click();

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
  await inspectorTab(page).click();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
  ]);

  await scene(page, second).click();

  await expect(drawnFrom(page)).toHaveCount(0);
  await expect(inspector(page)).toContainText("No ideas linked yet.");
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
  await inspectorTab(page).click();

  await inspector(page).getByRole("button", { name: "Link an idea" }).click();
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
  await inspectorTab(page).click();

  await inspector(page).getByRole("button", { name: "Link an idea" }).click();

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
  await inspectorTab(page).click();
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();
  await expect(linkDialog(page)).toBeVisible();

  await page.keyboard.press("Escape");

  await expect(linkDialog(page)).toBeHidden();
  await expect(inspector(page)).toBeVisible();
  await expect(inspector(page)).toContainText("No ideas linked yet.");
});

test("the tray's dialog finds ideas by name", async ({ page }) => {
  await aNewProject(page, "TrayLinkingFinds");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();

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
