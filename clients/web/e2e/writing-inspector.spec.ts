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
  inspectedSceneTitle,
  inspector,
  inspectorTab,
  label,
  openTheWritingView,
  projectOf,
  projectsOutline,
  scene,
  shown,
  statusNote,
  titled,
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
  // given: two named scenes open in the writing view
  const [, second] = await aTitledPair(page);
  // then: the tray is shut
  await expect(inspector(page)).toBeHidden();

  // when: the cursor is put in the second and the tray's tab clicked
  await scene(page, second).click();
  await inspectorTab(page).click();

  // then: the tray opens on that scene
  await expect(inspectorTab(page)).toHaveAttribute("aria-expanded", "true");
  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
});

test("the tray follows the cursor from scene to scene, and into a heading", async ({
  page,
}) => {
  // given: two named scenes open in the writing view, the tray open
  const [first, second] = await aTitledPair(page);
  await inspectorTab(page).click();

  // when: the cursor goes from scene to scene
  await scene(page, first).click();
  // then: the tray follows it
  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");
  await scene(page, second).click();
  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
  // when: the cursor goes into a heading
  await flow(page).getByRole("heading", { name: "Part One" }).click();

  // then: the tray shows that section
  await expect(inspectedSection(page)).toContainText("Part One");
  // and: its tab says so
  await expect(
    page.getByRole("button", { name: "This section", exact: true }),
    "the tab says what the tray is showing",
  ).toBeVisible();
});

test("a heading still being typed leaves the tray as it was", async ({
  page,
}) => {
  // given: the tray open on a named scene
  const [, second] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, second).click();

  // when: a new heading is begun after it
  await page.keyboard.press("ControlOrMeta+End");
  for (let nth = 0; nth < 3; nth++) await page.keyboard.press("Enter");

  // then: the heading takes the cursor
  await expect(
    flow(page).locator('[data-placeholder="A new section"]'),
  ).toBeFocused();
  // and: the tray still shows the scene
  await expect(
    inspectedSceneTitle(page),
    "the new heading is no section yet, least of all the one before it",
  ).toHaveValue("The loom");
});

test("reaching for the tray keeps what it shows", async ({ page }) => {
  // given: the tray open on a named scene
  const [, second] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, second).click();

  // when: the tray is clicked into
  await inspectedSceneTitle(page).click();

  // then: it still shows the scene
  await expect(inspectedSceneTitle(page)).toHaveValue("The loom");
});

test("a heading's pinned ideas are shown in the tray, each leading to its idea", async ({
  page,
}) => {
  // given: a section with two ideas pinned to it
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
  // and: the book open in the writing view
  await openTheWritingView(page);

  // when: the cursor goes into its heading and the tray is opened
  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.getByRole("button", { name: "This section", exact: true }).click();

  // then: the tray shows its pinned ideas in order
  await expect(pinnedIdeas(page).getByRole("link")).toHaveText([
    "The weaver's debt",
    "A war in the valley",
  ]);
  // and: each leads to its idea
  await expect(
    pinnedIdeas(page).getByRole("link", { name: "The weaver's debt" }),
  ).toHaveAttribute("href", new RegExp(`/ideas/[^/]*${debt}$`));
});

test("a heading with nothing pinned says so", async ({ page }) => {
  // given: a book with nothing pinned, the tray open
  await aTitledPair(page);
  await inspectorTab(page).click();

  // when: the cursor goes into a heading
  await flow(page).getByRole("heading", { name: "Part One" }).click();

  // then: the tray says nothing is pinned there
  await expect(inspectedSection(page)).toContainText("No ideas pinned here.");
});

const pinDialog = (page: Page) =>
  page.getByRole("dialog", { name: "Pin ideas" });

async function pinnedOnServer(page: Page, title: string) {
  const outline = await projectsOutline(page);
  const section = outline.sections.find(
    (held: { title: string }) => held.title === title,
  );

  return section.attachments
    .filter((attached: { kind: string }) => attached.kind === "idea")
    .map((attached: { id: string }) => attached.id);
}

async function aBookWithPins(page: Page) {
  await aNewProject(page, "Pinning");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  const already = await anIdeaPinnedTo(
    page,
    outline,
    ids["Part One"],
    "Already here",
  );
  await anIdeaPinnedTo(page, outline, ids["Part Two"], "Pinned elsewhere");
  const winter = await anIdea(page, "Winter comes early");
  const loom = await anIdea(page, "The loom remembers");
  await openTheWritingView(page);
  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.getByRole("button", { name: "This section", exact: true }).click();

  return { already, winter, loom };
}

