import { test, expect } from "@playwright/test";

import { corkboard } from "./support/board";
import {
  aNewProject,
  captureIdea,
  openTheOutline,
  openThePool,
  views,
} from "./support/shell";

const statusBar = (page: import("@playwright/test").Page) =>
  page.getByRole("contentinfo", { name: "Status" });

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
      statusBar: box(".status-bar"),
      corkboard: box(".corkboard"),
      column: box(".column"),
      tray: box(".tray"),
    };
  });

test("the board fills the window between the masthead and the status bar", async ({
  page,
}) => {
  await aNewProject(page, "Filling");

  const seen = await laidOut(page);

  expect(seen.masthead!.width).toBe(seen.window.width);
  expect(seen.masthead!.top).toBe(0);
  expect(seen.corkboard!.left).toBe(0);
  expect(seen.corkboard!.top).toBe(seen.masthead!.height);
  expect(seen.corkboard!.top + seen.corkboard!.height).toBe(
    seen.statusBar!.top,
  );
  expect(seen.corkboard!.width + seen.tray!.width).toBe(seen.window.width);
  expect(seen.scrollHeight).toBe(seen.window.height);
});

test("the status bar sits along the bottom of every view", async ({ page }) => {
  const atTheBottom = async () => {
    await expect(statusBar(page)).toBeVisible();
    const seen = await laidOut(page);
    expect(seen.statusBar!.left).toBe(0);
    expect(seen.statusBar!.width).toBe(seen.window.width);
    expect(seen.statusBar!.top + seen.statusBar!.height).toBe(
      seen.window.height,
    );
  };

  await page.goto("/");
  await atTheBottom();
  await aNewProject(page, "Footing");
  await atTheBottom();
  await openTheOutline(page);
  await atTheBottom();
  await openThePool(page);
  await atTheBottom();
});

test("the status bar never hides the end of a long page", async ({ page }) => {
  await page.setViewportSize({ width: 1000, height: 420 });
  await aNewProject(page, "Long");
  await openThePool(page);
  for (const nth of [1, 2, 3, 4, 5, 6, 7, 8]) {
    await captureIdea(page, `Thread ${nth}`);
  }

  await expect(
    statusBar(page),
    "a long page must not push the bar out of sight",
  ).toBeInViewport();

  await page.mouse.wheel(0, 5_000);

  const content = page.locator("main");
  await expect(statusBar(page)).toBeInViewport();
  const contentSeen = (await content.boundingBox())!;
  const barSeen = (await statusBar(page).boundingBox())!;
  expect(
    contentSeen.y + contentSeen.height,
    "scrolled to the end, the page must end above the bar rather than under it",
  ).toBeLessThanOrEqual(barSeen.y + 1);
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
    views(page).getByRole("link", { name: "Ideas" }),
  ).not.toHaveClass(/here/);

  await openThePool(page);

  await expect(views(page).getByRole("link", { name: "Ideas" })).toHaveClass(
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

for (const width of [375, 700, 900, 1100]) {
  test(`on a window ${width}px wide, the masthead fits without scrolling sideways`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 700 });
    const title = await aNewProject(page, "A rather long name for a phone");
    await views(page).getByRole("link", { name: "Write", exact: true }).click();
    await expect(page.getByRole("article", { name: "The book" })).toBeVisible();

    const seen = await page.evaluate(() => {
      const box = (of: string) =>
        document.querySelector(of)!.getBoundingClientRect();
      const inside = (of: string) => {
        const seen = box(of);
        return seen.width > 0 && seen.left >= 0 && seen.right <= innerWidth;
      };

      return {
        scrolled: document.documentElement.scrollWidth - innerWidth,
        logo: inside(".wordmark .mark"),
        name: box(".whose span").width,
        details: inside(".project-details-button"),
        views: [...document.querySelectorAll(".views .view")].every((view) =>
          inside(`.views a[href="${view.getAttribute("href")}"]`),
        ),
        theming: inside(".theming"),
      };
    });

    expect(seen.scrolled, "nothing reaches past the window").toBe(0);
    expect(seen.logo).toBe(true);
    expect(seen.name, "the name keeps room to be read").toBeGreaterThan(90);
    expect(seen.details).toBe(true);
    expect(seen.views, "every view can be reached").toBe(true);
    expect(seen.theming).toBe(true);
    await expect(page.getByRole("heading", { level: 1 })).toHaveAttribute(
      "title",
      title,
    );
  });
}

for (const width of [375, 1440]) {
  test(`on a window ${width}px wide, a short name sits right beside its details button`, async ({
    page,
  }) => {
    await page.setViewportSize({ width, height: 700 });
    await aNewProject(page, "x");
    await page.request.patch(
      `http://127.0.0.1:3000/api/projects/${new URL(page.url()).pathname.split("/")[2].split("-").pop()}`,
      { data: { name: "Kurz" } },
    );
    await page.reload();
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Kurz");

    const seen = await page.evaluate(() => {
      const name = document.createRange();
      name.selectNodeContents(document.querySelector(".whose span")!);
      const views = document.querySelector(".views")!.getBoundingClientRect();
      const bar = document.querySelector(".masthead")!.getBoundingClientRect();

      return {
        gap:
          document
            .querySelector(".project-details-button svg")!
            .getBoundingClientRect().left - name.getBoundingClientRect().right,
        viewsFrom: views.left - bar.left,
        viewsTo: bar.right - views.right,
      };
    });

    expect(seen.gap).toBeLessThanOrEqual(16);
    if (width < 900) {
      expect(seen.viewsFrom, "the views' row runs the full width").toBe(0);
      expect(seen.viewsTo, "the views' row runs the full width").toBe(0);
    }
  });
}
