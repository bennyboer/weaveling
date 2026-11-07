import { test, expect, type Page } from "@playwright/test";

import { aNewProject, anOpenProject, catalogued } from "./support/shell";
import { routingLive } from "./support/live";

const ideas = (page: Page) => page.getByRole("list", { name: "Ideas" });

const nothingYet = (page: Page) =>
  page.getByText("No ideas yet. Shoot an idea in and see where it goes.");

async function captureIdea(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
}

test("opening a project shows its pool of ideas", async ({ page }) => {
  // when: a new project is opened
  await anOpenProject(page, "Pool");

  // then: its empty pool of ideas shows
  await expect(nothingYet(page)).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("a captured idea appears in the pool", async ({ page }) => {
  // given: an open project
  await anOpenProject(page, "Capture");

  // when: an idea is captured
  await captureIdea(page, "The loom remembers");

  // then: it is in the pool
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await expect(nothingYet(page)).toHaveCount(0);
});

test("the idea field is emptied once the idea is captured", async ({
  page,
}) => {
  // given: an open project
  await anOpenProject(page, "Emptied");
  const field = page.getByRole("textbox", { name: "What is the idea?" });

  // when: an idea is captured
  await captureIdea(page, "The loom remembers");

  // then: the idea field is empty
  await expect(field).toHaveValue("");
});

test("an idea captured with no title still appears", async ({ page }) => {
  // given: an open project
  await anOpenProject(page, "Untitled");

  // when: an idea is captured with no title
  await page.getByRole("button", { name: "Capture", exact: true }).click();

  // then: it is in the pool, untitled
  await expect(ideas(page).getByText("Untitled")).toBeVisible();
});

test("Enter captures the idea instead of reloading the page", async ({
  page,
}) => {
  // given: an open project
  await anOpenProject(page, "Enter");

  // when: an idea is typed and enter pressed
  await page
    .getByRole("textbox", { name: "What is the idea?" })
    .fill("The shuttle");
  await page.getByRole("textbox", { name: "What is the idea?" }).press("Enter");

  // then: it is captured
  await expect(ideas(page).getByText("The shuttle")).toBeVisible();
});

test("several ideas are all kept", async ({ page }) => {
  // given: an open project
  await anOpenProject(page, "Several");

  // when: two ideas are captured
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await captureIdea(page, "She never returned");

  // then: both are in the pool
  await expect(ideas(page).getByText("She never returned")).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("a reload keeps the project open with its ideas", async ({ page }) => {
  // given: a project open with an idea
  await anOpenProject(page, "Reload");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await catalogued(page, ["The loom remembers"]);

  // when: the page is reloaded
  await page.reload();

  // then: the project is still open with its idea
  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("ideas of one project do not leak into another", async ({ page }) => {
  // given: a project with an idea
  await anOpenProject(page, "Mine");
  await captureIdea(page, "Only in mine");
  await expect(ideas(page).getByText("Only in mine")).toBeVisible();

  // when: another project is opened
  await anOpenProject(page, "Theirs");

  // then: its pool is empty
  await expect(ideas(page).getByText("Only in mine")).toHaveCount(0);
  await expect(nothingYet(page)).toBeVisible();
});

test("the pool is hidden until a project is opened", async ({ page }) => {
  // when: the workspace is opened
  await page.goto("/");

  // then: no pool shows
  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
});

test("the browser back button leaves the project", async ({ page }) => {
  // given: a project opened from the workspace
  const title = await aNewProject(page, "Back");
  await page.getByRole("link", { name: "All projects" }).click();
  await page.getByRole("link", { name: title }).click();
  await expect(page.getByRole("article", { name: "The book" })).toBeVisible();

  // when: the browser goes back
  await page.goBack();

  // then: the workspace shows
  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
});

test("the browser forward button returns to the project", async ({ page }) => {
  // given: a project opened and gone back from
  await anOpenProject(page, "Forward");
  await page.goBack();
  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);

  // when: the browser goes forward
  await page.goForward();

  // then: the project is open again
  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("all projects returns to the workspace", async ({ page }) => {
  // given: an open project
  await anOpenProject(page, "AllProjects");

  // when: all projects is chosen
  await page.getByRole("link", { name: "All projects" }).click();

  // then: the workspace shows
  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("a project url opened cold shows that project", async ({ page }) => {
  // given: a project with an idea
  await anOpenProject(page, "Cold");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await catalogued(page, ["The loom remembers"]);
  const address = page.url();

  // when: its address is opened afresh
  await page.goto(address);

  // then: the project shows with its idea
  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("a project can be opened in a new tab", async ({ page, context }) => {
  // given: a project listed in the workspace
  await routingLive(page, { withheld: ["projects"] });
  const title = await aNewProject(page, "NewTab");
  await page.getByRole("link", { name: "All projects" }).click();

  // when: it is opened in a new tab
  const opened = context.waitForEvent("page");
  await page
    .getByRole("link", { name: title })
    .click({ modifiers: ["ControlOrMeta"] });
  const tab = await opened;

  // then: the tab shows the project
  await expect(tab).toHaveURL(
    /\/projects\/project-[a-z0-9-]+-project_[0-9A-Za-z]{22}\/write$/,
  );
  await expect(tab.getByRole("article", { name: "The book" })).toBeVisible();
  // and: the workspace stays where it was
  await expect(page).toHaveURL(/\/$/);
  await tab.close();
});

test("an address that leads nowhere says so", async ({ page }) => {
  // when: an address that leads nowhere is opened
  await page.goto("/nowhere-in-particular");

  // then: it says so
  await expect(
    page.getByText("There is nothing woven at this address."),
  ).toBeVisible();
  // when: the way back is followed
  await page.getByRole("navigation", { name: "Path" }).getByRole("link", { name: "Your projects" }).click();
  // then: the workspace shows
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("the address carries a readable slug in front of the id", async ({
  page,
}) => {
  // given: a project named in words
  await aNewProject(page, "The Silent Loom");

  // then: its address carries a readable slug before the id
  const address = new URL(page.url()).pathname;

  expect(address).toMatch(
    /^\/projects\/project-the-silent-loom-[0-9a-f]{8}-project_[0-9A-Za-z]{22}\/board$/,
  );
});

test("a stale slug still reaches the project", async ({ page }) => {
  // given: a project with an idea
  await anOpenProject(page, "Stale");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await catalogued(page, ["The loom remembers"]);
  const id = new URL(page.url()).pathname.split("-").pop();

  // when: it is opened with a stale slug
  await page.goto(`/projects/something-else-entirely-${id}`);

  // then: the project shows with its idea
  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("an address with no slug at all still reaches the project", async ({
  page,
}) => {
  // given: a project with an idea
  await anOpenProject(page, "NoSlug");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await catalogued(page, ["The loom remembers"]);
  const id = new URL(page.url()).pathname.split("-").pop();

  // when: it is opened with no slug
  await page.goto(`/projects/${id}`);

  // then: the project shows with its idea
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("clicking an idea opens it", async ({ page }) => {
  // given: a project with an idea
  await anOpenProject(page, "Writing");
  await captureIdea(page, "The loom remembers");

  // when: the idea is clicked
  await ideas(page).getByRole("link", { name: "The loom remembers" }).click();

  // then: it opens
  await expect(
    page.getByRole("heading", { name: "The loom remembers" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/ideas\/the-loom-remembers-idea_/);
});

test("an untitled idea can still be opened", async ({ page }) => {
  // given: an untitled idea
  await anOpenProject(page, "Nameless");
  await page.getByRole("button", { name: "Capture", exact: true }).click();

  // when: it is clicked
  await ideas(page).getByRole("link", { name: "Untitled" }).click();

  // then: it opens, named untitled
  const name = page.getByRole("textbox", { name: "Idea name" });
  await expect(name).toHaveValue("");
  await expect(name).toHaveAttribute("placeholder", "Untitled");
});
