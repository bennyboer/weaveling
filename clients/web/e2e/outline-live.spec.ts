import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import { aNewProject, aSceneMade, API, openTheOutline } from "./support/shell";
import { projectOf, savedTitles, titled } from "./support/writing";
import { heardOn, routingLive } from "./support/live";

async function anOutlineFollowedLive(
  page: Page,
  named: string,
  { routed = false } = {},
) {
  await aNewProject(page, named);
  const book = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  let opened = 0;
  let heard = 0;
  heardOn(page, "outlines", () => {
    heard += 1;
  });
  page.on("response", (response) => {
    if (
      response.request().method() === "POST" &&
      response.url().endsWith("/api/outlines")
    ) {
      opened += 1;
    }
  });
  await openTheOutline(page);
  if (routed) {
    await expect.poll(() => opened).toBe(1);
  } else {
    await expect
      .poll(() => heard, { message: "followed live, told first where it stands" })
      .toBe(1);
  }
  expect(opened, "loaded once, and not again as it is followed").toBe(1);

  return { ...book, reloads: () => opened, heard: () => heard };
}

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "put" | "delete",
  path: string,
  data?: object,
) {
  const done = await page.request[method](`${API}${path}`, { data });
  expect(done.ok(), await done.text()).toBe(true);

  return done;
}

const titleOf = (page: Page, named: string) =>
  page.getByRole("textbox", { name: `Section ${named}` });

test("a section another author adds appears in the outline", async ({
  page,
}) => {
  // given: an outline followed live
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveAdded",
  );

  // when: another author adds a section
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  // then: it appears in the outline
  await expect(titleOf(page, "Part Three")).toBeVisible();
});

test("a scene another author attaches is named in the outline", async ({
  page,
}) => {
  // given: an outline followed live
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveNamed",
  );
  // and: a titled scene
  const scene = await aSceneMade(page);
  await titled(page, scene, "The arrival");

  // when: another author attaches it
  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: scene },
    section: ids["Part One"],
    after: null,
  });

  // then: it is named in the outline
  await expect(
    page.locator(".leaf").getByRole("link", { name: "The arrival" }),
  ).toBeVisible();
});

test("a title I am typing is not overwritten by another author's rename", async ({
  page,
}) => {
  // given: an outline followed live, a title of it being typed in
  const { outline, ids, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveTyping",
  );
  await titleOf(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  const before = heard();

  // when: another author renames it
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);

  // then: what I typed stays
  await expect(page.locator("input.title:focus")).toHaveValue(
    "Part One, drafted",
  );
});

test("a title I only sat in takes another author's rename without writing the old one back", async ({
  page,
}) => {
  // given: an outline followed live, a title of it sat in
  const { outline, ids, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveSatIn",
  );
  await titleOf(page, "Part One").click();
  const before = heard();
  // and: another author renamed it meanwhile
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);

  // when: the title is left
  await page.keyboard.press("Escape");

  // then: it takes their rename, writing back nothing
  await expect(titleOf(page, "The Beginning")).toHaveValue("The Beginning");
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("a scene the listing does not know yet is asked for once, not over and over", async ({
  page,
}) => {
  // given: scenes not heard of live
  await routingLive(page, { withheld: ["scenes"] });
  // and: an outline followed live
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveLagging",
  );
  // and: a scene the listing does not hold yet
  const lagging = await aSceneMade(page);
  let listed = 0;
  await page.route(/\/api\/scenes\?project=/, async (route) => {
    listed += 1;
    const answer = await route.fetch();
    const page_ = await answer.json();
    page_.scenes = page_.scenes.filter(
      (held: { id: string }) => held.id !== lagging,
    );
    await route.fulfill({ response: answer, json: page_ });
  });

  // when: another author attaches it
  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: lagging },
    section: ids["Part One"],
    after: null,
  });
  await expect.poll(() => listed).toBeGreaterThan(0);
  await page.waitForTimeout(1500);

  // then: the scenes are asked for once more, not over and over
  expect(
    listed,
    "asked again once for the change that brought it, not in a loop until the listing catches up",
  ).toBeLessThanOrEqual(2);
});

const renamedMeanwhile = (page: Page) =>
  page.getByRole("dialog", { name: "Renamed meanwhile" });

async function bothRenamed(page: Page, named: string) {
  const { outline, ids, heard } = await anOutlineFollowedLive(page, named);
  await titleOf(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  const before = heard();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);
  await page.keyboard.press("Escape");
}

