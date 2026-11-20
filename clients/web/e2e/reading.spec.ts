import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { routingLive } from "./support/live";
import {
  aNewProject,
  API,
  aSceneMade,
  scenesCatalogued,
  views,
} from "./support/shell";
import {
  openTheWritingView,
  projectOf,
  projectsOutline,
  scene,
  titled,
} from "./support/writing";

const book = (page: Page) => page.locator(".reading-view").getByRole("article");

const contents = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

const contentsTab = (page: Page) =>
  page.locator('.drawer-tab[aria-controls="reading-contents"]');

async function openTheReadingView(page: Page) {
  await views(page).getByRole("link", { name: "Read", exact: true }).click();
  await expect(book(page)).toBeVisible();
}

/// A chapter with two scenes, and a part under it with one, each scene with nothing written yet.
async function aBookOfThreeScenes(page: Page) {
  const { outline, ids } = await aBookShaped(page, [
    { title: "Morning" },
    { title: "The market", under: "Morning" },
  ]);
  const placed = { harbour: "", wall: "", stalls: "" };
  for (const [named, section, behind] of [
    ["harbour", "Morning", null],
    ["wall", "Morning", "harbour"],
    ["stalls", "The market", null],
  ] as const) {
    placed[named] = await aSceneMade(page);
    const attached = await page.request.post(
      `${API}/outlines/${outline}/attachments`,
      {
        data: {
          attachment: { kind: "scene", id: placed[named] },
          section: ids[section],
          after: behind ? { kind: "scene", id: placed[behind] } : null,
        },
      },
    );
    expect(attached.ok(), await attached.text()).toBe(true);
  }
  await scenesCatalogued(page, Object.values(placed));

  return placed;
}

/// Waits until the readings have been told every one of these words.
/// A scene first tells its text as it opens, still empty, so counting scenes is not enough.
async function toldToTheReadings(page: Page, words: string[]) {
  await expect
    .poll(
      async () => {
        const live = await page.request.get(
          `${API}/readings/live?project=${projectOf(page)}`,
        );
        const told = live.ok() ? JSON.stringify(await live.json()) : "";

        return words.filter((said) => !told.includes(said));
      },
      {
        message:
          "a live scene tells its text every couple of seconds, and the readings keep it from the message",
        timeout: 20_000,
      },
    )
    .toEqual([]);
}

/// Waits until the readings have heard of the project, so its book is there to be read.
async function readable(page: Page) {
  await expect
    .poll(
      async () =>
        (
          await page.request.get(
            `${API}/readings/live?project=${projectOf(page)}`,
          )
        ).status(),
      { message: "the readings learn of a project from its start" },
    )
    .toBe(200);
}

/// A book of three scenes, written in the writing view: one with words in italics, one plain, and one in bold.
async function aBookWritten(page: Page, named: string) {
  const title = await aNewProject(page, named);
  const placed = await aBookOfThreeScenes(page);
  await openTheWritingView(page);

  await scene(page, placed.harbour).click();
  await page.keyboard.type("The boats came in ");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type("one by one");
  await page.keyboard.press("ControlOrMeta+I");
  await page.keyboard.type(", their lamps swinging.");
  await scene(page, placed.wall).click();
  await page.keyboard.type("Hedy counted them from the wall.");
  await scene(page, placed.stalls).click();
  await page.keyboard.press("ControlOrMeta+B");
  await page.keyboard.type("Everybody bought the fish.");
  await toldToTheReadings(page, [
    "one by one",
    "Hedy counted them from the wall.",
    "Everybody bought the fish.",
  ]);

  return { title, placed };
}

test("the reading view shows the book as it was written, with its sections and scenes", async ({
  page,
}) => {
  // given: a book written in the writing view
  const { title } = await aBookWritten(page, "Reading");

  // when: its reading view is opened
  await openTheReadingView(page);

  // then: the book is shown under its project's name
  await expect(
    book(page).getByRole("heading", { level: 1, name: title }),
  ).toBeVisible();
  // and: its sections are headings at their depth
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Morning" }),
  ).toBeVisible();
  await expect(
    book(page).getByRole("heading", { level: 3, name: "The market" }),
  ).toBeVisible();
  // and: its scenes are shown with their marks
  await expect(book(page)).toContainText(
    "The boats came in one by one, their lamps swinging.",
  );
  await expect(book(page).locator("em")).toHaveText("one by one");
  await expect(book(page).locator("strong")).toHaveText(
    "Everybody bought the fish.",
  );
  // and: a break stands between the two scenes of the chapter, and nowhere else
  await expect(book(page).getByRole("separator")).toHaveCount(1);
  // and: the Read tab is the one shown
  await expect(
    views(page).getByRole("link", { name: "Read", exact: true }),
  ).toHaveClass(/here/);
});

test("the contents name only the sections, and lead to them", async ({
  page,
}) => {
  // given: a written book's reading view in a short window
  await page.setViewportSize({ width: 1200, height: 500 });
  await aBookWritten(page, "ReadingContents");
  await openTheReadingView(page);

  // expect: the contents list the sections and no scene
  await expect(contents(page).locator(".rail-entry")).toHaveText([
    "Morning",
    "The market",
  ]);

  // when: a section is chosen in the contents
  await contents(page).getByRole("button", { name: "The market" }).click();

  // then: its heading is brought into view
  await expect(
    book(page).getByRole("heading", { name: "The market" }),
  ).toBeInViewport();
});

