import { expect, type Page } from "@playwright/test";

export const API = "http://127.0.0.1:3000/api";

const aName = (of: string) =>
  `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

export const views = (page: Page) =>
  page.getByRole("navigation", { name: "Views" });

const slugOf = (title: string) =>
  title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

export async function aNewProject(page: Page, named: string): Promise<string> {
  const title = aName(named);
  const made = await page.request.post(`${API}/projects`, {
    data: { name: title, language: "en" },
  });
  expect(made.ok()).toBe(true);
  const { id } = await made.json();
  await expect
    .poll(async () => {
      const listed = await (await page.request.get(`${API}/projects`)).json();
      return listed.some((project: { id: string }) => project.id === id);
    })
    .toBe(true);

  await page.goto(`/projects/${slugOf(title)}-${id}/board`);
  await onTheBoard(page);

  return title;
}

export async function anOpenProject(
  page: Page,
  named: string,
): Promise<string> {
  const title = await aNewProject(page, named);

  await openThePool(page);

  return title;
}

export async function captureIdea(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "Ideas" }).getByText(idea),
  ).toBeVisible();
  await catalogued(page, [idea]);
}

export async function onTheBoard(page: Page) {
  await expect(page.getByRole("region", { name: "Board" })).toBeVisible();
}

export async function openTheBoard(page: Page) {
  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await onTheBoard(page);
}

export async function openTheOutline(page: Page) {
  await views(page).getByRole("link", { name: "Outline", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Add a section" }),
  ).toBeVisible();
}

export async function openThePool(page: Page) {
  await views(page).getByRole("link", { name: "Ideas", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Ideas", exact: true }),
  ).toBeVisible();
}

export async function aSceneMade(page: Page): Promise<string> {
  const project = new URL(page.url()).pathname.split("/")[2];
  const made = await page.request.post(`${API}/scenes`, {
    data: { project: project.split("-").pop() },
  });
  expect(made.status()).toBe(201);

  return (await made.json()).id;
}

export async function aLooseScene(page: Page): Promise<string> {
  const id = await aSceneMade(page);
  await sceneCatalogued(page, id);

  return id;
}

async function cataloguedScenes(
  page: Page,
): Promise<{ id: string; title: string }[]> {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const listed = await page.request.get(
    `${API}/scenes?project=${project}`,
  );

  return (await listed.json()).scenes;
}

async function cataloguedScene(page: Page, scene: string) {
  return (await cataloguedScenes(page)).find((held) => held.id === scene);
}

export async function scenesCatalogued(page: Page, scenes: string[]) {
  await expect
    .poll(
      async () => {
        const listed = new Set(
          (await cataloguedScenes(page)).map((held) => held.id),
        );

        return scenes.filter((scene) => !listed.has(scene));
      },
      {
        message:
          "the scenes listing is a projection, so a fresh page can only show what it has caught up with",
      },
    )
    .toEqual([]);
}

export async function sceneCatalogued(
  page: Page,
  scene: string,
  title?: string,
) {
  await expect
    .poll(async () => (await cataloguedScene(page, scene))?.title, {
      message:
        "the scenes listing is a projection, so a fresh page can only show what it has caught up with",
    })
    .toBe(title ?? "");
}

async function cataloguedTitles(page: Page): Promise<string[]> {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const listed = await page.request.get(
    `${API}/ideas?project=${project}`,
  );

  return (await listed.json()).map((idea: { title: string }) => idea.title);
}

export async function catalogued(page: Page, titles: string[]) {
  await expect
    .poll(() => cataloguedTitles(page), {
      message:
        "the ideas catalog is a projection, so a fresh page can only show what it has caught up with",
    })
    .toEqual(expect.arrayContaining(titles));
}

export async function uncatalogued(page: Page, titles: string[]) {
  await expect
    .poll(
      async () =>
        (await cataloguedTitles(page)).filter((title) =>
          titles.includes(title),
        ),
      {
        message:
          "the ideas catalog is a projection, so a discard leaves a fresh page only once it has caught up",
      },
    )
    .toEqual([]);
}
