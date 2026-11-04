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
  // given: a dark system
  await page.emulateMedia({ colorScheme: "dark" });
  // when: the app opens
  await page.goto("/");

  // then: no theme is chosen
  expect(await marked(page)).toBeNull();
  // and: following the system is shown
  await expect(
    theming(page).getByRole("button", { name: "Follow the system" }),
  ).toHaveAttribute("aria-pressed", "true");
  // and: the app is painted dark
  const inTheDark = await painted(page);

  // when: the system turns light
  await page.emulateMedia({ colorScheme: "light" });

  // then: the app is painted otherwise
  expect(await painted(page)).not.toBe(inTheDark);
});

test("a chosen theme overrides the system and survives a reload", async ({
  page,
}) => {
  // given: the app open on a dark system
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  const inTheDark = await painted(page);

  // when: the light theme is chosen
  await theming(page).getByRole("button", { name: "Light" }).click();

  // then: it is marked and kept
  expect(await marked(page)).toBe("light");
  expect(await kept(page)).toBe("light");
  // and: the app is painted otherwise
  const chosen = await painted(page);
  expect(chosen).not.toBe(inTheDark);

  // when: the page is reloaded
  await page.reload();

  // then: the light theme is still marked, painted and shown chosen
  expect(await marked(page)).toBe("light");
  expect(await painted(page)).toBe(chosen);
  await expect(
    theming(page).getByRole("button", { name: "Light" }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("the stored theme is applied before the app boots", async ({ page }) => {
  // given: the light theme chosen on a dark system
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/");
  await theming(page).getByRole("button", { name: "Light" }).click();

  // when: the page begins to load again
  await page.goto("/", { waitUntil: "commit" });

  // then: the light theme is marked before the app runs
  expect(
    await marked(page),
    "the root should be marked before any Rust runs, or the page flashes",
  ).toBe("light");
});

test("following the system again lets go of the choice", async ({ page }) => {
  // given: the dark theme chosen
  await page.goto("/");
  await theming(page).getByRole("button", { name: "Dark" }).click();
  expect(await marked(page)).toBe("dark");

  // when: following the system is chosen
  await theming(page)
    .getByRole("button", { name: "Follow the system" })
    .click();

  // then: no theme is marked
  expect(await marked(page)).toBeNull();
  // and: following the system is kept
  expect(await kept(page)).toBe("system");
});

test("the theme control is a control, not a full-height panel", async ({
  page,
}) => {
  // given: the app open
  await page.goto("/");
  await expect(theming(page)).toBeVisible();

  // when: the theme control and the masthead are measured
  const seen = await page.evaluate(() => {
    const box = (of: string) => {
      const found = document.querySelector(of)!.getBoundingClientRect();
      return { top: found.top, bottom: found.bottom, height: found.height };
    };

    return { bar: box(".masthead"), theming: box(".theming") };
  });

  // then: the control is shorter than the masthead
  expect(seen.theming.height).toBeLessThan(seen.bar.height - 12);
  // and: sits in its middle
  expect(
    Math.abs(
      (seen.theming.top + seen.theming.bottom) / 2 -
        (seen.bar.top + seen.bar.bottom - 1) / 2,
    ),
  ).toBeLessThan(1);
});

test("the theme can be set from anywhere in a project", async ({ page }) => {
  // given: a project open
  await aNewProject(page, "Theming");

  // then: the theme can be chosen
  await expect(theming(page)).toBeVisible();

  // when: the dark theme is chosen
  await theming(page).getByRole("button", { name: "Dark" }).click();

  // then: it is marked
  expect(await marked(page)).toBe("dark");
});

for (const [chosen, system] of [
  ["Light", "dark"],
  ["Dark", "light"],
] as const) {
  test(`a ${chosen.toLowerCase()} theme chosen against a ${system} system draws the browser's own controls to match`, async ({
    page,
  }) => {
    // given: the app open on a system of the other colour
    await page.emulateMedia({ colorScheme: system });
    await page.goto("/");

    // when: the theme is chosen
    await theming(page).getByRole("button", { name: chosen }).click();

    // then: the browser's own controls are drawn to match it
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
  // given: the app open on a phone
  await page.setViewportSize({ width: 375, height: 700 });
  await page.goto("/");
  const more = page.getByRole("button", { name: "More", exact: true });

  // then: more is offered
  await expect(more).toBeVisible();
  // and: the theme and the language are not in the masthead
  await expect(
    theming(page),
    "the masthead is drawn by now, so hidden means hidden, not yet missing",
  ).toHaveCount(0);
  await expect(page.getByRole("combobox", { name: "Language" })).toHaveCount(0);

  // when: the dark theme is chosen behind more
  await more.click();
  const menu = page.getByRole("dialog", { name: "More" });
  await menu.getByRole("button", { name: "Dark" }).click();
  // then: it is marked, kept and shown chosen
  expect(await marked(page)).toBe("dark");
  expect(await kept(page)).toBe("dark");
  await expect(menu.getByRole("button", { name: "Dark" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );

  // when: german is chosen behind more
  await menu.getByRole("combobox", { name: "Language" }).selectOption("de");
  // then: the app speaks german
  await expect(page.getByRole("button", { name: "Mehr", exact: true })).toBeVisible();

  // when: Escape is pressed
  await page.keyboard.press("Escape");
  // then: more closes
  await expect(page.getByRole("dialog", { name: "Mehr" })).toHaveCount(0);
});

test("growing past a phone's width brings the theme and the language back to the masthead", async ({
  page,
}) => {
  // given: the app open on a phone
  await page.setViewportSize({ width: 375, height: 700 });
  await page.goto("/");
  await expect(page.getByRole("button", { name: "More", exact: true })).toBeVisible();

  // when: the window grows wider
  await page.setViewportSize({ width: 1100, height: 700 });

  // then: the theme and the language are back in the masthead
  await expect(theming(page)).toBeVisible();
  await expect(page.getByRole("combobox", { name: "Language" })).toBeVisible();
  // and: more is gone
  await expect(page.getByRole("button", { name: "More", exact: true })).toHaveCount(0);
});

test("on a wide window, the three themes stand side by side", async ({
  page,
}) => {
  // given: a wide window
  await page.setViewportSize({ width: 1100, height: 700 });
  // when: the app opens
  await page.goto("/");

  // then: the themes stand in the masthead
  await expect(theming(page)).toBeVisible();
  // and: more is not offered
  await expect(page.getByRole("button", { name: "More", exact: true })).toHaveCount(0);
});
