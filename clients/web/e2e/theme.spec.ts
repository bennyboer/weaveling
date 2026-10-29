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
  await expect(theming(page)).toBeVisible();

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

for (const [chosen, system] of [
  ["Light", "dark"],
  ["Dark", "light"],
] as const) {
  test(`a ${chosen.toLowerCase()} theme chosen against a ${system} system draws the browser's own controls to match`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme: system });
    await page.goto("/");

    await theming(page).getByRole("button", { name: chosen }).click();

    expect(
      await page.evaluate(
        () => getComputedStyle(document.documentElement).colorScheme,
      ),
    ).toBe(chosen.toLowerCase());
  });
}

test("on a phone, the theme and the language are chosen behind the masthead's More", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 700 });
  await page.goto("/");
  const more = page.getByRole("button", { name: "More", exact: true });

  await expect(more).toBeVisible();
  await expect(
    theming(page),
    "the masthead is drawn by now, so hidden means hidden, not yet missing",
  ).toHaveCount(0);
  await expect(page.getByRole("combobox", { name: "Language" })).toHaveCount(0);

  await more.click();
  const menu = page.getByRole("dialog", { name: "More" });
  await menu.getByRole("button", { name: "Dark" }).click();
  expect(await marked(page)).toBe("dark");
  expect(await kept(page)).toBe("dark");
  await expect(menu.getByRole("button", { name: "Dark" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  await menu.getByRole("combobox", { name: "Language" }).selectOption("de");
  await expect(page.getByRole("button", { name: "Mehr", exact: true })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Mehr" })).toHaveCount(0);
});

test("growing past a phone's width brings the theme and the language back to the masthead", async ({
  page,
}) => {
  await page.setViewportSize({ width: 375, height: 700 });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "More", exact: true })).toBeVisible();

  await page.setViewportSize({ width: 1100, height: 700 });

  await expect(theming(page)).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Language" })).toBeVisible();
  await expect(page.getByRole("button", { name: "More", exact: true })).toHaveCount(0);
});

test("on a wide window, the three themes stand side by side", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1100, height: 700 });
  await page.goto("/");

  await expect(theming(page)).toBeVisible();
  await expect(page.getByRole("button", { name: "More", exact: true })).toHaveCount(0);
});
