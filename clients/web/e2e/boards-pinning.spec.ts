import { test, expect } from "@playwright/test";

import {
  anOpenProject,
  capture,
  corkboard,
  laidOut,
  openTheBoard,
  select,
  waiting,
} from "./support/board";

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

test("several pinned pieces all appear, each in its own spot", async ({ page }) => {
  await anOpenProject(page, "Several");
  const names = ["One", "Two", "Three", "Four", "Five", "Six"];
  for (const name of names) {
    await capture(page, name);
  }
  await openTheBoard(page);
  await expect(waiting(page).getByRole("button")).toHaveCount(names.length);

  await page.evaluate(() => {
    for (const button of document.querySelectorAll(".waiting button")) {
      (button as HTMLElement).click();
    }
  });

  await expect(corkboard(page).locator(".pinned")).toHaveCount(names.length);
  const spots = await corkboard(page)
    .locator(".pinned")
    .evaluateAll((cards) => cards.map((card) => card.getAttribute("style")));
  expect(new Set(spots).size, `six pins landed on ${spots.join(" / ")}`).toBe(names.length);
});

test("a spot freed by unpinning is handed to the next piece", async ({ page }) => {
  await anOpenProject(page, "Reused");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);
  const spots = await corkboard(page)
    .locator(".pinned")
    .evaluateAll((cards) => cards.map((card) => card.getAttribute("style")));
  expect(new Set(spots).size, `two pins landed on ${spots.join(" / ")}`).toBe(2);
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

test("a placement whose piece was never captured draws nothing", async ({ page }) => {
  await anOpenProject(page, "Dangling");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const board = await fetch("/api/boards", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project }),
    }).then((it) => it.json());

    await fetch(`/api/boards/${board.id}/pieces`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        piece: "piece_000000000000000000000A",
        spot: { x: 520, y: 40 },
      }),
    });
  });

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a pinned piece can be unpinned back to the waiting list", async ({ page }) => {
  await anOpenProject(page, "Unpinning");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();

  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  await expect(waiting(page).getByText("The loom remembers")).toBeVisible();
});

test("an unpinned piece stays off the board after a reload", async ({ page }) => {
  await anOpenProject(page, "UnpinLasts");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  await expect(waiting(page).getByText("The loom remembers")).toBeVisible();
});

test("freshly pinned pieces land inside the board as it is first shown", async ({ page }) => {
  await anOpenProject(page, "Layout");
  const names = ["One", "Two", "Three", "Four", "Five", "Six"];
  for (const name of names) {
    await capture(page, name);
  }
  await openTheBoard(page);

  for (const name of names) {
    await page.getByRole("button", { name: `Pin ${name}` }).click();
  }
  await expect(corkboard(page).locator(".pinned")).toHaveCount(names.length);

  const board = await laidOut(page);
  for (const card of board.cards) {
    expect(card.right, `${card.name} should not need scrolling to be seen`).toBeLessThanOrEqual(
      board.client.width,
    );
    expect(card.bottom, `${card.name} should not need scrolling to be seen`).toBeLessThanOrEqual(
      board.client.height,
    );
  }
});

test("a piece pinned beyond the edge can still be scrolled to", async ({ page }) => {
  await anOpenProject(page, "FarAway");
  await capture(page, "Far away");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin Far away" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const board = await fetch("/api/boards", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project }),
    }).then((it) => it.json());
    const piece = board.pieces[0].piece;

    await fetch(`/api/boards/${board.id}/pieces/${piece}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ spot: { x: 40, y: 3000 } }),
    });
  });
  await page.reload();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  const board = await laidOut(page);
  expect(board.scroll.height).toBeGreaterThan(board.client.height);

  const middle = await corkboard(page).boundingBox();
  await page.mouse.move(middle!.x + middle!.width / 2, middle!.y + middle!.height / 2);
  await page.mouse.wheel(0, 400);

  await expect
    .poll(() => corkboard(page).evaluate((it) => it.scrollTop), {
      message: "the wheel should carry the board towards the far piece",
    })
    .toBeGreaterThan(0);
});

test("nothing can be pinned until the board has actually arrived", async ({ page }) => {
  await anOpenProject(page, "Loading");
  await capture(page, "The loom remembers");

  let held: (() => void) | undefined;
  const opening = new Promise<void>((release) => {
    held = release;
  });
  await page.route("**/api/boards", async (route) => {
    await opening;
    await route.continue();
  });

  await page.getByRole("link", { name: "Open the board" }).click();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  await expect(
    waiting(page).getByRole("button"),
    "a pin clicked before the board exists would be swallowed",
  ).toHaveCount(0);

  held!();

  await expect(waiting(page).getByRole("button")).toHaveCount(1);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
});
