import { test, expect, type Page } from "@playwright/test";

const aTitle = () => `The Loom ${crypto.randomUUID().slice(0, 8)}`;

const titleField = (page: Page) => page.getByPlaceholder("A working title…");

const rowFor = (page: Page, title: string) =>
  page.getByRole("listitem").filter({ hasText: title });

async function aProjectCalled(page: Page, title: string) {
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create" }).click();
  await expect(rowFor(page, title)).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test("a project can be created and appears in the list", async ({ page }) => {
  const title = aTitle();

  await aProjectCalled(page, title);

  await expect(rowFor(page, title)).toBeVisible();
});

test("Enter creates the project instead of reloading the page", async ({ page }) => {
  const title = aTitle();

  await titleField(page).fill(title);
  await titleField(page).press("Enter");

  await expect(rowFor(page, title)).toBeVisible();
});

test("the title field is emptied once the project is created", async ({ page }) => {
  const title = aTitle();

  await aProjectCalled(page, title);

  await expect(titleField(page)).toHaveValue("");
});

test("a name the server rejects leaves the problem on screen", async ({ page }) => {
  await titleField(page).fill("a".repeat(201));
  await page.getByRole("button", { name: "Create" }).click();

  await expect(page.getByRole("alert")).toContainText("at most 200 characters");
});

test("a blank title cannot be submitted", async ({ page }) => {
  await titleField(page).fill("   ");

  await expect(page.getByRole("button", { name: "Create" })).toBeDisabled();
});

test("a project can be renamed", async ({ page }) => {
  const title = aTitle();
  const rewoven = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;
  await aProjectCalled(page, title);

  await rowFor(page, title).getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(rewoven);
  await page.getByRole("button", { name: "Save" }).click();

  await expect(rowFor(page, rewoven)).toBeVisible();
  await expect(rowFor(page, title)).toHaveCount(0);
});

test("deleting asks first, and cancelling keeps the project", async ({ page }) => {
  const title = aTitle();
  await aProjectCalled(page, title);

  await rowFor(page, title).getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await expect(page.getByRole("dialog")).toContainText("cannot be undone");
  await page.getByRole("button", { name: "Cancel" }).click();

  await expect(rowFor(page, title)).toBeVisible();
});

test("confirming a delete removes the project", async ({ page }) => {
  const title = aTitle();
  await aProjectCalled(page, title);

  await rowFor(page, title).getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Delete" }).click();

  await expect(rowFor(page, title)).toHaveCount(0);
});
