import { test, expect } from "@playwright/test";

import { corkboard } from "./support/board";
import { aNewProject, openThePool, views } from "./support/shell";

const laidOut = (page: import("@playwright/test").Page) =>
  page.evaluate(() => {
    const box = (of: string) => {
      const found = document.querySelector(of);
      if (!found) return null;
      const seen = found.getBoundingClientRect();
      return {
        left: Math.round(seen.left),
        top: Math.round(seen.top),
        width: Math.round(seen.width),
        height: Math.round(seen.height),
      };
    };

    return {
      window: { width: innerWidth, height: innerHeight },
      scrollHeight: document.documentElement.scrollHeight,
      masthead: box(".masthead"),
      corkboard: box(".corkboard"),
      column: box(".column"),
      tray: box(".tray"),
    };
  });

test("the board fills the window under the masthead", async ({ page }) => {
  await aNewProject(page, "Filling");

  const seen = await laidOut(page);

  expect(seen.masthead!.width).toBe(seen.window.width);
  expect(seen.masthead!.top).toBe(0);
  expect(seen.corkboard!.left).toBe(0);
  expect(seen.corkboard!.top).toBe(seen.masthead!.height);
  expect(seen.corkboard!.width + seen.tray!.width).toBe(seen.window.width);
  expect(seen.scrollHeight).toBe(seen.window.height);
});

test("the pool keeps a narrow column in the middle", async ({ page }) => {
  await aNewProject(page, "Reading");
  await openThePool(page);

  const seen = await laidOut(page);

  expect(seen.corkboard).toBeNull();
  expect(seen.column!.width).toBeLessThan(seen.window.width);
  expect(seen.column!.left).toBeGreaterThan(0);
  expect(seen.column!.left + seen.column!.width).toBe(
    seen.window.width - seen.column!.left,
  );
});

test("the switcher marks the view you are on", async ({ page }) => {
  await aNewProject(page, "Switching");

  await expect(views(page).getByRole("link", { name: "Board" })).toHaveClass(
    /here/,
  );
  await expect(
    views(page).getByRole("link", { name: "Pieces" }),
  ).not.toHaveClass(/here/);

  await openThePool(page);

  await expect(views(page).getByRole("link", { name: "Pieces" })).toHaveClass(
    /here/,
  );
  await expect(
    views(page).getByRole("link", { name: "Board" }),
  ).not.toHaveClass(/here/);
});

test("the masthead centres its lettering, not just its boxes", async ({
  page,
}) => {
  await aNewProject(page, "Centred");

  const off = await page.evaluate(() => {
    const bar = document.querySelector(".masthead")!.getBoundingClientRect();
    const middle = (bar.top + bar.bottom - 1) / 2;

    return [...document.querySelectorAll(".masthead span")].map((lettering) => {
      const seen = lettering.getBoundingClientRect();

      return {
        said: lettering.textContent,
        by: Math.abs((seen.top + seen.bottom) / 2 - middle),
      };
    });
  });

  expect(off.length).toBeGreaterThan(2);
  for (const { said, by } of off) {
    expect(
      by,
      `"${said}" should sit in the middle of the masthead`,
    ).toBeLessThan(1);
  }
});

test("the masthead is there whether or not a project is open", async ({
  page,
}) => {
  await page.goto("/");

  await expect(page.getByRole("banner")).toBeVisible();
  await expect(page.getByRole("link", { name: "All projects" })).toBeVisible();
  await expect(views(page)).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();

  await aNewProject(page, "Everywhere");

  await expect(page.getByRole("banner")).toBeVisible();
  await expect(views(page)).toBeVisible();
});

test("the wordmark leads back to every project", async ({ page }) => {
  await aNewProject(page, "Wordmark");

  await page.getByRole("link", { name: "All projects" }).click();

  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
  await expect(corkboard(page)).toHaveCount(0);
});
