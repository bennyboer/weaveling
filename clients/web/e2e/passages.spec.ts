import { test, expect, type Page } from "@playwright/test";

const API = "http://127.0.0.1:3000/api";

const surface = (page: Page) => page.locator(".surface .ProseMirror");

async function anOpenPassage(page: Page): Promise<string> {
  await page.goto("/");
  const created = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/passages") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Start writing" }).click();
  const response = await created;
  const { id } = await response.json();

  await expect(surface(page)).toBeVisible();
  await expect(page.getByText("Synced")).toBeVisible();

  return id;
}

test("the editor connects to the sync socket", async ({ page }) => {
  const passage = await anOpenPassage(page);

  expect(passage).toMatch(/^passage_/);
  await expect(page.getByText("Synced")).toBeVisible();
});

test("typed prose reaches the server's own projection", async ({ page, request }) => {
  const passage = await anOpenPassage(page);

  await surface(page).click();
  await page.keyboard.type("The loom stood silent.");

  await expect
    .poll(
      async () => {
        const response = await request.get(`${API}/passages/${passage}`);
        return (await response.json()).text;
      },
      { timeout: 10_000 },
    )
    .toContain("The loom stood silent.");
});