test("the contents are open at first, and stay as they were left apart from the writing view's", async ({
  page,
}) => {
  // given: a project's reading view opened for the first time
  await aNewProject(page, "ReadingDrawer");
  await aBookShaped(page, [{ title: "Morning" }]);
  await readable(page);
  await openTheReadingView(page);

  // expect: its contents are open
  await expect(contents(page)).toBeVisible();

  // when: they are closed and the view is opened again
  await contentsTab(page).click();
  await expect(contents(page)).toBeHidden();
  await page.reload();
  await expect(book(page)).toBeVisible();

  // then: they are still closed
  await expect(contents(page)).toBeHidden();
  await expect(contentsTab(page)).toHaveAttribute("aria-expanded", "false");

  // when: the writing view's contents are opened, and the reading view is opened again
  await openTheWritingView(page);
  await page.locator('.drawer-tab[aria-controls="book-rail"]').click();
  await openTheReadingView(page);

  // then: the reading view's contents are still closed
  await expect(contents(page)).toBeHidden();
});

test("a project whose outline was never opened reads as its name alone", async ({
  page,
}) => {
  // given: a new project, its book never shaped
  const title = await aNewProject(page, "ReadingEmpty");
  await readable(page);

  // when: its reading view is opened
  await openTheReadingView(page);

  // then: the book shows its name and nothing else
  await expect(
    book(page).getByRole("heading", { level: 1, name: title }),
  ).toBeVisible();
  await expect(book(page).getByRole("heading", { level: 2 })).toHaveCount(0);
});

const saying = (words: string) => [
  {
    kind: "paragraph",
    runs: [
      {
        kind: "text",
        text: words,
        marks: { emphasis: false, strong: false, code: false, link: null },
      },
    ],
  },
];

test("a scene's text told while reading is swapped in, and the rest of the book stays as it is", async ({
  page,
}) => {
  // given: a written book's reading view, its other scenes marked as drawn
  const live = await routingLive(page);
  const { placed } = await aBookWritten(page, "ReadingLiveText");
  await openTheReadingView(page);
  const drawn = (scene: string) =>
    book(page).locator(`[data-scene="${scene}"]`);
  for (const scene of [placed.harbour, placed.stalls]) {
    await drawn(scene).evaluate((held) =>
      held.setAttribute("data-drawn", "once"),
    );
  }

  // when: another author's change to one scene is told
  live.tell("readings", {
    version: 1,
    event: {
      kind: "text",
      scene: placed.wall,
      blocks: saying("Hedy counted them twice."),
    },
  });

  // then: that scene reads as told
  await expect(drawn(placed.wall)).toHaveText("Hedy counted them twice.");
  // and: the other scenes were not drawn again
  for (const scene of [placed.harbour, placed.stalls]) {
    await expect(drawn(scene)).toHaveAttribute("data-drawn", "once");
  }
});

test("the book's sections and title told while reading are shown as told", async ({
  page,
}) => {
  // given: a written book's reading view
  const live = await routingLive(page);
  const { placed } = await aBookWritten(page, "ReadingLiveOutline");
  await openTheReadingView(page);

  // when: its sections are told rearranged, the market first and the morning under it, and its title anew
  live.tell("readings", {
    version: 1,
    event: {
      kind: "outline",
      sections: [
        {
          section: "section_market",
          depth: 0,
          title: "The market",
          scenes: [placed.stalls],
        },
        {
          section: "section_morning",
          depth: 1,
          title: "Morning",
          scenes: [placed.wall, placed.harbour],
        },
      ],
    },
  });
  live.tell("readings", {
    version: 2,
    event: { kind: "title", title: "The Loom, Again" },
  });

  // then: the book is laid out as told, under its new title
  await expect(book(page).getByRole("heading", { level: 1 })).toHaveText(
    "The Loom, Again",
  );
  await expect(book(page).getByRole("heading", { level: 2 })).toHaveText([
    "The market",
  ]);
  await expect(book(page).getByRole("heading", { level: 3 })).toHaveText([
    "Morning",
  ]);
  await expect(book(page).locator("[data-scene]")).toHaveText([
    "Everybody bought the fish.",
    "Hedy counted them from the wall.",
    "The boats came in one by one, their lamps swinging.",
  ]);
  // and: the contents follow
  await expect(contents(page).locator(".rail-entry")).toHaveText([
    "The market",
    "Morning",
  ]);
});

test("a scene placed in the book since it was read has the book read again", async ({
  page,
}) => {
  // given: a written book's reading view
  const live = await routingLive(page);
  const { placed } = await aBookWritten(page, "ReadingLivePlaced");
  await openTheReadingView(page);

  // when: its sections are told holding a scene it was never told the text of
  const readAgain = page.waitForRequest((asked) =>
    asked.url().includes("/api/readings/live?"),
  );
  live.tell("readings", {
    version: 1,
    event: {
      kind: "outline",
      sections: [
        {
          section: "section_morning",
          depth: 0,
          title: "Morning",
          scenes: [placed.harbour, "scene_placed_since"],
        },
      ],
    },
  });

  // then: the book is read again
  await readAgain;
});

test("a reader sees the book as its author writes it, the words and the sections", async ({
  page,
}) => {
  // given: a written book, its reading view open in another tab
  const { placed } = await aBookWritten(page, "ReadingFollowed");
  const reader = await page.context().newPage();
  await reader.goto(page.url().replace(/\/write$/, "/read"));
  await expect(
    book(reader).getByText("Hedy counted them from the wall."),
  ).toBeVisible();

  let readAgain = 0;
  reader.on("request", (asked) => {
    if (asked.url().includes("/api/readings/live?")) {
      readAgain += 1;
    }
  });

  // when: the author writes on in a scene
  await scene(page, placed.wall).click();
  await page.keyboard.press("End");
  await page.keyboard.type(" Twice.");

  // then: the reader sees the words as they settle, without reading again
  await expect(
    book(reader).locator(`[data-scene="${placed.wall}"]`),
  ).toHaveText("Hedy counted them from the wall. Twice.", { timeout: 15_000 });

  expect(
    readAgain,
    "a changed text is told whole, so the book is not read again",
  ).toBe(0);

  // when: the author adds a chapter
  const outline = (await projectsOutline(page)).id;
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { title: "Evening", under: null, after: null },
  });
  expect(added.status()).toBe(201);

  // then: the reader sees it
  await expect(
    book(reader).getByRole("heading", { level: 2, name: "Evening" }),
  ).toBeVisible({ timeout: 15_000 });
});