test("ideas are pinned to a section from the inspector, after what it holds", async ({
  page,
}) => {
  // given: the tray open on a section with an idea pinned, another pinned elsewhere and two pinned nowhere
  const { already, winter, loom } = await aBookWithPins(page);

  // when: pinning ideas is begun from the tray
  await inspectedSection(page)
    .getByRole("button", { name: "Pin an idea" })
    .click();
  // then: only the ideas pinned nowhere are offered
  await expect(
    pinDialog(page)
      .getByRole("list", { name: "Ideas to pin" })
      .getByRole("checkbox"),
    "an idea pinned anywhere in the outline is not offered again",
  ).toHaveCount(2);
  // when: both are pinned
  await pinDialog(page).getByLabel("Winter comes early").check();
  await pinDialog(page).getByLabel("The loom remembers").check();
  await pinDialog(page).getByRole("button", { name: "Pin 2" }).click();

  // then: the tray shows them after what the section held
  await expect(pinDialog(page)).toBeHidden();
  await expect(pinnedIdeas(page).getByRole("link")).toHaveText([
    "Already here",
    "Winter comes early",
    "The loom remembers",
  ]);
  // and: they are saved so
  await expect
    .poll(() => pinnedOnServer(page, "Part One"))
    .toEqual([already, winter, loom]);
});

test("an idea unpinned in the inspector leaves the outline and is offered again", async ({
  page,
}) => {
  // given: the tray open on a section with an idea pinned
  await aBookWithPins(page);

  // when: the idea is unpinned
  await pinnedIdeas(page)
    .getByRole("button", { name: "Unpin Already here" })
    .click();

  // then: the tray says nothing is pinned there
  await expect(inspectedSection(page)).toContainText("No ideas pinned here.");
  // and: the section is saved without it
  await expect.poll(() => pinnedOnServer(page, "Part One")).toEqual([]);
  // when: pinning ideas is begun
  await inspectedSection(page)
    .getByRole("button", { name: "Pin an idea" })
    .click();
  // then: the idea is offered again
  await expect(pinDialog(page).getByLabel("Already here")).toBeVisible();
});

test("an untitled scene is named in the tray by its opening words", async ({
  page,
}) => {
  // given: an empty scene open in the writing view
  await aNewProject(page, "TrayUntitled");
  const [only] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await openTheWritingView(page);
  // and: the tray open on it
  await scene(page, only).click();
  await inspectorTab(page).click();
  // then: the tray invites to name it
  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "An empty scene",
  );

  // when: it is written in
  await scene(page, only).click();
  await page.keyboard.type("The loom stood silent.");

  // then: the tray names it by its opening words
  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
  // and: it is still untitled
  await expect(inspectedSceneTitle(page)).toHaveValue("");
});

test("the tray stays open or shut across a reload, and Escape in it shuts it", async ({
  page,
}) => {
  // given: the tray open on a scene
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: the page is reloaded
  await page.reload();
  // then: the tray is still open
  await expect(inspector(page)).toBeVisible();

  // when: Escape is pressed in the tray
  await inspector(page).getByRole("button", { name: "Close the tray" }).focus();
  await page.keyboard.press("Escape");
  // then: it shuts
  await expect(inspector(page)).toBeHidden();
  // when: the page is reloaded
  await page.reload();
  // then: the tray is still shut
  await expect(flow(page)).toBeVisible();
  await expect(inspector(page)).toBeHidden();
});

test("Escape while writing leaves the tray open", async ({ page }) => {
  // given: the tray open on a scene, the cursor in it
  const [first] = await aTitledPair(page);
  await inspectorTab(page).click();
  await scene(page, first).click();

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the tray stays open
  await expect(
    inspector(page),
    "Escape in the prose belongs to the margin, not the tray",
  ).toBeVisible();
});

