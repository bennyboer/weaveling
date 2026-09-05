import { test, expect, type Page } from "@playwright/test";

import { aNewProject } from "./support/shell";

const theming = (page: Page) => page.getByRole("group", { name: "Theme" });

const marked = (page: Page) =>
  page.evaluate(() => document.documentElement.getAttribute("data-theme"));

const kept = (page: Page) =>
  page.evaluate(() => localStorage.getItem("weaveling.theme"));

const painted = (page: Page) =>
  page.evaluate(() => getComputedStyle(document.body).backgroundColor);

test("the system decides until the author disagrees", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");

  expect(await marked(page)).toBeNull();
  await expect(
    theming(page).getByRole("button", { name: "Follow the system" }),
  ).toHaveAttribute("aria-pressed", "true");
  const inTheDark = await painted(page);

  await page.emulateMedia({ colorScheme: "light" });

  expect(await painted(page)).not.toBe(inTheDark);
});

test("a chosen theme overrides the system and survives a reload", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  const inTheDark = await painted(page);

  await theming(page).getByRole("button", { name: "Light" }).click();

  expect(await marked(page)).toBe("light");
  expect(await kept(page)).toBe("light");
  const chosen = await painted(page);
  expect(chosen).not.toBe(inTheDark);

  await page.reload();

  expect(await marked(page)).toBe("light");
  expect(await painted(page)).toBe(chosen);
  await expect(
    theming(page).getByRole("button", { name: "Light" }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("the stored theme is applied before the app boots", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await theming(page).getByRole("button", { name: "Light" }).click();

  await page.goto("/", { waitUntil: "commit" });

  expect(
    await marked(page),
    "the root should be marked before any Rust runs, or the page flashes",
  ).toBe("light");
});

test("following the system again lets go of the choice", async ({ page }) => {
  await page.goto("/");
  await theming(page).getByRole("button", { name: "Dark" }).click();
  expect(await marked(page)).toBe("dark");

  await theming(page)
    .getByRole("button", { name: "Follow the system" })
    .click();

  expect(await marked(page)).toBeNull();
  expect(await kept(page)).toBe("system");
});

test("the theme control is a control, not a full-height panel", async ({
  page,
}) => {
  await page.goto("/");

  const seen = await page.evaluate(() => {
    const box = (of: string) => {
      const found = document.querySelector(of)!.getBoundingClientRect();
      return { top: found.top, bottom: found.bottom, height: found.height };
    };

    return { bar: box(".masthead"), theming: box(".theming") };
  });

  expect(seen.theming.height).toBeLessThan(seen.bar.height - 12);
  expect(
    Math.abs(
      (seen.theming.top + seen.theming.bottom) / 2 -
        (seen.bar.top + seen.bar.bottom - 1) / 2,
    ),
  ).toBeLessThan(1);
});

test("the theme can be set from anywhere in a project", async ({ page }) => {
  await aNewProject(page, "Theming");

  await expect(theming(page)).toBeVisible();

  await theming(page).getByRole("button", { name: "Dark" }).click();

  expect(await marked(page)).toBe("dark");
});
