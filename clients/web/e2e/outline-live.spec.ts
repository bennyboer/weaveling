import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import { aNewProject, aSceneMade, API, openTheOutline } from "./support/shell";
import { savedTitles, titled } from "./support/writing";

async function anOutlineFollowedLive(page: Page, named: string) {
  await aNewProject(page, named);
  const book = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  let opened = 0;
  let heard = 0;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/outlines/live")) {
      socket.on("framereceived", () => {
        heard += 1;
      });
    }
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
  await expect
    .poll(() => opened, {
      message:
        "the outline is loaded, and loaded again once it is followed live",
    })
    .toBeGreaterThanOrEqual(2);

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
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveAdded",
  );

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  await expect(titleOf(page, "Part Three")).toBeVisible();
});

test("a scene another author attaches is named in the outline", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveNamed",
  );
  const scene = await aSceneMade(page);
  await titled(page, scene, "The arrival");

  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: scene },
    section: ids["Part One"],
    after: null,
  });

  await expect(
    page.locator(".leaf").getByRole("link", { name: "The arrival" }),
  ).toBeVisible();
});

test("a title I am typing is not overwritten by another author's rename", async ({
  page,
}) => {
  const { outline, ids, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveTyping",
  );
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

  await expect(page.locator("input.title:focus")).toHaveValue(
    "Part One, drafted",
  );
});

test("a title I only sat in takes another author's rename without writing the old one back", async ({
  page,
}) => {
  const { outline, ids, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveSatIn",
  );
  await titleOf(page, "Part One").click();
  const before = heard();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => heard()).toBeGreaterThan(before);

  await page.keyboard.press("Escape");

  await expect(titleOf(page, "The Beginning")).toHaveValue("The Beginning");
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("a scene the listing does not know yet is asked for once, not over and over", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveLagging",
  );
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

  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: lagging },
    section: ids["Part One"],
    after: null,
  });
  await expect.poll(() => listed).toBeGreaterThan(0);
  await page.waitForTimeout(1500);

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
  await bothRenamed(page, "OutlineLiveBoth");

  await expect(renamedMeanwhile(page)).toContainText(
    "someone else renamed it to “The Beginning”. Yours is “Part One, drafted”.",
  );
});

test("keeping my title over one given meanwhile keeps mine", async ({
  page,
}) => {
  await bothRenamed(page, "OutlineLiveKept");

  await renamedMeanwhile(page)
    .getByRole("button", { name: "Keep mine" })
    .click();

  await expect(titleOf(page, "Part One, drafted")).toHaveValue(
    "Part One, drafted",
  );
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One, drafted", "Part Two"]);
});

test("taking the title given meanwhile drops mine", async ({ page }) => {
  await bothRenamed(page, "OutlineLiveTook");

  await renamedMeanwhile(page)
    .getByRole("button", { name: "Take theirs" })
    .click();

  await expect(titleOf(page, "The Beginning")).toHaveValue("The Beginning");
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("renaming, nesting with Tab and renaming on in the same field asks nothing", async ({
  page,
}) => {
  await anOutlineFollowedLive(page, "OutlineLiveTabbed");
  await titleOf(page, "Part Two").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");

  await page.keyboard.press("Tab");
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, drafted"]);
  await page.waitForTimeout(300);
  await expect(page.locator("input.title:focus")).toHaveValue(
    "Part Two, drafted",
  );
  await page.keyboard.type(" again");
  await page.keyboard.press("Escape");

  await expect(renamedMeanwhile(page)).toHaveCount(0);
  await expect
    .poll(() => savedTitles(page))
    .toEqual(["Part One", "Part Two, drafted again"]);
});

test("typing the very title someone else gave meanwhile asks nothing", async ({
  page,
}) => {
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

  await page.keyboard.press("Escape");

  await page.waitForTimeout(300);
  await expect(renamedMeanwhile(page)).toHaveCount(0);
});

test("a move made from an outline that is behind is refused gently, and the outline catches up", async ({
  page,
}) => {
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, () => {});
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineOvertaken",
  );
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
  await titleOf(page, "Part Two").click();

  await page.keyboard.press("Alt+ArrowUp");

  await expect(
    page.getByText("Someone else changed this meanwhile"),
  ).toBeVisible();
  await expect(titleOf(page, "Part Two")).toHaveCount(0);
});

async function anOutlineThatHearsNothing(
  page: Page,
  named: string,
  sections: { title: string; after?: string }[],
) {
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, () => {});
  await aNewProject(page, named);
  const book = await aBookShaped(page, sections);
  await openTheOutline(page);

  return book;
}

test("a move placed after a section removed meanwhile is refused gently", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineThatHearsNothing(
    page,
    "OutlineGoneNeighbour",
    [
      { title: "Part One" },
      { title: "Part Two", after: "Part One" },
      { title: "Part Three", after: "Part Two" },
    ],
  );
  await anotherAuthor(
    page,
    "delete",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
  );
  await titleOf(page, "Part Three").click();

  await page.keyboard.press("Alt+ArrowUp");

  await expect(
    page.getByText("Someone else changed this meanwhile"),
  ).toBeVisible();
  await expect(titleOf(page, "Part One")).toHaveCount(0);
});

test("asked which name stays, the question follows the language chosen meanwhile", async ({
  page,
}) => {
  await bothRenamed(page, "OutlineLiveSpoken");
  await expect(renamedMeanwhile(page)).toBeVisible();

  await page.getByRole("combobox", { name: "Language" }).selectOption("de");

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
  const { outline, ids, reloads, heard } = await anOutlineFollowedLive(
    page,
    "OutlineLiveTakenIn",
  );
  const before = reloads();
  const told = heard();

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  await expect(titleOf(page, "Part Three")).toBeVisible();
  await expect.poll(() => heard()).toBeGreaterThan(told);
  await page.waitForTimeout(500);
  expect(reloads(), "the section was taken in as it came").toBe(before);
});

test("many changes by another author, taken in one by one, leave the outline as the server holds it", async ({
  page,
}) => {
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

  const saved = await savedShape(page, outline);
  await expect.poll(() => shownShape(page)).toEqual(saved);
  await expect.poll(() => heard()).toBeGreaterThanOrEqual(told + 8);
  expect(reloads(), "every change followed on, so none was opened again").toBe(before);
});


test("a change missed on the way opens the outline again, and the outline comes out right", async ({
  page,
}) => {
  let passed = 0;
  let connected = false;
  await page.routeWebSocket(/\/api\/outlines\/live\?outline=/, (socket) => {
    connected = true;
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      passed += 1;
      if (passed !== 2) {
        socket.send(frame);
      }
    });
  });
  await aNewProject(page, "OutlineLiveMissed");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  await openTheOutline(page);
  await expect.poll(() => connected).toBe(true);
  await page.waitForTimeout(300);

  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part One"]}`, {
    title: "The Beginning",
  });
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part One"]}`, {
    title: "The Very Beginning",
  });
  await anotherAuthor(page, "patch", `/outlines/${outline}/sections/${ids["Part Two"]}`, {
    title: "The Middle",
  });

  await expect.poll(() => passed).toBeGreaterThanOrEqual(3);
  await expect(titleOf(page, "The Middle")).toHaveValue("The Middle");
  await expect(
    titleOf(page, "The Very Beginning"),
    "the rename that was missed is there too, so the outline was opened again",
  ).toHaveValue("The Very Beginning");
});