const switcher = (page: Page) => page.locator(".reading-view .switcher");

const whatToRead = (page: Page) =>
  page.getByRole("dialog", { name: "What to read" });

const DAY = 86_400_000;

const daysFromNow = (days: number) =>
  new Date(Date.now() + days * DAY).toISOString();

/// Makes a reading copy of the project, once the readings know as much of the work as it holds.
async function aCopyMade(
  page: Page,
  copy: {
    name: string;
    shared: object;
    open_until: string | null;
    deleted_on: string | null;
  },
) {
  await expect(async () => {
    const made = await page.request.post(`${API}/readings/copies`, {
      data: { project: projectOf(page), ...copy },
    });
    expect(made.status(), await made.text()).toBe(201);
  }, "the readings learn of the outline from its messages").toPass();
}

/// Waits until the project's copies are listed under these names, newest first.
/// The list is kept from the copies' messages, so it lags a moment behind a change.
async function copiesListed(page: Page, names: string[]) {
  await expect
    .poll(
      async () => {
        const listed = await page.request.get(
          `${API}/readings/copies?project=${projectOf(page)}`,
        );

        return listed.ok()
          ? (await listed.json()).map((copy: { name: string }) => copy.name)
          : [];
      },
      { message: "the readings list a copy once they hear it was made" },
    )
    .toEqual(names);
}

/// A project's reading view, the work shaped but nothing written, with two copies:
/// an older one of its section, closed and kept for good, and a newer one of the whole work, open.
async function aReadingView(page: Page, named: string) {
  const title = await aNewProject(page, named);
  const { ids } = await aBookShaped(page, [{ title: "Morning" }]);
  await readable(page);
  await aCopyMade(page, {
    name: "First draft, part one",
    shared: { kind: "sections", first: ids["Morning"], last: ids["Morning"] },
    open_until: daysFromNow(-10),
    deleted_on: null,
  });
  await aCopyMade(page, {
    name: "Draft for Lena",
    shared: { kind: "book" },
    open_until: daysFromNow(28),
    deleted_on: daysFromNow(88),
  });
  await copiesListed(page, ["Draft for Lena", "First draft, part one"]);
  await openTheReadingView(page);

  return title;
}

test("the switcher lists the live work and every reading copy, with when each closes and is deleted", async ({
  page,
}) => {
  // given: a project's reading view
  await aReadingView(page, "ReadingSwitcher");

  // expect: the switcher marks the live work as live
  await expect(switcher(page).locator(".when")).toHaveText("Live");

  // when: the switcher is opened
  await switcher(page).click();

  // then: the live work is listed first, as what is read
  await expect(switcher(page)).toHaveAttribute("aria-expanded", "true");
  const entries = whatToRead(page).locator(".entry");
  await expect(entries.first()).toContainText("The work as it is now");
  await expect(entries.first()).toHaveAttribute("aria-current", "true");
  // and: then every copy, newest first, with who made it and what it holds
  await expect(
    whatToRead(page).getByRole("heading", { name: "Reading copies · 2" }),
  ).toBeVisible();
  await expect(whatToRead(page).locator(".entry .name")).toHaveText([
    "The work as it is now",
    "Draft for Lena",
    "First draft, part one",
  ]);
  await expect(whatToRead(page).locator(".entry .meta")).toHaveText([
    "You · the whole work",
    "You · one section",
  ]);
  // and: whether each is open, and when it is deleted
  await expect(entries.nth(1)).toContainText("Open until");
  await expect(entries.nth(2)).toContainText("Closed");
  await expect(entries.nth(2)).toContainText("Kept for good");
});

test("a copy chosen is read as it was frozen, and the live work is a choice away", async ({
  page,
}) => {
  // given: a project's reading view, its copies made of the work's one section
  const title = await aReadingView(page, "ReadingCopy");
  // and: a section added to the work since
  const outline = (await projectsOutline(page)).id;
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { title: "Evening", under: null, after: null },
  });
  expect(added.status()).toBe(201);
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Evening" }),
  ).toBeVisible({ timeout: 15_000 });

  // when: a copy is chosen
  await switcher(page).click();
  await whatToRead(page)
    .getByRole("link", { name: /Draft for Lena/ })
    .click();

  // then: it is read, named in the switcher, with when it was frozen and how long it is open
  await expect(page).toHaveURL(/\/read\/copy_\w+$/);
  await expect(switcher(page)).toContainText("Draft for Lena");
  await expect(switcher(page).locator(".when")).toHaveText(
    /^Frozen .* · open until .* · deleted on .*$/,
  );
  // and: it holds the work as it stood, without the section added since
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Morning" }),
  ).toBeVisible();
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Evening" }),
  ).toHaveCount(0);
  // and: the path names it a reading copy
  await expect(page.getByRole("navigation", { name: "Path" })).toContainText(
    "Reading copy",
  );

  // when: the live work is chosen again
  await switcher(page).click();
  await whatToRead(page)
    .getByRole("link", { name: /The work as it is now/ })
    .click();

  // then: the live work is read, with the section added since
  await expect(page).toHaveURL(/\/read$/);
  await expect(
    book(page).getByRole("heading", { level: 1, name: title }),
  ).toBeVisible();
  await expect(
    book(page).getByRole("heading", { level: 2, name: "Evening" }),
  ).toBeVisible();
});

