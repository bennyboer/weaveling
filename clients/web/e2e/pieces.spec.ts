import { test, expect, type Page } from "@playwright/test";

const aName = (of: string) => `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

const pieces = (page: Page) => page.getByRole("list", { name: "Pieces" });

const nothingYet = (page: Page) =>
  page.getByText("No pieces yet. Shoot an idea in and see where it goes.");

async function anOpenProject(page: Page, named: string): Promise<string> {
  const title = aName(named);

  await page.goto("/");
  await page.getByPlaceholder("A working title…").fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page.getByRole("link", { name: title }).click();

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();

  return title;
}

async function capture(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
}

test("opening a project shows its pool of pieces", async ({ page }) => {
  await anOpenProject(page, "Pool");

  await expect(nothingYet(page)).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("a captured piece appears in the pool", async ({ page }) => {
  await anOpenProject(page, "Capture");

  await capture(page, "The loom remembers");

  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
  await expect(nothingYet(page)).toHaveCount(0);
});

test("the idea field is emptied once the piece is captured", async ({ page }) => {
  await anOpenProject(page, "Emptied");
  const field = page.getByRole("textbox", { name: "What is the idea?" });

  await capture(page, "The loom remembers");

  await expect(field).toHaveValue("");
});

test("a piece captured with no title still appears", async ({ page }) => {
  await anOpenProject(page, "Untitled");

  await page.getByRole("button", { name: "Capture", exact: true }).click();

  await expect(pieces(page).getByText("Untitled")).toBeVisible();
});

test("Enter captures the piece instead of reloading the page", async ({ page }) => {
  await anOpenProject(page, "Enter");

  await page.getByRole("textbox", { name: "What is the idea?" }).fill("The shuttle");
  await page.getByRole("textbox", { name: "What is the idea?" }).press("Enter");

  await expect(pieces(page).getByText("The shuttle")).toBeVisible();
});

test("several pieces are all kept", async ({ page }) => {
  await anOpenProject(page, "Several");

  await capture(page, "The loom remembers");
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
  await capture(page, "She never returned");

  await expect(pieces(page).getByText("She never returned")).toBeVisible();
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
});

test("a reload keeps the project open with its pieces", async ({ page }) => {
  await anOpenProject(page, "Reload");
  await capture(page, "The loom remembers");
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();

  await page.reload();

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
});

test("pieces of one project do not leak into another", async ({ page }) => {
  await anOpenProject(page, "Mine");
  await capture(page, "Only in mine");
  await expect(pieces(page).getByText("Only in mine")).toBeVisible();

  await anOpenProject(page, "Theirs");

  await expect(pieces(page).getByText("Only in mine")).toHaveCount(0);
  await expect(nothingYet(page)).toBeVisible();
});

test("the pool is hidden until a project is opened", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Pieces" })).toHaveCount(0);
});

test("the browser back button leaves the project", async ({ page }) => {
  await anOpenProject(page, "Back");

  await page.goBack();

  await expect(page.getByRole("heading", { name: "Pieces" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
});

test("the browser forward button returns to the project", async ({ page }) => {
  await anOpenProject(page, "Forward");
  await page.goBack();
  await expect(page.getByRole("heading", { name: "Pieces" })).toHaveCount(0);

  await page.goForward();

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();
  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
});

test("all projects returns to the workspace", async ({ page }) => {
  await anOpenProject(page, "AllProjects");

  await page.getByRole("link", { name: "All projects" }).click();

  await expect(page.getByRole("heading", { name: "Pieces" })).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("a project url opened cold shows that project", async ({ page }) => {
  await anOpenProject(page, "Cold");
  await capture(page, "The loom remembers");
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
  const address = page.url();

  await page.goto(address);

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
});

test("a project can be opened in a new tab", async ({ page, context }) => {
  const title = await anOpenProject(page, "NewTab");
  await page.getByRole("link", { name: "All projects" }).click();

  const opened = context.waitForEvent("page");
  await page.getByRole("link", { name: title }).click({ modifiers: ["ControlOrMeta"] });
  const tab = await opened;

  await expect(tab).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_/);
  await expect(tab.getByRole("heading", { name: "Pieces" })).toBeVisible();
  await expect(page).toHaveURL(/\/$/);
  await tab.close();
});

test("an address that leads nowhere says so", async ({ page }) => {
  await page.goto("/nowhere-in-particular");

  await expect(page.getByText("There is nothing woven at this address.")).toBeVisible();
  await page.getByRole("link", { name: "All projects" }).click();
  await expect(page.getByPlaceholder("A working title…")).toBeVisible();
});

test("the address carries a readable slug in front of the id", async ({ page }) => {
  await anOpenProject(page, "The Silent Loom");

  const address = new URL(page.url()).pathname;

  expect(address).toMatch(/^\/projects\/project-the-silent-loom-[0-9a-f]{8}-project_[0-9A-Za-z]{22}$/);
});

test("a stale slug still reaches the project", async ({ page }) => {
  await anOpenProject(page, "Stale");
  await capture(page, "The loom remembers");
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
  const id = new URL(page.url()).pathname.split("-").pop();

  await page.goto(`/projects/something-else-entirely-${id}`);

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
});

test("an address with no slug at all still reaches the project", async ({ page }) => {
  await anOpenProject(page, "NoSlug");
  await capture(page, "The loom remembers");
  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
  const id = new URL(page.url()).pathname.split("-").pop();

  await page.goto(`/projects/${id}`);

  await expect(pieces(page).getByText("The loom remembers")).toBeVisible();
});

test("clicking a piece opens it for writing", async ({ page }) => {
  await anOpenProject(page, "Writing");
  await capture(page, "The loom remembers");

  await pieces(page).getByRole("link", { name: "The loom remembers" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page.getByText("Synced")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
});

test("prose written in a piece survives a reload", async ({ page }) => {
  await anOpenProject(page, "Survives");
  await capture(page, "The loom remembers");
  await pieces(page).getByRole("link", { name: "The loom remembers" }).click();
  await expect(page.getByText("Synced")).toBeVisible();

  await page.locator(".surface .ProseMirror").click();
  await page.keyboard.type("She had not touched it since spring.");
  await expect(page.locator(".surface .ProseMirror")).toContainText("since spring");

  await page.reload();

  await expect(page.locator(".surface .ProseMirror")).toContainText("since spring");
});

test("a piece that has been opened is marked in the pool", async ({ page }) => {
  await anOpenProject(page, "Marked");
  await capture(page, "The loom remembers");
  await pieces(page).getByRole("link", { name: "The loom remembers" }).click();
  await expect(page.getByText("Synced")).toBeVisible();

  await page.getByRole("link", { name: "Back to the pool" }).click();

  await expect(pieces(page).getByRole("img", { name: "Opened for writing" })).toBeVisible();
});

test("opening a piece twice keeps the same prose", async ({ page }) => {
  await anOpenProject(page, "Twice");
  await capture(page, "The loom remembers");
  await pieces(page).getByRole("link", { name: "The loom remembers" }).click();
  await expect(page.getByText("Synced")).toBeVisible();
  await page.locator(".surface .ProseMirror").click();
  await page.keyboard.type("Written once.");
  await expect(page.locator(".surface .ProseMirror")).toContainText("Written once.");

  await page.getByRole("link", { name: "Back to the pool" }).click();
  await pieces(page).getByRole("link", { name: "The loom remembers" }).click();

  await expect(page.locator(".surface .ProseMirror")).toContainText("Written once.");
});

test("an untitled piece can still be opened for writing", async ({ page }) => {
  await anOpenProject(page, "Nameless");
  await page.getByRole("button", { name: "Capture", exact: true }).click();

  await pieces(page).getByRole("link", { name: "Untitled" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
});