test("a title renamed elsewhere while I rename it asks which name stays", async ({
  page,
}) => {
  // given: a title I renamed while another author renamed it too
  await bothRenamed(page, "OutlineLiveBoth");

  // then: which name stays is asked
  await expect(renamedMeanwhile(page)).toContainText(
    "someone else renamed it to “The Beginning”. Yours is “Part One, drafted”.",
  );
});

test("keeping my title over one given meanwhile keeps mine", async ({
  page,
}) => {
  // given: a title I renamed while another author renamed it too
  await bothRenamed(page, "OutlineLiveKept");

  // when: mine is kept
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  // then: mine stays
  await expect(titleOf(page, "Part One, drafted")).toHaveValue(
    "Part One, drafted",
  );
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
});

test("taking the title given meanwhile drops mine", async ({ page }) => {
  // given: a title I renamed while another author renamed it too
  await bothRenamed(page, "OutlineLiveTook");

  // when: theirs is taken
  await renamedMeanwhile(page)
    .getByRole("button", { name: "Take theirs" })
    .click();

  // then: theirs stays
  await expect(titleOf(page, "The Beginning")).toHaveValue("The Beginning");
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("renaming, nesting with Tab and renaming on in the same field asks nothing", async ({
  page,
}) => {
  // given: an outline followed live, a title of it being typed in
  await anOutlineFollowedLive(page, "OutlineLiveTabbed");
  await titleOf(page, "Part Two").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");

  // when: the section is nested with tab
  await page.keyboard.press("Tab");
  // then: the title typed so far is saved
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, drafted"]);
  await page.waitForTimeout(300);
  // and: the title is still being typed in
  await expect(page.locator("input.title:focus")).toHaveValue(
    "Part Two, drafted",
  );
  // when: more is typed and the title left
  await page.keyboard.type(" again");
  await page.keyboard.press("Escape");

  // then: nothing is asked
  await expect(renamedMeanwhile(page)).toHaveCount(0);
  // and: the whole title is saved
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, drafted again"]);
});

test("typing the very title someone else gave meanwhile asks nothing", async ({
  page,
}) => {
  // given: an outline followed live, a title of it retyped as another author renames it the same
  const { outline, ids, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveSame",
  );
  await titleOf(page, "Part One").click();
  await page.keyboard.press("ControlOrMeta+A");
  await page.keyboard.type("The Beginning");
  const before = heard();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);

  // when: the title is left
  await page.keyboard.press("Escape");

  // then: nothing is asked
  await page.waitForTimeout(300);
  await expect(renamedMeanwhile(page)).toHaveCount(0);
});

test("a move made from an outline that is behind is refused gently, and the outline catches up", async ({
  page,
}) => {
  // given: an outline that hears nothing
  await routingLive(page, { withheld: ["outlines"] });
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineOvertaken",
    { routed: true },
  );
  // and: a section removed meanwhile
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part Two"]}`,
  );
  await page.waitForTimeout(300);
  await expect(
    titleOf(page, "Part Two"),
    "this outline hears nothing",
  ).toBeVisible();
  // when: that section is moved
  await titleOf(page, "Part Two").click();

  await page.keyboard.press("Alt+ArrowUp");

  // then: it is refused gently
  await expect(
    page.getByText("Someone else changed this meanwhile"),
  ).toBeVisible();
  // and: the outline catches up
  await expect(titleOf(page, "Part Two")).toHaveCount(0);
});

async function anOutlineThatHearsNothing(
  page: Page,
  named: string,
  sections: { title: string; after?: string }[],
) {
  await routingLive(page, { withheld: ["outlines"] });
  await aNewProject(page, named);
  const book = await aBookShaped(page, sections);
  await openTheOutline(page);

  return book;
}

test("a section moved up twice in a row ends two places up", async ({ page }) => {
  // given: an outline of three sections
  const { outline } = await anOutlineThatHearsNothing(page, "OutlineUpTwice", [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "Part Three", after: "Part Two" },
  ]);
  // and: moves slow to answer
  await page.route(
    (url) => url.pathname.startsWith(`/api/outlines/${outline}/sections/`) && url.pathname.endsWith("/place"),
    async (route) => {
      await new Promise((resolve) => setTimeout(resolve, 300));
      await route.fallback();
    },
  );

  // when: the last is moved up twice in a row
  await titleOf(page, "Part Three").click();
  await page.keyboard.press("Alt+ArrowUp");
  await page.keyboard.press("Alt+ArrowUp");

  // then: it is saved two places up
  await expect
    .poll(() => savedTitles(page), {
      message: "each move up is worked out from where the one before left it",
    })
    .toEqual(["Part Three", "Part One", "Part Two"]);
});