test("Escape closes the switcher and hands the focus back to it", async ({
  page,
}) => {
  // given: a project's reading view, the switcher open
  await aReadingView(page, "ReadingSwitcherKeys");
  await switcher(page).click();
  await whatToRead(page).locator(".entry").first().focus();

  // when: Escape is pressed
  await page.keyboard.press("Escape");

  // then: the list is gone and the switcher has the focus
  await expect(whatToRead(page)).toBeHidden();
  await expect(switcher(page)).toBeFocused();
  await expect(switcher(page)).toHaveAttribute("aria-expanded", "false");
});

test("on a phone, the contents start closed and open from the status bar, with no tab over the text", async ({
  page,
}) => {
  // given: a project's reading view on a phone
  await page.setViewportSize({ width: 375, height: 700 });
  await aReadingView(page, "ReadingPhone");

  // then: no tab lies over the text, the button sits in the status bar
  await expect(contentsTab(page)).toBeHidden();
  const status = page.getByRole("contentinfo", { name: "Status" });
  const toggle = page.getByRole("button", { name: "Contents", exact: true });
  const bar = (await status.boundingBox())!;
  const seen = (await toggle.boundingBox())!;
  expect(seen.y, "it sits in the status bar").toBeGreaterThanOrEqual(bar.y);

  // and: the contents start closed, as they would lie over the text
  await expect(contents(page)).toBeHidden();
  await expect(toggle).toHaveAttribute("aria-expanded", "false");

  // when: it is pressed
  await toggle.click();

  // then: they show, and the button says so
  await expect(contents(page)).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
});

const sharing = (page: Page) =>
  page.getByRole("dialog", { name: "Share a reading copy" });

async function openTheShareDialog(page: Page) {
  await switcher(page).click();
  await whatToRead(page)
    .getByRole("button", { name: "Share a reading copy" })
    .click();
  await expect(sharing(page)).toBeVisible();
}

function copyAskedFor(page: Page) {
  return page.waitForRequest(
    (asked) =>
      asked.method() === "POST" && asked.url().endsWith("/api/readings/copies"),
  );
}

test("an author shares the whole work as a reading copy, and is taken to it", async ({
  page,
}) => {
  // given: a project's reading view, the share dialog open
  await aReadingView(page, "ReadingShare");
  await openTheShareDialog(page);

  // expect: the copy is named for the day, the whole work shared
  const name = sharing(page).getByLabel("Name of the copy");
  await expect(name).toHaveValue(/^Reading copy, /);
  await expect(sharing(page).getByLabel("What to share")).toHaveValue("book");

  // when: it is named and made
  await name.fill("For Lena");
  const asked = copyAskedFor(page);
  await sharing(page).getByRole("button", { name: "Make the copy" }).click();

  // then: the whole work is asked for under that name, closing before it is deleted
  const made = (await asked).postDataJSON();
  expect(made.name).toBe("For Lena");
  expect(made.shared).toEqual({ kind: "book" });
  expect(Date.parse(made.open_until)).toBeLessThan(Date.parse(made.deleted_on));
  // and: the dialog is gone and the copy is read
  await expect(sharing(page)).toBeHidden();
  await expect(page).toHaveURL(/\/read\/copy_\w+$/);
  await expect(switcher(page)).toContainText("For Lena");
});

test("sections in a row are shared from the first ticked to the last", async ({
  page,
}) => {
  // given: a work of four sections, its share dialog open on sections in a row
  await aNewProject(page, "ReadingShareSections");
  const { ids } = await aBookShaped(page, [
    { title: "One" },
    { title: "Two", after: "One" },
    { title: "Three", after: "Two" },
    { title: "Four", after: "Three" },
  ]);
  await readable(page);
  await openTheReadingView(page);
  await openTheShareDialog(page);
  await sharing(page).getByLabel("What to share").selectOption("sections");

  // when: the first and the third are ticked
  await sharing(page).getByRole("checkbox", { name: "One" }).check();
  await sharing(page).getByRole("checkbox", { name: "Three" }).check();

  // then: the second is taken in, the fourth not
  for (const [title, ticked] of [
    ["One", true],
    ["Two", true],
    ["Three", true],
    ["Four", false],
  ] as const) {
    await expect(
      sharing(page).getByRole("checkbox", { name: title }),
    ).toBeChecked({
      checked: ticked,
    });
  }
  await expect(sharing(page)).toContainText("3 sections chosen.");

  // when: the copy is made
  const asked = copyAskedFor(page);
  await sharing(page).getByRole("button", { name: "Make the copy" }).click();

  // then: the run from the first to the third is asked for
  expect((await asked).postDataJSON().shared).toEqual({
    kind: "sections",
    first: ids["One"],
    last: ids["Three"],
  });
});

test("a copy open with no end closes when it is deleted, unless it is kept for good", async ({
  page,
}) => {
  // given: a project's share dialog
  await aReadingView(page, "ReadingShareTimes");
  await openTheShareDialog(page);
  const open = sharing(page).getByRole("group", { name: "Open for feedback" });
  const kept = sharing(page).getByRole("group", {
    name: "Deleted with its comments",
  });

  // when: it is to be open with no end, and deleted after 90 days
  await open.getByRole("button", { name: "No end" }).click();

  // then: it is open until the day it is deleted
  const deletedOn = (await sharing(page)
    .getByText(/^On .*\. Members can read it/)
    .textContent())!.match(/^On (.*)\. Members/)![1];
  await expect(
    sharing(page).getByText(
      `Until ${deletedOn}. Its readers can read and comment until then.`,
    ),
  ).toBeVisible();

  // when: it is kept for good
  await kept.getByRole("button", { name: "Keep for good" }).click();

  // then: it is open with no end
  await expect(sharing(page).getByText(/^With no end\./)).toBeVisible();
  await expect(
    sharing(page).getByText("Kept for good, with its comments."),
  ).toBeVisible();
});

