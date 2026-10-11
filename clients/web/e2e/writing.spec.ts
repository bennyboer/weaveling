import { test, expect, type Page } from "@playwright/test";

import { aNewProject, views } from "./support/shell";

const API = "http://127.0.0.1:3000/api";

const flow = (page: Page) => page.getByRole("article", { name: "The book" });

async function openTheWritingView(page: Page) {
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  await expect(flow(page)).toBeVisible();
}

async function aBookShaped(
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
}

test("Write sits beside the other views and opens the book", async ({
  page,
}) => {
  await aNewProject(page, "Writing");

  await openTheWritingView(page);

  await expect(page).toHaveURL(/\/projects\/[^/]+\/write$/);
  await expect(
    views(page).getByRole("link", { name: "Write", exact: true }),
  ).toHaveClass(/here/);
});

test("an unshaped book points to the outline", async ({ page }) => {
  await aNewProject(page, "Unshaped");

  await openTheWritingView(page);

  await expect(flow(page)).toContainText("This book has no sections yet.");
  await flow(page).getByRole("link", { name: "the outline" }).click();
  await expect(page).toHaveURL(/\/outline$/);
});

test("the book's sections read in order, nested as headings", async ({
  page,
}) => {
  await aNewProject(page, "Shaped");
  await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
    { title: "The Workshop", under: "Part One" },
    { title: "Upstairs", under: "The Workshop" },
    { title: "The River", under: "Part Two" },
  ]);

  await openTheWritingView(page);

  await expect(flow(page).getByRole("heading")).toHaveText([
    "Part One",
    "The Workshop",
    "Upstairs",
    "Part Two",
    "The River",
  ]);
  await expect(
    flow(page).getByRole("heading", { name: "Part One", level: 2 }),
  ).toBeVisible();
  await expect(
    flow(page).getByRole("heading", { name: "The Workshop", level: 3 }),
  ).toBeVisible();
  await expect(
    flow(page).getByRole("heading", { name: "Upstairs", level: 4 }),
  ).toBeVisible();
});

test("an untitled section still has a heading to find it by", async ({
  page,
}) => {
  await aNewProject(page, "Nameless");
  await aBookShaped(page, [{ title: "" }]);

  await openTheWritingView(page);

  await expect(flow(page).getByRole("heading")).toHaveText([
    "Untitled section",
  ]);
});

test("the book survives a reload of the writing view", async ({ page }) => {
  await aNewProject(page, "Reloaded");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);

  await page.reload();

  await expect(flow(page).getByRole("heading")).toHaveText(["Part One"]);
});