test("on a wide window the open tray sits beside the column and ends above the status bar", async ({
  page,
}) => {
  // given: a scene open on a wide window, the cursor in it
  await page.setViewportSize({ width: 1440, height: 900 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  const noteBefore = (await statusNote(page).boundingBox())!;

  // when: the tray is opened
  await inspectorTab(page).click();

  // then: it sits beside the scene
  const tray = (await inspector(page).locator(".inside").boundingBox())!;
  const written = (await scene(page, first).boundingBox())!;
  const note = (await statusNote(page).boundingBox())!;
  expect(written.x + written.width).toBeLessThanOrEqual(tray.x);
  // and: ends above the status bar
  expect(
    tray.y + tray.height,
    "the tray covers nothing of the status bar",
  ).toBeLessThanOrEqual(note.y + 1);
  // and: the status bar keeps its place, uncovered
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
  // given: a scene open on a phone, the cursor in it
  await page.setViewportSize({ width: 400, height: 800 });
  const [first] = await aTitledPair(page);
  await scene(page, first).click();

  // when: the tray is opened from the status bar
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  // then: it lies within the window, on that scene
  const tray = (await inspector(page).boundingBox())!;
  expect(tray.x).toBeGreaterThanOrEqual(0);
  expect(tray.x + tray.width).toBeLessThanOrEqual(400);
  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");

  // when: it is toggled again
  await page.getByRole("button", { name: "This scene", exact: true }).click();
  // then: it is shut
  await expect(inspector(page)).toBeHidden();
});

test("a scene renamed in the tray is named so in the margin, on the server and in the outline", async ({
  page,
}) => {
  // given: the tray open on a scene being written in
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("Begun.");
  await inspectorTab(page).click();

  // when: the scene is renamed in the tray
  await inspectedSceneTitle(page).fill("The homecoming");
  await inspectedSceneTitle(page).press("Enter");

  // then: the margin names it so
  await expect(label(page, first)).toHaveText("The homecoming");
  // and: the cursor is back where it was writing
  await expect(
    scene(page, first),
    "Enter puts the cursor back where it was writing",
  ).toBeFocused();
  await page.keyboard.type(" Still.");
  await expect(scene(page, first)).toHaveText("Begun. Still.");
  // and: the outline names it so
  await sceneCatalogued(page, first, "The homecoming");
  await openTheOutline(page);
  await expect(page.locator(".leaf .name").first()).toHaveText(
    "The homecoming",
  );
});

test("Escape in the tray's title drops what was typed and keeps the tray open", async ({
  page,
}) => {
  // given: the tray open on a named scene
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: another title is typed and Escape pressed
  await inspectedSceneTitle(page).fill("Not this");
  await inspectedSceneTitle(page).press("Escape");

  // then: the old title stands
  await expect(inspectedSceneTitle(page)).toHaveValue("The arrival");
  // and: the tray stays open
  await expect(inspector(page)).toBeVisible();
  // and: the cursor is back in the scene
  await expect(scene(page, first)).toBeFocused();
  await expect(label(page, first)).toHaveText("The arrival");
});

test("a title cleared in the tray leaves the opening words to name the scene", async ({
  page,
}) => {
  // given: the tray open on a named scene with some words
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await page.keyboard.type("The loom stood silent.");
  await inspectorTab(page).click();

  // when: its title is cleared in the tray
  await inspectedSceneTitle(page).fill("");
  await inspectedSceneTitle(page).press("Enter");

  // then: the margin names it by its opening words
  await expect(label(page, first)).toHaveText("The loom stood silent.");
  // and: so does the tray
  await expect(inspectedSceneTitle(page)).toHaveAttribute(
    "placeholder",
    "The loom stood silent.",
  );
});

test("a scene renamed in the margin is named so in the open tray", async ({
  page,
}) => {
  // given: the tray open on a named scene
  const [first] = await aTitledPair(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: the scene is renamed in the margin
  await flow(page)
    .locator(`.scene-row:has([data-scene="${first}"])`)
    .getByRole("button", { name: "Scene actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("Renamed");
  await flow(page).getByRole("textbox", { name: "Scene title" }).press("Enter");

  // then: the tray names it so
  await expect(inspectedSceneTitle(page)).toHaveValue("Renamed");
});

test("the tray lists the ideas a scene draws on, each leading to its idea", async ({
  page,
}) => {
  // given: a scene drawing on two ideas, open in the writing view
  await aNewProject(page, "TrayIdeas");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();

  // when: the tray is opened
  await inspectorTab(page).click();

  // then: it lists the ideas
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "The loom remembers",
    "A war down the valley",
  ]);
  // when: one is clicked
  await drawnFrom(page)
    .getByRole("link", { name: "The loom remembers" })
    .click();
  // then: it leads to that idea
  await expect(page).toHaveURL(new RegExp(`/ideas/[^/]*${loom}$`));
});

test("an idea unlinked in the tray no longer counts for the scene", async ({
  page,
}) => {
  // given: the tray open on a scene drawing on two ideas
  await aNewProject(page, "TrayUnlink");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdeaLinkedTo(page, first, "The loom remembers");
  const war = await anIdeaLinkedTo(page, first, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: one is unlinked
  await drawnFrom(page)
    .getByRole("button", { name: "Unlink The loom remembers" })
    .click();

  // then: the tray lists only the other
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "A war down the valley",
  ]);
  // and: the scene is saved drawing only on the other
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
  // given: the tray open on a scene drawing on an idea, with a scene after it drawing on none
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

  // when: the cursor goes into the next scene
  await scene(page, second).click();

  // then: the tray says it draws on no idea
  await expect(drawnFrom(page)).toHaveCount(0);
  await expect(inspector(page)).toContainText("No ideas linked yet.");
});

test("ideas linked from the tray are listed in the order they were picked, and kept", async ({
  page,
}) => {
  // given: the tray open on a scene, with three ideas in the project
  await aNewProject(page, "TrayLinking");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const loom = await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  const winter = await anIdea(page, "Winter comes early");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: two ideas are picked and linked
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();
  await linkDialog(page).getByLabel("Winter comes early").check();
  await linkDialog(page).getByLabel("The loom remembers").check();
  await linkDialog(page).getByRole("button", { name: "Link 2" }).click();

  // then: the tray lists them in the order they were picked
  await expect(linkDialog(page)).toBeHidden();
  await expect(drawnFrom(page).getByRole("link")).toHaveText([
    "Winter comes early",
    "The loom remembers",
  ]);
  // and: the scene is saved so
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
  // given: the tray open on a scene drawing on one of two ideas
  await aNewProject(page, "TrayLinkingOffers");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: linking ideas is begun
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();

  // then: only the other idea is offered
  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("A war down the valley");
});

test("Escape closes the tray's dialog and leaves the tray open", async ({
  page,
}) => {
  // given: linking ideas begun from the tray
  await aNewProject(page, "TrayLinkingEscape");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();
  await expect(linkDialog(page)).toBeVisible();

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: linking closes
  await expect(linkDialog(page)).toBeHidden();
  // and: the tray stays open as it was
  await expect(inspector(page)).toBeVisible();
  await expect(inspector(page)).toContainText("No ideas linked yet.");
});

test("the tray's dialog finds ideas by name", async ({ page }) => {
  // given: linking ideas begun from the tray, with two ideas in the project
  await aNewProject(page, "TrayLinkingFinds");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdea(page, "The loom remembers");
  await anIdea(page, "A war down the valley");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();

  // when: a name is searched for
  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("loom");

  // then: only the idea so called is offered
  await expect(
    linkDialog(page).getByRole("list", { name: "Ideas to link" }),
  ).toHaveText("The loom remembers");
  // when: nothing matches
  await linkDialog(page)
    .getByRole("searchbox", { name: "Find an idea" })
    .fill("nothing like it");
  // then: it says so
  await expect(linkDialog(page)).toContainText("No idea matches.");
});

test("with every idea already linked, the tray's dialog says so", async ({
  page,
}) => {
  // given: the tray open on a scene drawing on every idea
  await aNewProject(page, "TrayEveryIdeaLinked");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  await anIdeaLinkedTo(page, first, "The loom remembers");
  await openTheWritingView(page);
  await scene(page, first).click();
  await inspectorTab(page).click();

  // when: linking ideas is begun
  await inspector(page).getByRole("button", { name: "Link an idea" }).click();

  // then: it says every idea is linked already
  await expect(linkDialog(page)).toContainText(
    "Every idea in this project is already linked.",
  );
});

test("a section removed meanwhile is told as removed in the tray", async ({
  page,
}) => {
  // given: the tray open on a section
  await aNewProject(page, "TraySectionRemoved");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheWritingView(page);
  await flow(page).getByRole("heading", { name: "Part One" }).click();
  await page.getByRole("button", { name: "This section", exact: true }).click();
  await expect(inspectedSection(page)).toContainText("Part One");

  // when: the section is removed meanwhile
  const removed = await page.request.delete(
    `${API}/outlines/${outline}/sections/${ids["Part One"]}`,
  );
  expect(removed.ok()).toBe(true);

  // then: the tray says it was removed
  await expect(inspectedSection(page)).toContainText("This section was removed.");
});