test("scenes in a row are shared from the first ticked to the last, across their sections", async ({
  page,
}) => {
  // given: two chapters of titled scenes, the share dialog open on scenes in a row
  await aNewProject(page, "ReadingShareScenes");
  const scenes = await aBookWithScenes(page, { Morning: 2, Market: 1 });
  const [harbour, wall] = scenes["Morning"];
  const [stalls] = scenes["Market"];
  for (const [scene, title] of [
    [harbour, "At the harbour"],
    [wall, "On the wall"],
    [stalls, "The stalls"],
  ]) {
    await titled(page, scene, title);
  }
  await readable(page);
  await openTheReadingView(page);
  await openTheShareDialog(page);
  await sharing(page).getByLabel("What to share").selectOption("scenes");

  // expect: the scenes stand under their sections, which cannot be ticked
  await expect(sharing(page).locator(".tree li.heading")).toHaveText([
    "Morning",
    "Market",
  ]);
  await expect(sharing(page).getByRole("checkbox")).toHaveCount(3);

  // when: the first and the last scene are ticked
  await sharing(page).getByRole("checkbox", { name: "At the harbour" }).check();
  await sharing(page).getByRole("checkbox", { name: "The stalls" }).check();

  // then: the scene between is taken in, across the chapters
  await expect(
    sharing(page).getByRole("checkbox", { name: "On the wall" }),
  ).toBeChecked();
  await expect(sharing(page)).toContainText("3 scenes chosen.");

  // when: the copy is made
  const asked = copyAskedFor(page);
  await sharing(page).getByRole("button", { name: "Make the copy" }).click();

  // then: the run from the first scene to the last is asked for
  expect((await asked).postDataJSON().shared).toEqual({
    kind: "scenes",
    first: harbour,
    last: stalls,
  });
});

const copyTools = (page: Page) =>
  page.getByRole("button", { name: "More about this copy" });

/// A project's reading view on its newest copy, which the one person without accounts may change.
async function aCopyRead(page: Page, named: string) {
  await aReadingView(page, named);
  await switcher(page).click();
  await whatToRead(page)
    .getByRole("link", { name: /Draft for Lena/ })
    .click();
  await expect(switcher(page)).toContainText("Draft for Lena");
}

test("whoever may change a copy renames it from its menu", async ({ page }) => {
  // given: a copy being read
  await aCopyRead(page, "ReadingRename");

  // when: it is renamed from its menu
  await copyTools(page).click();
  await page.getByRole("menuitem", { name: "Rename…" }).click();
  const renaming = page.getByRole("dialog", { name: "Rename the copy" });
  const name = renaming.getByLabel("Name of the copy");
  await expect(name).toHaveValue("Draft for Lena");
  await name.fill("For Lena, again");
  const asked = page.waitForRequest(
    (sent) => sent.method() === "PUT" && sent.url().endsWith("/name"),
  );
  await renaming.getByRole("button", { name: "Rename", exact: true }).click();

  // then: the new name is asked for, and the copy is read under it
  expect((await asked).postDataJSON()).toEqual({ name: "For Lena, again" });
  await expect(renaming).toBeHidden();
  await expect(switcher(page)).toContainText("For Lena, again");
  // and: it is still named so when read again
  await page.reload();
  await expect(switcher(page)).toContainText("For Lena, again");
});

test("a copy deleted from its menu is gone, and the live work is read", async ({
  page,
}) => {
  // given: a copy being read
  await aCopyRead(page, "ReadingDelete");

  // when: it is deleted from its menu, as the dialog warns
  await copyTools(page).click();
  await page.getByRole("menuitem", { name: "Delete…" }).click();
  const deleting = page.getByRole("dialog", { name: "Delete the copy?" });
  await expect(deleting).toContainText(
    "“Draft for Lena” is deleted for good, with all its comments.",
  );
  const asked = page.waitForRequest((sent) => sent.method() === "DELETE");
  await deleting.getByRole("button", { name: "Delete for good" }).click();

  // then: it is deleted, and the live work is read
  expect((await asked).url()).toMatch(/\/api\/readings\/copies\/copy_/);
  await expect(page).toHaveURL(/\/read$/);
  await expect(page.locator(".reading-view .chip.live")).toBeVisible();
  // and: only the other copy is listed, even when the page is read again
  await copiesListed(page, ["First draft, part one"]);
  await page.reload();
  await switcher(page).click();
  await expect(whatToRead(page).locator(".entry .name")).toHaveText([
    "The work as it is now",
    "First draft, part one",
  ]);
});

test("the copy's menu and the switcher's list close at a click beside them", async ({
  page,
}) => {
  // given: a copy being read, its menu open
  await aCopyRead(page, "ReadingClickAway");
  await copyTools(page).click();
  await expect(page.getByRole("menu")).toBeVisible();

  // when: the page beside it is clicked
  await page.mouse.click(1200, 450);

  // then: the menu is gone
  await expect(page.getByRole("menu")).toBeHidden();
  await expect(copyTools(page)).toHaveAttribute("aria-expanded", "false");

  // when: the switcher's list is opened, and the page beside it clicked
  await switcher(page).click();
  await expect(whatToRead(page)).toBeVisible();
  await page.mouse.click(1200, 450);

  // then: the list is gone too
  await expect(whatToRead(page)).toBeHidden();
});

