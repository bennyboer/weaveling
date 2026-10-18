import { expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./book";
import { aNewProject, API, views } from "./shell";

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
