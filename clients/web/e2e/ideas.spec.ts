import { test, expect, type Page } from "@playwright/test";

import { aNewProject, anOpenProject, onTheBoard } from "./support/shell";

const ideas = (page: Page) => page.getByRole("list", { name: "Ideas" });

const nothingYet = (page: Page) =>
  page.getByText("No ideas yet. Shoot an idea in and see where it goes.");

async function captureIdea(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
}

test("opening a project shows its pool of ideas", async ({ page }) => {
  await anOpenProject(page, "Pool");

  await expect(nothingYet(page)).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("a captured idea appears in the pool", async ({ page }) => {
  await anOpenProject(page, "Capture");

  await captureIdea(page, "The loom remembers");

  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await expect(nothingYet(page)).toHaveCount(0);
});

test("the idea field is emptied once the idea is captured", async ({
  page,
}) => {
  await anOpenProject(page, "Emptied");
  const field = page.getByRole("textbox", { name: "What is the idea?" });

  await captureIdea(page, "The loom remembers");

  await expect(field).toHaveValue("");
});

test("an idea captured with no title still appears", async ({ page }) => {
  await anOpenProject(page, "Untitled");

  await page.getByRole("button", { name: "Capture", exact: true }).click();

  await expect(ideas(page).getByText("Untitled")).toBeVisible();
});

test("Enter captures the idea instead of reloading the page", async ({
  page,
}) => {
  await anOpenProject(page, "Enter");

  await page
    .getByRole("textbox", { name: "What is the idea?" })
    .fill("The shuttle");
  await page.getByRole("textbox", { name: "What is the idea?" }).press("Enter");

  await expect(ideas(page).getByText("The shuttle")).toBeVisible();
});

test("several ideas are all kept", async ({ page }) => {
  await anOpenProject(page, "Several");

  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  await captureIdea(page, "She never returned");

  await expect(ideas(page).getByText("She never returned")).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("a reload keeps the project open with its ideas", async ({ page }) => {
  await anOpenProject(page, "Reload");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();

  await page.reload();

  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("ideas of one project do not leak into another", async ({ page }) => {
  await anOpenProject(page, "Mine");
  await captureIdea(page, "Only in mine");
  await expect(ideas(page).getByText("Only in mine")).toBeVisible();

  await anOpenProject(page, "Theirs");

  await expect(ideas(page).getByText("Only in mine")).toHaveCount(0);
  await expect(nothingYet(page)).toBeVisible();
});

test("the pool is hidden until a project is opened", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
});

test("the browser back button leaves the project", async ({ page }) => {
  await aNewProject(page, "Back");

  await page.goBack();

  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
});

test("the browser forward button returns to the project", async ({ page }) => {
  await anOpenProject(page, "Forward");
  await page.goBack();
  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);

  await page.goForward();

  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("all projects returns to the workspace", async ({ page }) => {
  await anOpenProject(page, "AllProjects");

  await page.getByRole("link", { name: "All projects" }).click();

  await expect(page.getByRole("heading", { name: "Ideas" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("a project url opened cold shows that project", async ({ page }) => {
  await anOpenProject(page, "Cold");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  const address = page.url();

  await page.goto(address);

  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("a project can be opened in a new tab", async ({ page, context }) => {
  const title = await aNewProject(page, "NewTab");
  await page.getByRole("link", { name: "All projects" }).click();

  const opened = context.waitForEvent("page");
  await page
    .getByRole("link", { name: title })
    .click({ modifiers: ["ControlOrMeta"] });
  const tab = await opened;

  await expect(tab).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
  await onTheBoard(tab);
  await expect(page).toHaveURL(/\/$/);
  await tab.close();
});

test("an address that leads nowhere says so", async ({ page }) => {
  await page.goto("/nowhere-in-particular");

  await expect(
    page.getByText("There is nothing woven at this address."),
  ).toBeVisible();
  await page.getByRole("link", { name: "Back to your projects" }).click();
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("the address carries a readable slug in front of the id", async ({
  page,
}) => {
  await aNewProject(page, "The Silent Loom");

  const address = new URL(page.url()).pathname;

  expect(address).toMatch(
    /^\/projects\/project-the-silent-loom-[0-9a-f]{8}-project_[0-9A-Za-z]{22}$/,
  );
});

test("a stale slug still reaches the project", async ({ page }) => {
  await anOpenProject(page, "Stale");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  const id = new URL(page.url()).pathname.split("-").pop();

  await page.goto(`/projects/something-else-entirely-${id}`);

  await expect(page.getByRole("heading", { name: "Ideas" })).toBeVisible();
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("an address with no slug at all still reaches the project", async ({
  page,
}) => {
  await anOpenProject(page, "NoSlug");
  await captureIdea(page, "The loom remembers");
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  const id = new URL(page.url()).pathname.split("-").pop();

  await page.goto(`/projects/${id}`);

  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
});

test("clicking an idea opens it", async ({ page }) => {
  await anOpenProject(page, "Writing");
  await captureIdea(page, "The loom remembers");

  await ideas(page).getByRole("link", { name: "The loom remembers" }).click();

  await expect(
    page.getByRole("heading", { name: "The loom remembers" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/ideas\/the-loom-remembers-idea_/);
});

test("an untitled idea can still be opened", async ({ page }) => {
  await anOpenProject(page, "Nameless");
  await page.getByRole("button", { name: "Capture", exact: true }).click();

  await ideas(page).getByRole("link", { name: "Untitled" }).click();

  const name = page.getByRole("textbox", { name: "Idea name" });
  await expect(name).toHaveValue("");
  await expect(name).toHaveAttribute("placeholder", "Untitled");
});