test("a section moved up while another author moved the one above it away is moved up among what is there now", async ({
  page,
}) => {
  // given: an outline of three sections that hears nothing
  const { outline, ids } = await anOutlineThatHearsNothing(page, "OutlineUpRaced", [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "Part Three", after: "Part Two" },
  ]);
  // and: another author moving the first to the end just as my move goes out
  let mine = 0;
  await page.route(
    (url) => url.pathname === `/api/outlines/${outline}/sections/${ids["Part Three"]}/place`,
    async (route) => {
      mine += 1;
      if (mine === 1) {
        await anotherAuthor(page, "put", `/outlines/${outline}/sections/${ids["Part One"]}/place`, {
          under: null,
          after: ids["Part Three"],
        });
      }
      await route.fallback();
    },
  );

  // when: the last is moved up
  await titleOf(page, "Part Three").click();
  await page.keyboard.press("Alt+ArrowUp");

  // then: it is moved up among the sections as they now stand
  await expect
    .poll(() => savedTitles(page), {
      message: "worked out on the order it was shown, it would land after the section moved away and so go down",
    })
    .toEqual(["Part Three", "Part Two", "Part One"]);
  // and: my move was refused once, as made on an outline that had moved on, and made again
  expect(mine).toBe(2);
});

test("a move worked out on a section removed meanwhile is worked out again on what remains", async ({
  page,
}) => {
  // given: an outline that hears nothing
  const { outline, ids } = await anOutlineThatHearsNothing(
    page,
    "OutlineGoneNeighbour",
    [
      { title: "Part One" },
      { title: "Part Two", after: "Part One" },
      { title: "Part Three", after: "Part Two" },
    ],
  );
  // and: a section removed meanwhile
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
  );
  // when: the last is moved up, which on what was shown places it after the removed one
  await titleOf(page, "Part Three").click();

  await page.keyboard.press("Alt+ArrowUp");

  // then: it is moved up among the sections that remain
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part Three", "Part Two"]);
  // and: the outline catches up
  await expect(titleOf(page, "Part One")).toHaveCount(0);
  // and: nothing is refused
  await expect(page.getByText("Someone else changed this meanwhile")).toHaveCount(0);
});

test("asked which name stays, the question follows the language chosen meanwhile", async ({
  page,
}) => {
  // given: asked which name stays
  await bothRenamed(page, "OutlineLiveSpoken");
  await expect(renamedMeanwhile(page)).toBeVisible();

  // when: german is chosen
  await page.getByRole("combobox", { name: "Language" }).selectOption("de");

  // then: the question is asked in german
  const asked = page.getByRole("dialog", { name: "Inzwischen umbenannt" });
  await expect(asked).toContainText(
    "Während du diesen Titel bearbeitet hast, hat jemand anderes ihn in „The Beginning“ umbenannt. Deiner lautet „Part One, drafted“.",
  );
  await expect(asked.getByRole("button", { name: "Ihren übernehmen" })).toBeVisible();
  await expect(asked.getByRole("button", { name: "Meinen behalten" })).toBeVisible();
});

const shownShape = (page: Page) =>
  page.evaluate(() =>
    [...document.querySelectorAll(".branch")].map((branch) => {
      let depth = 0;
      for (let up = branch.parentElement; up; up = up.parentElement) {
        if (up.classList.contains("twigs")) depth += 1;
      }
      const title = branch.querySelector<HTMLInputElement>(".title")!.value;

      return `${depth} ${title}`;
    }),
  );

async function savedShape(page: Page, outline: string) {
  const found = await (await page.request.get(`${API}/outlines/${outline}`)).json();
  const depth = (section: { parent: string | null }): number => {
    let deep = 0;
    let parent = section.parent;
    while (parent) {
      deep += 1;
      parent = found.sections.find((held: { section: string }) => held.section === parent)?.parent ?? null;
    }
    return deep;
  };

  return found.sections.map(
    (section: { title: string; parent: string | null }) =>
      `${depth(section)} ${section.title}`,
  );
}

test("a section another author adds is taken in, without opening the outline again", async ({
  page,
}) => {
  // given: an outline followed live
  const { outline, ids, reloads, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveTakenIn",
  );
  const before = reloads();
  const told = heard();

  // when: another author adds a section
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  // then: it appears
  await expect(titleOf(page, "Part Three")).toBeVisible();
  // and: the outline is not opened again
  await expect.poll(() => heard()).toBeGreaterThan(told);
  await page.waitForTimeout(500);
  expect(reloads(), "the section was taken in as it came").toBe(before);
});