test("the contents fold away the sections under a section, one by one or all at once", async ({
  page,
}) => {
  // given: a work of a part with two chapters under it, and a part with none
  await aNewProject(page, "ReadingFolding");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Threads", under: "Part One" },
    { title: "The Market", under: "Part One", after: "Threads" },
    { title: "Part Two", after: "Part One" },
  ]);
  await readable(page);
  await openTheReadingView(page);
  const entries = contents(page).locator(".rail-entry");
  const unfoldAll = contents(page).getByRole("button", {
    name: "Unfold all",
    exact: true,
  });
  const foldAll = contents(page).getByRole("button", {
    name: "Fold all",
    exact: true,
  });

  // expect: every section shows, and only a part with chapters under it can be folded
  await expect(entries).toHaveText([
    "Part One",
    "Threads",
    "The Market",
    "Part Two",
  ]);
  await expect(contents(page).locator(".fold")).toHaveCount(1);
  await expect(unfoldAll).toBeDisabled();

  // when: everything is folded
  await foldAll.click();

  // then: the chapters are gone, and nothing more can be folded
  await expect(entries).toHaveText(["Part One", "Part Two"]);
  await expect(foldAll).toBeDisabled();

  // when: the first part is unfolded by itself
  await contents(page)
    .getByRole("button", { name: /Part One/, expanded: false })
    .click();

  // then: its chapters show again
  await expect(entries).toHaveText([
    "Part One",
    "Threads",
    "The Market",
    "Part Two",
  ]);
  await expect(unfoldAll).toBeDisabled();
});

test("whoever may change a copy extends its dates, which only move later", async ({
  page,
}) => {
  // given: a copy being read, open for four more weeks and deleted in about three months
  await aCopyRead(page, "ReadingExtend");

  // when: its dates are to be extended
  await copyTools(page).click();
  await page.getByRole("menuitem", { name: "Extend dates…" }).click();
  const extending = page.getByRole("dialog", { name: "Extend the dates" });
  const extend = extending.getByRole("button", { name: "Extend", exact: true });

  // then: nothing is changed until something is chosen
  await expect(extend).toBeDisabled();

  // when: it is opened 30 days more and kept a year more
  await extending
    .getByRole("group", { name: "Open for feedback" })
    .getByRole("button", { name: "30 days more" })
    .click();
  await extending
    .getByRole("group", { name: "Deleted with its comments" })
    .getByRole("button", { name: "A year more" })
    .click();
  const asked = page.waitForRequest(
    (sent) => sent.method() === "PUT" && sent.url().endsWith("/times"),
  );
  await extend.click();

  // then: later times are asked for, closing before the copy is deleted
  const times = (await asked).postDataJSON();
  const days = (moment: string) =>
    (Date.parse(moment) - Date.now()) / 86_400_000;
  expect(days(times.open_until)).toBeGreaterThan(55);
  expect(days(times.deleted_on)).toBeGreaterThan(450);
  // and: the copy is read with them
  await expect(extending).toBeHidden();
  await expect(switcher(page).locator(".when")).toContainText("open until");
});

test("a copy kept for good offers no deletion to extend", async ({ page }) => {
  // given: the older copy, closed and kept for good, being read
  await aReadingView(page, "ReadingExtendKept");
  await switcher(page).click();
  await whatToRead(page)
    .getByRole("link", { name: /First draft, part one/ })
    .click();

  // when: its dates are to be extended
  await copyTools(page).click();
  await page.getByRole("menuitem", { name: "Extend dates…" }).click();
  const extending = page.getByRole("dialog", { name: "Extend the dates" });

  // then: it may be opened again, but its deletion has nothing later to go to
  await expect(
    extending.getByRole("group", { name: "Open for feedback" }),
  ).toBeVisible();
  await expect(
    extending.getByRole("group", { name: "Deleted with its comments" }),
  ).toHaveCount(0);
  await expect(extending).toContainText("Kept for good, with its comments.");
});

const feedback = (page: Page) =>
  page.getByRole("complementary", { name: "Feedback" });

const feedbackTab = (page: Page) =>
  page.locator('.drawer-tab[aria-controls="reading-feedback"]');

/// A draft written through the API on the copy being read, saying one plain sentence.
async function aDraftOn(page: Page, place: object, saying: string) {
  const copy = new URL(page.url()).pathname.split("/").pop()!;
  const drafted = await page.request.post(`${API}/feedback`, {
    data: {
      about: { kind: "reading_copy", reading_copy: copy, place },
      comment: [{ kind: "paragraph", runs: [{ kind: "text", text: saying }] }],
    },
  });
  expect(drafted.status(), await drafted.text()).toBe(201);
}

/// The first section of the copy being read, as it was frozen.
async function aSectionOfTheCopy(page: Page) {
  const copy = new URL(page.url()).pathname.split("/").pop()!;
  const read = await (
    await page.request.get(`${API}/readings/copies/${copy}`)
  ).json();

  return read.contents.parts.find(
    (part: { kind: string }) => part.kind === "section",
  ).section as string;
}

