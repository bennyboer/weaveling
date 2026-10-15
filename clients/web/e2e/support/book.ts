import { expect, type Page } from "@playwright/test";

import { aSceneMade, scenesCatalogued } from "./shell";

const API = "http://127.0.0.1:3000/api";

export async function aBookShaped(
  page: Page,
  sections: { title: string; under?: string; after?: string }[],
) {
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const opened = await page.request.post(`${API}/outlines`, {
    data: { project },
  });
  const outline = (await opened.json()).id;
  const ids: Record<string, string> = {};

  for (const { title, under, after } of sections) {
    const added = await page.request.post(
      `${API}/outlines/${outline}/sections`,
      {
        data: {
          title,
          under: under ? ids[under] : null,
          after: after ? ids[after] : null,
        },
      },
    );
    expect(added.status(), await added.text()).toBe(201);
    ids[title] = (await added.json()).section;
  }

  return { outline, ids };
}

export async function aBookWithScenes(
  page: Page,
  sections: Record<string, number>,
) {
  const titles = Object.keys(sections);
  const { outline, ids } = await aBookShaped(
    page,
    titles.map((title, nth) => ({ title, after: titles[nth - 1] })),
  );
  const scenes: Record<string, string[]> = {};

  for (const title of titles) {
    scenes[title] = [];
    for (let nth = 0; nth < sections[title]; nth++) {
      const scene = await aSceneMade(page);
      const behind = scenes[title].at(-1);
      const attached = await page.request.post(
        `${API}/outlines/${outline}/attachments`,
        {
          data: {
            attachment: { kind: "scene", id: scene },
            section: ids[title],
            after: behind ? { kind: "scene", id: behind } : null,
          },
        },
      );
      expect(attached.ok(), await attached.text()).toBe(true);
      scenes[title].push(scene);
    }
  }

  await scenesCatalogued(page, Object.values(scenes).flat());

  return scenes;
}