test("many changes by another author, taken in one by one, leave the outline as the server holds it", async ({
  page,
}) => {
  // given: an outline followed live
  const { outline, ids, reloads, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveMany",
  );
  const added = async (title: string, under: string | null, after: string | null) => {
    const done = await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
      title,
      under,
      after,
    });
    return (await done.json()).section as string;
  };
  const before = reloads();
  const told = heard();

  // when: another author changes it many times over
  const chapter = await added("Chapter 1", ids["Part One"], null);
  const second = await added("Chapter 2", ids["Part One"], chapter);
  await added("Chapter 3", ids["Part One"], second);
  await anotherAuthor(page, "post", `/outlines/${outline}/sections/${second}/promote`);
  await anotherAuthor(page, "post", `/outlines/${outline}/sections/${ids["Part Two"]}/demote`);
  await anotherAuthor(page, "put", `/outlines/${outline}/sections/${chapter}/place`, {
    under: null,
    after: null,
  });
  await anotherAuthor(page, "delete", `/outlines/${outline}/sections/${ids["Part One"]}`);
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${second}`, {
    title: "The Second Chapter",
  });

  // then: it is shown as the server holds it
  const saved = await savedShape(page, outline);
  await expect.poll(() => shownShape(page)).toEqual(saved);
  // and: it was not opened again
  await expect.poll(() => heard()).toBeGreaterThanOrEqual(told + 8);
  expect(reloads(), "every change followed on, so none was opened again").toBe(before);
});


test("a change missed on the way opens the outline again, and the outline comes out right", async ({
  page,
}) => {
  // given: an outline followed live that will miss a change
  let passed = 0;
  let connected = false;
  await routingLive(page, {
    connecting: () => {
      connected = true;
      return true;
    },
    heard: {
      outlines: (_told, pass) => {
        passed += 1;
        if (passed !== 2) {
          pass();
        }
      },
    },
  });
  await aNewProject(page, "OutlineLiveMissed");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheOutline(page);
  await expect.poll(() => connected).toBe(true);
  await page.waitForTimeout(300);

  // when: another author renames its sections, one change missed on the way
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part One"]}`, {
    title: "The Beginning",
  });
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part One"]}`, {
    title: "The Very Beginning",
  });
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part Two"]}`, {
    title: "The Middle",
  });

  // then: every rename shows
  await expect.poll(() => passed).toBeGreaterThanOrEqual(3);
  await expect(titleOf(page, "The Middle")).toHaveValue("The Middle");
  await expect(
    titleOf(page, "The Very Beginning"),
    "the rename that was missed is there too, so the outline was opened again",
  ).toHaveValue("The Very Beginning");
});

test("an outline is opened once, and a change made before it is followed is caught up as it connects", async ({
  page,
}) => {
  // given: an outline shown but not yet followed
  const live = await routingLive(page, { held: ["outlines"] });
  const connect = () => live.release("outlines");
  const { outline, reloads } = await anOutlineFollowedLive(page, "OutlineHello", {
    routed: true,
  });

  // when: another author adds a section before it is followed
  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Written meanwhile",
    under: null,
    after: null,
  });
  // and: the outline then connects
  connect();

  // then: the section shows
  await expect(
    titleOf(page, "Written meanwhile"),
    "told where the outline stands, a newer version than held, it is opened again",
  ).toBeVisible();
  // and: the outline was opened once more
  expect(reloads()).toBe(2);
});

test("an idea noted elsewhere that the list of ideas does not hold yet is asked for, and named", async ({
  page,
}) => {
  // given: ideas not heard of live
  await routingLive(page, { withheld: ["ideas"] });
  // and: an outline followed live
  const { outline, ids } = await anOutlineFollowedLive(page, "OutlineUnknownIdea");
  // and: an idea captured elsewhere
  const captured = await anotherAuthor(page, "post", "/ideas", {
    project: projectOf(page),
    title: "Winter comes early",
  });
  const winter = (await captured.json()).id as string;

  // when: another author notes it in the outline
  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "idea", id: winter },
    section: ids["Part One"],
    after: null,
  });

  // then: it is named
  await expect(
    page.locator(".leaf .name"),
    "the outline hears of the idea only through the note, so it asks for the ideas again",
  ).toHaveText("Winter comes early");
});
