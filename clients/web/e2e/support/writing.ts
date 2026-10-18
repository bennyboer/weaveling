import { expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./book";
import { aNewProject, API, sceneCatalogued, views } from "./shell";

export const flow = (page: Page) => page.getByRole("article", { name: "The book" });

export const scene = (page: Page, id: string) =>
  flow(page).locator(`[data-scene="${id}"] .ProseMirror`);

export const projectOf = (page: Page) =>
  new URL(page.url()).pathname.split("/")[2].split("-").pop();

export const rail = (page: Page) =>
  page.getByRole("complementary", { name: "Contents" });

export async function openTheWritingView(page: Page) {
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();
}

export async function theBook(page: Page) {
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project: projectOf(page) },
  });
  const outline = await opened.json();

  return outline.sections.map(
    (section: {
      title: string;
      attachments: { kind: string; id: string }[];
    }) => [
      section.title,
      section.attachments
        .filter((attached) => attached.kind === "scene")
        .map((attached) => attached.id),
    ],
  );
}

export async function aBookInTheWritingView(
  page: Page,
  named: string,
  sections: Record<string, number>,
) {
  await aNewProject(page, named);
  const made = await aBookWithScenes(page, sections);
  await openTheWritingView(page);

  return made;
}

export async function projectsOutline(page: Page) {
  const project = projectOf(page);
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });

  return opened.json();
}

export async function cataloguedOpening(page: Page, scene: string) {
  const listed = await page.request.get(
    `${API}/scenes?project=${projectOf(page)}`,
  );
  const { scenes } = await listed.json();

  return (
    scenes.find((held: { id: string }) => held.id === scene)?.opening ?? ""
  );
}

export const savedTitles = async (page: Page) =>
  (await projectsOutline(page)).sections.map(
    (section: { title: string }) => section.title,
  );

export const beginning = (page: Page) =>
  flow(page).getByRole("textbox", { name: "A new scene" });

export const newScene = (page: Page) =>
  flow(page).getByRole("textbox", { name: "A new scene" });

export const heading = (page: Page, named: string) =>
  flow(page).getByRole("heading", { name: named });

export const label = (page: Page, id: string) =>
  flow(page).locator(`.scene-row:has([data-scene="${id}"]) .scene-label`);

export async function titled(page: Page, scene: string, title: string) {
  const renamed = await page.request.patch(`${API}/scenes/${scene}`, {
    data: { title },
  });
  expect(renamed.ok()).toBe(true);
  await sceneCatalogued(page, scene, title);
}

export const sceneActions = (page: Page, id: string) =>
  flow(page)
    .locator(`.scene-row:has([data-scene="${id}"])`)
    .getByRole("button", { name: "Scene actions" });

export const sectionActions = (page: Page, named: string) =>
  flow(page)
    .locator(".heading-row")
    .filter({ has: page.getByRole("heading", { name: named }) })
    .getByRole("button", { name: "Section actions" });

export const menuItem = (page: Page, named: string) =>
  flow(page).getByRole("menuitem", { name: named });

export const asked = (page: Page) => page.getByRole("dialog");

export async function sceneOnServer(page: Page, id: string) {
  return page.request.get(`${API}/scenes/${id}`);
}

export const gapBelow = (page: Page, id: string) =>
  flow(page).locator(`.insertion[data-after="${id}"]`);

export const addHere = (gap: ReturnType<Page["locator"]>) =>
  gap.getByRole("button", { name: "Add here" });

export const bringingBack = (page: Page) =>
  page.getByRole("dialog", { name: "Bring back a scene" });

export const offered = (page: Page) =>
  bringingBack(page)
    .getByRole("list", { name: "Scenes not in the book" })
    .getByRole("button");

export const statusNote = (page: Page) => page.locator(".status-note");

export const backToWhereYouLeftOffButton = (page: Page) =>
  page.getByRole("button", { name: /Back to where you left off/ });

export async function backToWhereILeftOff(page: Page) {
  await backToWhereYouLeftOffButton(page).click();
}

export const saveState = (page: Page) => statusNote(page).locator(".save-state");

export async function holdingWhatIsWritten(page: Page) {
  const held: (string | Buffer)[] = [];
  const servers: { send: (frame: string | Buffer) => void }[] = [];
  let holding = false;

  await page.routeWebSocket(/\/api\/sync$/, (socket) => {
    const server = socket.connectToServer();
    servers.push(server);

    socket.onMessage((frame) =>
      holding ? held.push(frame) : server.send(frame),
    );
  });

  return {
    hold: () => {
      holding = true;
    },
    held: () => held.length,
    release: () => {
      holding = false;
      held.splice(0).forEach((frame) => servers.at(-1)!.send(frame));
    },
  };
}

export async function holding(page: Page, method: string, url: string) {
  let release!: () => void;
  const released = new Promise<void>((done) => (release = done));
  let asked = 0;

  await page.route(url, async (route) => {
    if (route.request().method() !== method) {
      return route.fallback();
    }
    asked += 1;
    await released;
    await route.continue();
  });

  return { asked: () => asked, release };
}

export const wordCount = (page: Page) =>
  statusNote(page).getByRole("button", { name: /words? in/ });

export const sceneTab = (page: Page) =>
  page.getByRole("button", { name: "This scene" });

export const sceneTray = (page: Page) =>
  page.getByRole("complementary", { name: "This scene" });

export const trayTitle = (page: Page) =>
  sceneTray(page).getByRole("textbox", { name: "Scene title" });

export const following = (page: Page, side: string, scene: string) =>
  page.goto(
    page.url().replace(/\/write(\/loose)?(\?.*)?$/, `${side}?scene=${scene}`),
  );

export async function theBookIsIn(page: Page, language: string | null) {
  const changed = await page.request.put(
    `${API}/projects/${projectOf(page)}/details`,
    { data: { language, description: "" } },
  );
  expect(changed.ok()).toBe(true);
  await page.reload();
}

export const shown = (page: Page, id: string) =>
  scene(page, id).evaluate((prose) => {
    const marks = (spoken: Element) =>
      [...getComputedStyle(spoken).quotes.matchAll(/"([^"]*)"/g)].map(
        (found) => found[1],
      );
    const read = (node: Node): string =>
      node instanceof Element && node.matches("q.speech")
        ? `${marks(node)[0]}${[...node.childNodes].map(read).join("")}${marks(node)[1]}`
        : node instanceof Element
          ? [...node.childNodes].map(read).join("")
          : (node.textContent ?? "");

    return [...prose.children].map(read).join("\n");
  });
