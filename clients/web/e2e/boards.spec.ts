import { test, expect, type Page } from "@playwright/test";

const aName = (of: string) => `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

const corkboard = (page: Page) => page.getByRole("region", { name: "Board" });

const waiting = (page: Page) => page.getByRole("list", { name: "Pieces not on the board" });

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
  await expect(page.getByRole("list", { name: "Pieces" }).getByText(idea)).toBeVisible();
}

async function openTheBoard(page: Page) {
  await page.getByRole("link", { name: "Open the board" }).click();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
}

test("a project's board can be opened from its pool", async ({ page }) => {
  await anOpenProject(page, "Board");

  await openTheBoard(page);

  await expect(page).toHaveURL(/\/projects\/project-[a-z0-9-]+-project_[0-9A-Za-z]{22}\/board$/);
});

test("a captured piece waits beside the board until it is pinned", async ({ page }) => {
  await anOpenProject(page, "Waiting");
  await capture(page, "The loom remembers");

  await openTheBoard(page);

  await expect(waiting(page).getByText("The loom remembers")).toBeVisible();
  await expect(corkboard(page).getByText("The loom remembers")).toHaveCount(0);
});

test("pinning a piece puts it on the board and takes it off the waiting list", async ({ page }) => {
  await anOpenProject(page, "Pinning");
  await capture(page, "The loom remembers");
  await openTheBoard(page);

  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
  await expect(waiting(page).getByText("The loom remembers")).toHaveCount(0);
});

test("a pinned piece is placed somewhere on the board", async ({ page }) => {
  await anOpenProject(page, "Placed");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  const card = corkboard(page).locator(".pinned").first();

  await expect(card).toBeVisible();
  const box = await card.boundingBox();
  const surface = await corkboard(page).boundingBox();
  expect(box).not.toBeNull();
  expect(surface).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(surface!.x);
  expect(box!.y).toBeGreaterThanOrEqual(surface!.y);
});

test("pinned pieces survive a reload", async ({ page }) => {
  await anOpenProject(page, "Survives");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  await page.reload();

  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
  await expect(waiting(page).getByText("The loom remembers")).toHaveCount(0);
});

test("reopening the board finds the same board rather than a new one", async ({ page }) => {
  await anOpenProject(page, "Reopen");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  await page.getByRole("link", { name: "Back to the pool" }).click();
  await openTheBoard(page);

  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
});

test("several pinned pieces all appear", async ({ page }) => {
  await anOpenProject(page, "Several");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);

  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();

  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);
});

test("a discarded piece leaves the board", async ({ page }) => {
  await anOpenProject(page, "Discarded");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  const board = page.url();
  const listed = await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const pieces = await fetch(`/api/pieces?project=${project}`).then((it) => it.json());
    const going = pieces.find((piece: { title: string }) => piece.title === "The loom remembers");
    await fetch(`/api/pieces/${going.id}`, { method: "DELETE" });

    return going.id;
  });
  expect(listed).toBeTruthy();

  await page.goto(board);

  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  await expect(waiting(page).getByText("She never returned")).toBeVisible();
});

test("an untitled piece can be pinned", async ({ page }) => {
  await anOpenProject(page, "Nameless");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.getByRole("list", { name: "Pieces" }).getByText("Untitled")).toBeVisible();
  await openTheBoard(page);

  await page.getByRole("button", { name: "Pin Untitled" }).click();

  await expect(corkboard(page).getByText("Untitled")).toBeVisible();
});

test("a pinned piece opens for writing from the board", async ({ page }) => {
  await anOpenProject(page, "Writing");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await corkboard(page).getByRole("link", { name: "The loom remembers" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
});
