import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, sceneCatalogued } from "./support/shell";

const projectSegment = (page: Page) =>
  new URL(page.url()).pathname.split("/")[2];

const idIn = (segment: string) => segment.split("-").pop() ?? segment;

const appearances = (page: Page) =>
  page.getByRole("list", { name: "Appears in" });

async function anIdea(page: Page, title: string) {
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: idIn(projectSegment(page)), title },
  });
  expect(captured.status()).toBe(201);

  return (await captured.json()).id as string;
}

async function noteInASection(page: Page, idea: string, section: string) {
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project: idIn(projectSegment(page)) },
  });
  expect(opened.status()).toBe(200);
  const { id: outline } = await opened.json();
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { under: null, after: null, title: section },
  });
  expect(added.status()).toBe(201);
  const attached = await page.request.post(
    `${API}/outlines/${outline}/attachments`,
    {
      data: {
        attachment: { kind: "idea", id: idea },
        section: (await added.json()).section,
        after: null,
      },
    },
  );
  expect(attached.status()).toBe(200);
}

async function linkFromAScene(page: Page, idea: string, titled: string) {
  const made = await page.request.post(`${API}/scenes`, {
    data: { project: idIn(projectSegment(page)) },
  });
  expect(made.status()).toBe(201);
  const { id: scene } = await made.json();
  expect(
    (
      await page.request.patch(`${API}/scenes/${scene}`, {
        data: { title: titled },
      })
    ).status(),
  ).toBe(200);
  expect(
    (
      await page.request.post(`${API}/scenes/${scene}/ideas`, {
        data: { idea },
      })
    ).status(),
  ).toBe(200);
  await sceneCatalogued(page, scene, titled);

  return scene as string;
}

async function inspect(page: Page, idea: string) {
  await page.goto(`/projects/${projectSegment(page)}/ideas/${idea}`);
}

test("an idea's page is headed by its name", async ({ page }) => {
  // given: an idea
  await aNewProject(page, "Inspected");
  const idea = await anIdea(page, "A girl in a wood");

  // when: it is opened
  await inspect(page, idea);

  // then: its page is headed by its name
  await expect(
    page.getByRole("heading", { name: "A girl in a wood" }),
  ).toBeVisible();
});

test("an idea is renamed in place and keeps the name", async ({ page }) => {
  // given: an idea opened
  await aNewProject(page, "Renamed");
  const idea = await anIdea(page, "A girl in a wood");
  await inspect(page, idea);

  // when: it is renamed in place
  await page.getByRole("textbox", { name: "Idea name" }).fill("A girl lost");
  await page.getByRole("textbox", { name: "Idea name" }).press("Enter");
  await expect
    .poll(
      async () =>
        (await (await page.request.get(`${API}/ideas/${idea}`)).json()).title,
    )
    .toBe("A girl lost");
  // then: it keeps the new name after a reload
  await page.reload();

  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl lost",
  );
});

test("an idea in no section and no scene says it is not in the book", async ({
  page,
}) => {
  // given: an idea in no section and no scene
  await aNewProject(page, "Nowhere");
  const idea = await anIdea(page, "A girl in a wood");

  // when: it is opened
  await inspect(page, idea);

  // then: it says it is not in the book
  await expect(page.getByText("Not in the book yet")).toBeVisible();
  await expect(appearances(page)).toHaveCount(0);
});

test("an idea shows the sections that note it and the scenes that draw on it", async ({
  page,
}) => {
  // given: an idea noted in a section and drawn on by a scene
  await aNewProject(page, "Appearing");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  await linkFromAScene(page, idea, "The clearing");

  // when: it is opened
  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("listitem")).toHaveCount(2, {
      timeout: 1_000,
    });
  }).toPass();

  // then: it shows both
  await expect(
    appearances(page).getByRole("link", { name: "Chapter one" }),
  ).toBeVisible();
  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
  ).toBeVisible();
});