test("a copy's feedback lists your drafts, each with what it is about, and one deleted is gone", async ({
  page,
}) => {
  // given: a copy being read, its feedback closed where it and the contents do not both fit beside the text
  await aCopyRead(page, "ReadingDrafts");
  await expect(feedbackTab(page)).toHaveAttribute("aria-expanded", "false");

  // when: the feedback is opened
  await feedbackTab(page).click();

  // then: nothing is said about the copy yet
  await expect(feedback(page)).toContainText(
    "Nothing said about this copy yet.",
  );

  // when: drafts are written on the whole copy and on its section
  await aDraftOn(page, { kind: "whole" }, "The ending lands.");
  await aDraftOn(
    page,
    { kind: "section", section: await aSectionOfTheCopy(page) },
    "Morning drags a little.",
  );
  await page.reload();

  // then: they are listed as your drafts, which only you see, each saying what it is about
  const drafts = feedback(page).getByRole("region", { name: "Your drafts" });
  await expect(drafts).toContainText("Your drafts · 2");
  await expect(drafts).toContainText("only you see these");
  await expect(drafts.locator(".note")).toHaveCount(2);
  await expect(drafts.locator(".note").first()).toContainText(
    "On the whole copy",
  );
  await expect(drafts.locator(".note").first()).toContainText(
    "The ending lands.",
  );
  await expect(drafts.locator(".note").nth(1)).toContainText("On “Morning”");

  // when: the first is deleted
  await drafts
    .locator(".note")
    .first()
    .getByRole("button", { name: "Delete this draft" })
    .click();

  // then: only the other is left, even when the page is read again
  await expect(drafts.locator(".note")).toHaveCount(1);
  await page.reload();
  await expect(
    feedback(page).getByRole("region", { name: "Your drafts" }),
  ).toContainText("Morning drags a little.");
});

test("the live work has no feedback column, as feedback is given on a copy", async ({
  page,
}) => {
  // given: a project's live work being read
  await aReadingView(page, "ReadingLiveNoFeedback");

  // expect: no feedback beside it, nor a tab to open it
  await expect(feedbackTab(page)).toHaveCount(0);
});

test("a copy's contents and its feedback never lie over each other, however wide either is drawn", async ({
  page,
}) => {
  // given: a copy being read on a wide window, with its contents and its feedback open
  await page.setViewportSize({ width: 1440, height: 800 });
  await aCopyRead(page, "ReadingDrawersApart");
  const contentsPane = page.getByRole("complementary", { name: "Contents" });
  if ((await feedbackTab(page).getAttribute("aria-expanded")) === "false") {
    await feedbackTab(page).click();
  }
  await expect(contentsPane).toBeVisible();
  await expect(feedback(page)).toBeVisible();

  // when: the feedback is drawn far over to the left
  const handle = page.getByRole("separator", { name: "Resize the feedback" });
  const at = (await handle.boundingBox())!;
  const y = at.y + Math.min(at.height / 2, 200);
  await page.mouse.move(at.x + at.width / 2, y);
  await page.mouse.down();
  await page.mouse.move(at.x - 400, y, { steps: 4 });
  await page.mouse.move(at.x - 900, y, { steps: 4 });
  await page.mouse.up();

  // then: it stops short of the contents, leaving some of the page in sight between them
  const left = (await contentsPane.boundingBox())!;
  const right = (await feedback(page).boundingBox())!;
  expect(left.x + left.width, "the drawers never overlap").toBeLessThanOrEqual(
    right.x - 160,
  );
});

/// A copy being read, its feedback open.
async function aCopysFeedback(page: Page, named: string) {
  await aCopyRead(page, named);
  if ((await feedbackTab(page).getAttribute("aria-expanded")) === "false") {
    await feedbackTab(page).click();
  }
  await expect(feedback(page)).toBeVisible();
}

const commentField = (page: Page) =>
  feedback(page).getByRole("textbox", { name: "Your comment" });

/// Your drafts on the copy being read, as the server keeps them.
async function draftsKept(page: Page) {
  const copy = new URL(page.url()).pathname.split("/").pop()!;
  const listed = await page.request.get(
    `${API}/feedback?kind=reading_copy&id=${copy}`,
  );
  expect(listed.ok(), await listed.text()).toBe(true);

  return (await listed.json()) as { comment: unknown; about: unknown }[];
}

test("a comment on the whole copy is written with bold, italics and paragraphs, and kept as a draft", async ({
  page,
}) => {
  // given: a copy's feedback, a comment begun on the whole copy
  await aCopysFeedback(page, "ReadingCommentWhole");
  await feedback(page)
    .getByRole("button", { name: "Comment on the whole copy" })
    .click();
  await expect(
    feedback(page).getByRole("region", { name: "A new draft" }),
  ).toContainText("On the whole copy");
  const save = feedback(page).getByRole("button", { name: "Save the draft" });

  // expect: nothing said cannot be saved
  await expect(save).toBeDisabled();

  // when: a sentence is written with a word in bold and one in italics, and a second paragraph
  await commentField(page).click();
  await page.keyboard.type("The ending ");
  await page.keyboard.press("ControlOrMeta+B");
  await page.keyboard.type("lands");
  await page.keyboard.press("ControlOrMeta+B");
  await page.keyboard.type(", ");
  await feedback(page).getByRole("button", { name: "Italic" }).click();
  await page.keyboard.type("truly");
  await feedback(page).getByRole("button", { name: "Italic" }).click();
  await page.keyboard.type(".");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Keep it.");
  await save.click();

  // then: it is listed as your draft, drawn as written
  const drafts = feedback(page).getByRole("region", { name: "Your drafts" });
  await expect(drafts.locator(".note")).toHaveCount(1);
  await expect(drafts.locator(".note strong")).toHaveText("lands");
  await expect(drafts.locator(".note em")).toHaveText("truly");
  await expect(drafts.locator(".note .comment p")).toHaveText([
    "The ending lands, truly.",
    "Keep it.",
  ]);
  // and: kept on the server as blocks and runs, about the whole copy
  const [kept] = await draftsKept(page);
  expect(kept.about).toMatchObject({
    kind: "reading_copy",
    place: { kind: "whole" },
  });
  expect(kept.comment).toEqual([
    {
      kind: "paragraph",
      runs: [
        { kind: "text", text: "The ending " },
        { kind: "text", text: "lands", strong: true },
        { kind: "text", text: ", " },
        { kind: "text", text: "truly", emphasis: true },
        { kind: "text", text: "." },
      ],
    },
    { kind: "paragraph", runs: [{ kind: "text", text: "Keep it." }] },
  ]);
});

