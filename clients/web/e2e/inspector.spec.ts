import { test, expect, type Page } from "@playwright/test";

import { aNewProject } from "./support/shell";

const API = "http://127.0.0.1:3000/api";

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

  return scene as string;
}

async function inspect(page: Page, idea: string) {
  await page.goto(`/projects/${projectSegment(page)}/ideas/${idea}`);
}

test("an idea's page is headed by its name", async ({ page }) => {
  await aNewProject(page, "Inspected");
  const idea = await anIdea(page, "A girl in a wood");

  await inspect(page, idea);

  await expect(
    page.getByRole("heading", { name: "A girl in a wood" }),
  ).toBeVisible();
});

test("an idea is renamed in place and keeps the name", async ({ page }) => {
  await aNewProject(page, "Renamed");
  const idea = await anIdea(page, "A girl in a wood");
  await inspect(page, idea);

  await page.getByRole("textbox", { name: "Idea name" }).fill("A girl lost");
  await page.getByRole("textbox", { name: "Idea name" }).press("Enter");
  await expect
    .poll(async () => (await (await page.request.get(`${API}/ideas/${idea}`)).json()).title)
    .toBe("A girl lost");
  await page.reload();

  await expect(page.getByRole("textbox", { name: "Idea name" })).toHaveValue(
    "A girl lost",
  );
});

test("an idea in no section and no scene says it is not in the book", async ({
  page,
}) => {
  await aNewProject(page, "Nowhere");
  const idea = await anIdea(page, "A girl in a wood");

  await inspect(page, idea);

  await expect(page.getByText("Not in the book yet")).toBeVisible();
  await expect(appearances(page)).toHaveCount(0);
});

test("an idea shows the sections that note it and the scenes that draw on it", async ({
  page,
}) => {
  await aNewProject(page, "Appearing");
  const idea = await anIdea(page, "A girl in a wood");
  await noteInASection(page, idea, "Chapter one");
  await linkFromAScene(page, idea, "The clearing");

  await expect(async () => {
    await inspect(page, idea);
    await expect(appearances(page).getByRole("listitem")).toHaveCount(2, {
      timeout: 1_000,
    });
  }).toPass();

  await expect(
    appearances(page).getByRole("link", { name: "Chapter one" }),
  ).toBeVisible();
  await expect(
    appearances(page).getByRole("link", { name: "The clearing" }),
  ).toBeVisible();
});

test("a scene the idea appears in opens from the inspector", async ({
  page,
}) => {
  await aNewProject(page, "Following");
  const idea = await anIdea(page, "A girl in a wood");
  const scene = await linkFromAScene(page, idea, "The clearing");
  await expect(async () => {
    await inspect(page, idea);
    await expect(
      appearances(page).getByRole("link", { name: "The clearing" }),
    ).toBeVisible({ timeout: 1_000 });
  }).toPass();

  await appearances(page).getByRole("link", { name: "The clearing" }).click();

  await expect(page).toHaveURL(new RegExp(`/scenes/${scene}$`));
});