test("a scene in the book the idea appears in opens in the writing view", async ({
  page,
}) => {
  // given: an idea opened, drawn on by a scene in the book
  await aNewProject(page, "FollowingIntoTheBook");
  const idea = await anIdea(page, "A girl in a wood");
  const scene = await linkFromAScene(page, idea, "The clearing");
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project: idIn(projectSegment(page)) },
  });
  const outline = (await opened.json()).id;
  const added = await page.request.post(`${API}/outlines/${outline}/sections`, {
    data: { title: "Part One", under: null, after: null },
  });
  await page.request.post(`${API}/outlines/${outline}/attachments`, {
    data: {
      attachment: { kind: "scene", id: scene },
      section: (await added.json()).section,
      after: null,
    },
  });
  await expect(async () => {
    await inspect(page, idea);
    await expect(
      appearances(page).getByRole("link", { name: "The clearing" }),
    ).toBeVisible({ timeout: 1_000 });
  }).toPass();

  // when: the scene is followed
  await appearances(page).getByRole("link", { name: "The clearing" }).click();

  // then: it opens in the writing view, ready to write in
  await expect(page).toHaveURL(/\/write$/);
  await expect(
    page.locator(`[data-scene="${scene}"] .ProseMirror`),
  ).toBeFocused();
});

test("a scene not in the book the idea appears in opens on the writing view's other side", async ({
  page,
}) => {
  // given: an idea opened, drawn on by a scene not in the book
  await aNewProject(page, "Following");
  const idea = await anIdea(page, "A girl in a wood");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(
      appearances(page).getByRole("link", { name: "The clearing" }),
    ).toBeVisible({ timeout: 1_000 });
  }).toPass();

  // when: the scene is followed
  await appearances(page).getByRole("link", { name: "The clearing" }).click();

  // then: it opens among the scenes not in the book, ready to write in
  await expect(page).toHaveURL(/\/write\/loose$/);
  await expect(
    page.locator(`[data-scene="${scene}"] .ProseMirror`),
  ).toBeFocused();
});

const titleOf = async (page: Page, idea: string) =>
  (await (await page.request.get(`${API}/ideas/${idea}`)).json()).title;

async function renameInPlace(page: Page, title: string) {
  const field = page.getByRole("textbox", { name: "Idea name" });
  await field.fill(title);
  await field.press("Enter");
}

test("renames made while one is on its way land as the last, sent once", async ({
  page,
}) => {
  // given: an idea opened
  await aNewProject(page, "RenamedInARow");
  const idea = await anIdea(page, "A girl in a wood");
  await inspect(page, idea);
  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl in a wood",
  );
  // and: its first rename held on its way
  const sent: string[] = [];
  let release = () => {};
  const released = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route(new RegExp(`/api/ideas/${idea}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      return route.fallback();
    }
    sent.push(route.request().postDataJSON().title);
    if (sent.length === 1) {
      await released;
    }
    await route.fallback();
  });

  // when: it is renamed three times in a row
  await renameInPlace(page, "A girl lost");
  await expect.poll(() => sent.length).toBe(1);
  await renameInPlace(page, "A girl found");
  await renameInPlace(page, "A girl at home");
  // and: the first rename is let through
  release();

  // then: the last name lands
  await expect.poll(() => titleOf(page, idea)).toBe("A girl at home");
  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl at home",
  );
  // and: the renames waiting were sent as one
  expect(
    sent,
    "renames made while one is on its way go out as the newest of them",
  ).toEqual(["A girl lost", "A girl at home"]);
});

test("a rename refused because another author renamed meanwhile is made again on theirs", async ({
  page,
}) => {
  // given: an idea opened
  await aNewProject(page, "RenamedMeanwhile");
  const idea = await anIdea(page, "A girl in a wood");
  await inspect(page, idea);
  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl in a wood",
  );
  // and: another author renaming it just before my rename arrives
  const matched: (string | null)[] = [];
  await page.route(new RegExp(`/api/ideas/${idea}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      return route.fallback();
    }
    matched.push(await route.request().headerValue("If-Match"));
    if (matched.length === 1) {
      const theirs = await page.request.patch(`${API}/ideas/${idea}`, {
        data: { title: "A boy in a wood" },
      });
      expect(theirs.ok()).toBe(true);
    }
    await route.fallback();
  });

  // when: it is renamed here
  await renameInPlace(page, "A girl lost");

  // then: my rename lands over theirs
  await expect.poll(() => titleOf(page, idea)).toBe("A girl lost");
  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl lost",
  );
  // and: it was refused once on the version it was made on, then made on theirs
  expect(
    matched,
    "the first rename names the version it was made on and is refused as outdated",
  ).toEqual(['"1"', '"2"']);
});