test("a link is set on what is selected, and an address that leads nowhere safe is refused", async ({
  page,
}) => {
  // given: a comment begun with a word to link
  await aCopysFeedback(page, "ReadingCommentLink");
  await feedback(page)
    .getByRole("button", { name: "Comment on the whole copy" })
    .click();
  await commentField(page).click();
  await page.keyboard.type("See here");
  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowLeft");
  await page.keyboard.press("Shift+ArrowLeft");

  // when: a script is given as the address
  await feedback(page).getByRole("button", { name: "Link" }).click();
  const address = feedback(page).getByRole("textbox", {
    name: "Address of the link",
  });
  await address.fill("javascript:alert(1)");
  await address.press("Enter");

  // then: it is refused
  await expect(feedback(page).getByRole("alert")).toHaveText(
    "Only web addresses and email addresses can be links.",
  );

  // when: a web address is given without its scheme
  await address.fill("example.org/notes");
  await address.press("Enter");
  await feedback(page).getByRole("button", { name: "Save the draft" }).click();

  // then: the word links to it as a web address
  await expect(
    feedback(page).locator(".drafts .note a", { hasText: "here" }),
  ).toHaveAttribute("href", "https://example.org/notes");
  const [kept] = await draftsKept(page);
  expect(kept.comment).toEqual([
    {
      kind: "paragraph",
      runs: [
        { kind: "text", text: "See " },
        { kind: "text", text: "here", link: "https://example.org/notes" },
      ],
    },
  ]);
});

test("a draft is changed in place, its formatting kept, and Escape leaves it as it was", async ({
  page,
}) => {
  // given: a draft with a word in bold
  await aCopysFeedback(page, "ReadingCommentChange");
  await aDraftOn(page, { kind: "whole" }, "First thought.");
  await page.reload();
  const note = feedback(page).locator(".drafts .note");
  await expect(note).toContainText("First thought.");

  // when: it is changed, and Escape pressed
  await note.getByRole("button", { name: "Change this draft" }).click();
  await expect(commentField(page)).toHaveText("First thought.");
  await commentField(page).press("End");
  await page.keyboard.type(" Not this.");
  await page.keyboard.press("Escape");

  // then: it is as it was
  await expect(note.locator(".comment")).toHaveText("First thought.");

  // when: it is changed again and saved
  await note.getByRole("button", { name: "Change this draft" }).click();
  await commentField(page).press("End");
  await page.keyboard.type(" Second thought.");
  await page.keyboard.press("ControlOrMeta+Enter");

  // then: it says the new words, even when the page is read again
  await expect(note.locator(".comment")).toHaveText(
    "First thought. Second thought.",
  );
  await page.reload();
  await expect(feedback(page).locator(".drafts .note .comment")).toHaveText(
    "First thought. Second thought.",
  );
});

test("what is pasted into a comment keeps only what a comment can hold", async ({
  page,
}) => {
  // given: a comment begun
  await aCopysFeedback(page, "ReadingCommentPaste");
  await feedback(page)
    .getByRole("button", { name: "Comment on the whole copy" })
    .click();
  await commentField(page).click();

  // when: markup is pasted, a heading, words in bold, a script's link and an image among it
  await commentField(page).evaluate((field) => {
    const data = new DataTransfer();
    data.setData(
      "text/html",
      '<h1>Loud</h1><p>Very <b>bold</b> <a href="javascript:alert(1)">trap</a></p><img src=x onerror=alert(1)>',
    );
    data.setData("text/plain", "Loud Very bold trap");
    field.dispatchEvent(
      new ClipboardEvent("paste", {
        clipboardData: data,
        bubbles: true,
        cancelable: true,
      }),
    );
  });

  // then: the heading is a paragraph of its words, the bold is kept, and neither the script's link nor the image is
  await expect(commentField(page).locator("p")).toHaveText([
    "Loud",
    "Very bold trap",
  ]);
  await expect(commentField(page).locator("strong")).toHaveText("bold");
  await expect(commentField(page).locator("h1, img, a")).toHaveCount(0);
});

test("a comment holds lists begun as they are typed, broken lines, and undoes what was typed", async ({
  page,
}) => {
  // given: a comment begun
  await aCopysFeedback(page, "ReadingCommentLists");
  await feedback(page)
    .getByRole("button", { name: "Comment on the whole copy" })
    .click();
  await commentField(page).click();

  // when: a list is typed as one, left by an empty item, a numbered one after it, and a line broken
  await page.keyboard.type("- first");
  await page.keyboard.press("Enter");
  await page.keyboard.type("second");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("1. one");
  await page.keyboard.press("Enter");
  await page.keyboard.press("Enter");
  await page.keyboard.type("Line");
  await page.keyboard.press("Shift+Enter");
  // and: after a pause, more typed and undone, as typing close together is undone together
  await page.waitForTimeout(700);
  await page.keyboard.type("broken, oops");
  await page.keyboard.press("ControlOrMeta+Z");
  await feedback(page).getByRole("button", { name: "Save the draft" }).click();

  // then: it is kept as two lists and a paragraph with its line broken, without what was undone
  await expect(feedback(page).locator(".drafts .note ul li")).toHaveText([
    "first",
    "second",
  ]);
  const [kept] = await draftsKept(page);
  expect(kept.comment).toEqual([
    {
      kind: "list",
      ordered: false,
      items: [
        [{ kind: "text", text: "first" }],
        [{ kind: "text", text: "second" }],
      ],
    },
    { kind: "list", ordered: true, items: [[{ kind: "text", text: "one" }]] },
    {
      kind: "paragraph",
      runs: [{ kind: "text", text: "Line" }, { kind: "break" }],
    },
  ]);
});
