import { test, expect } from "@playwright/test";

import { corkboard } from "./support/board";
import { aNewProject, API, captureIdea, openTheOutline, openThePool, views } from "./support/shell";
import { projectOf } from "./support/writing";

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
  // given: a project open on its board
  await aNewProject(page, "Filling");

  // when: the page is measured
  const seen = await laidOut(page);

  // then: the board fills the window between the masthead and the status bar
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
  // expect: the status bar runs along the bottom of the projects, the board, the outline and the ideas
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
  await expect(page.getByRole("heading", { name: "Your projects" })).toBeVisible();
  await atTheBottom();
  await aNewProject(page, "Footing");
  await atTheBottom();
  await openTheOutline(page);
  await atTheBottom();
  await openThePool(page);
  await atTheBottom();
});

test("the status bar never hides the end of a long page", async ({ page }) => {
  // given: a short window
  await page.setViewportSize({ width: 1000, height: 420 });
  // and: a page of many ideas
  await aNewProject(page, "Long");
  await openThePool(page);
  for (const nth of [1, 2, 3, 4, 5, 6, 7, 8]) {
    await captureIdea(page, `Thread ${nth}`);
  }

  // then: the status bar is in sight
  await expect(
    statusBar(page),
    "a long page must not push the bar out of sight",
  ).toBeInViewport();

  // when: the page is scrolled to its end
  await page.mouse.wheel(0, 5_000);

  // then: the status bar is still in sight
  const content = page.locator("main");
  await expect(statusBar(page)).toBeInViewport();
  // and: the page ends above it
  const contentSeen = (await content.boundingBox())!;
  const barSeen = (await statusBar(page).boundingBox())!;
  expect(
    contentSeen.y + contentSeen.height,
    "scrolled to the end, the page must end above the bar rather than under it",
  ).toBeLessThanOrEqual(barSeen.y + 1);
});

test("the pool keeps a narrow column in the middle", async ({ page }) => {
  // given: a project open on its ideas
  await aNewProject(page, "Reading");
  await openThePool(page);

  // when: the page is measured
  const seen = await laidOut(page);

  // then: the ideas sit in a narrow column in the middle
  expect(seen.corkboard).toBeNull();
  expect(seen.column!.width).toBeLessThan(seen.window.width);
  expect(seen.column!.left).toBeGreaterThan(0);
  expect(seen.column!.left + seen.column!.width).toBe(
    seen.window.width - seen.column!.left,
  );
});

test("the switcher marks the view you are on", async ({ page }) => {
  // given: a project open on its board
  await aNewProject(page, "Switching");

  // then: the switcher marks the board
  await expect(views(page).getByRole("link", { name: "Board" })).toHaveClass(
    /here/,
  );
  await expect(
    views(page).getByRole("link", { name: "Ideas" }),
  ).not.toHaveClass(/here/);

  // when: the ideas are opened
  await openThePool(page);

  // then: the switcher marks the ideas
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
  // given: a project open
  await aNewProject(page, "Centred");

  // when: the masthead's lettering is measured
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

  // then: each sits in its middle
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
  // given: no project open
  await page.goto("/");

  // then: the masthead is there
  await expect(page.getByRole("banner")).toBeVisible();
  await expect(page.getByRole("link", { name: "All projects" })).toBeVisible();
  await expect(views(page)).toHaveCount(0);
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();

  // when: a project is opened
  await aNewProject(page, "Everywhere");

  // then: the masthead is there too
  await expect(page.getByRole("banner")).toBeVisible();
  await expect(views(page)).toBeVisible();
});

test("the wordmark leads back to every project", async ({ page }) => {
  // given: a project open
  await aNewProject(page, "Wordmark");

  // when: the wordmark is clicked
  await page.getByRole("link", { name: "All projects" }).click();

  // then: every project is shown
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
  await expect(corkboard(page)).toHaveCount(0);
});

for (const width of [375, 700, 900, 1100]) {
  test(`on a window ${width}px wide, the masthead fits without scrolling sideways`, async ({
    page,
  }) => {
    // given: a window of the given width
    await page.setViewportSize({ width, height: 700 });
    // and: a long-named project open in the writing view
    const title = await aNewProject(page, "A rather long name for a phone");
    await views(page).getByRole("link", { name: "Write", exact: true }).click();
    await expect(page.getByRole("article", { name: "The book" })).toBeVisible();

    // when: the masthead is measured
    const seen = await page.evaluate(() => {
      const box = (of: string) =>
        document.querySelector(of)!.getBoundingClientRect();
      const inside = (of: string) => {
        if (!document.querySelector(of)) {
          return false;
        }
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
        theming: inside(".theming") || inside(".more-button"),
        speaking: inside(".speaking") || inside(".more-button"),
      };
    });

    // then: nothing reaches past the window
    expect(seen.scrolled, "nothing reaches past the window").toBe(0);
    // and: the name, details, views, theme and language all stay in reach
    expect(seen.logo).toBe(true);
    expect(seen.name, "the name keeps room to be read").toBeGreaterThan(90);
    expect(seen.details).toBe(true);
    expect(seen.views, "every view can be reached").toBe(true);
    expect(seen.theming, "the theme can be chosen").toBe(true);
    expect(seen.speaking, "the language can be chosen").toBe(true);
    // and: the full name is kept for the reader
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
    // given: a window of the given width
    await page.setViewportSize({ width, height: 700 });
    // and: a project with a short name
    await aNewProject(page, "x");
    await page.request.patch(
      `${API}/projects/${projectOf(page)}`,
      { data: { name: "Kurz" } },
    );
    await page.reload();
    await expect(page.getByRole("heading", { level: 1 })).toHaveText("Kurz");

    // when: the masthead is measured
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

    // then: the name sits right beside its details button
    expect(seen.gap).toBeLessThanOrEqual(16);
    // and: on a narrow window the views run the full width
    if (width < 900) {
      expect(seen.viewsFrom, "the views' row runs the full width").toBe(0);
      expect(seen.viewsTo, "the views' row runs the full width").toBe(0);
    }
  });
}

test("the writing view comes first among a project's views", async ({
  page,
}) => {
  // given: a project open
  await aNewProject(page, "Order");

  // then: the writing view comes first among its views
  await expect(views(page).getByRole("link")).toHaveText([
    "Write",
    "Board",
    "Outline",
    "Ideas",
  ]);
});
