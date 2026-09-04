import { test, expect, type Locator, type Page } from "@playwright/test";

const aName = (of: string) => `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

const corkboard = (page: Page) => page.getByRole("region", { name: "Board" });

const waiting = (page: Page) => page.getByRole("list", { name: "Pieces not on the board" });

const bar = (page: Page) => corkboard(page).locator(".pinned-actions");

const cardNamed = (page: Page, named: string) =>
  corkboard(page).locator(".pinned").filter({ hasText: named });

async function select(page: Page, named: string) {
  await cardNamed(page, named).click();
  await expect(bar(page)).toHaveAttribute("aria-label", `Actions for ${named}`);
}

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

test("a pinned piece opens for writing from the board", async ({ page }) => {
  await anOpenProject(page, "Writing");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await corkboard(page).locator(".pinned").dblclick();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
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

test("a card can be nudged with the arrow keys", async ({ page }) => {
  await anOpenProject(page, "Nudging");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  await card.focus();
  await page.keyboard.press("ArrowDown");

  await expect(card, "one arrow press moves one grid cell").toHaveAttribute("style", /top: 45px/);

  await page.keyboard.press("Shift+ArrowDown");

  await expect(card, "shift leaps eight cells").toHaveAttribute("style", /top: 85px/);
});

test("a nudge survives a reload", async ({ page }) => {
  await anOpenProject(page, "NudgeLasts");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").focus();
  await page.keyboard.press("ArrowRight");
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /left: 45px/);

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute("style", /left: 45px/);
});

async function dragBy(page: Page, held: Locator, x: number, y: number) {
  const box = await held.boundingBox();
  expect(box).not.toBeNull();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + x, from.y + y, { steps: 8 });
  await page.mouse.up();
}

test("a card dragged anywhere on its body lands where it was dropped and stays there", async ({
  page,
}) => {
  await anOpenProject(page, "Dragging");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 120, 90);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 160px; top: 130px;/,
  );
});

test("the writing view leads back to the board", async ({ page }) => {
  await anOpenProject(page, "BackToBoard");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").dblclick();
  await expect(page.locator(".surface .ProseMirror")).toBeVisible();

  await page.getByRole("link", { name: "Back to the board" }).click();

  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
});

test("a cancelled drag puts the card back where it was", async ({ page }) => {
  await anOpenProject(page, "Cancelled");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  const carried = await page.evaluate(async () => {
    const card = document.querySelector(".pinned")!;
    const fire = (type: string, extra: PointerEventInit = {}) =>
      card.dispatchEvent(new PointerEvent(type, { bubbles: true, pointerId: 7, ...extra }));
    const style = () => document.querySelector(".pinned")!.getAttribute("style");

    fire("pointerdown");
    fire("pointermove", { movementX: 30, movementY: 20 });
    await new Promise((it) => setTimeout(it, 50));
    const held = style();

    fire("pointercancel");
    await new Promise((it) => setTimeout(it, 50));
    fire("pointermove", { movementX: 100, movementY: 100 });
    await new Promise((it) => setTimeout(it, 50));

    return { held, afterwards: style() };
  });

  expect(carried.held).toMatch(/left: 70px; top: 60px;/);
  expect(carried.afterwards).toMatch(/left: 40px; top: 40px;/);
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);
});

const laidOut = (page: Page) =>
  page.evaluate(() => {
    const board = document.querySelector(".corkboard")!;
    const origin = board.getBoundingClientRect();

    return {
      client: { width: board.clientWidth, height: board.clientHeight },
      scroll: { width: board.scrollWidth, height: board.scrollHeight },
      cards: [...document.querySelectorAll(".pinned")].map((card) => {
        const box = card.getBoundingClientRect();

        return {
          name: card.querySelector(".name")!.textContent,
          right: Math.round(box.right - origin.left + board.scrollLeft),
          bottom: Math.round(box.bottom - origin.top + board.scrollTop),
        };
      }),
    };
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

test("a card lands on the grid, however sloppily it is dropped", async ({ page }) => {
  await anOpenProject(page, "Snapping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 43, 27);

  await expect(
    corkboard(page).locator(".pinned"),
    "40+43 snaps to 85 and 40+27 snaps to 65",
  ).toHaveAttribute("style", /left: 85px; top: 65px;/);
});

test("a slip too small to be a drag leaves the card where it was", async ({ page }) => {
  await anOpenProject(page, "Slipping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  await dragBy(page, corkboard(page).locator(".pinned"), 2, 1);

  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );
});

test("a single click selects a card, and the bare board deselects it", async ({ page }) => {
  await anOpenProject(page, "Selecting");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const cards = corkboard(page).locator(".pinned");
  await expect(cards).toHaveCount(2);

  await cards.first().click();

  await expect(cards.first()).toHaveClass(/selected/);
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  await cards.nth(1).click();

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
  await expect(cards.nth(1)).toHaveClass(/selected/);

  await corkboard(page).click({ position: { x: 12, y: 320 } });

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("escape lets go of a selected card", async ({ page }) => {
  await anOpenProject(page, "Escaping");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await corkboard(page).locator(".pinned").click();
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);

  await page.keyboard.press("Escape");

  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(0);
});

test("a card stays selected all the way through a drag", async ({ page }) => {
  await anOpenProject(page, "StaysLit");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 60, from.y + 45, { steps: 6 });

  await expect(card, "the outline must not blink off mid-drag").toHaveClass(/selected/);

  await page.mouse.up();

  await expect(card).toHaveClass(/selected/);
});

test("a dropped card does not flash back to where it came from", async ({ page }) => {
  await anOpenProject(page, "NoFlash");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 50, { steps: 6 });
  await page.mouse.up();

  const rightAway = await card.getAttribute("style");
  expect(rightAway, "the drop must not wait on the server").toMatch(/left: 140px; top: 90px;/);
  await expect(card).toHaveAttribute("style", /left: 140px; top: 90px;/);
});

test("clicking a card's title selects it rather than opening it", async ({ page }) => {
  await anOpenProject(page, "TitleClick");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  await corkboard(page).locator(".name").click();

  await expect(page).toHaveURL(board);
  await expect(corkboard(page).locator(".pinned.selected")).toHaveCount(1);
});

test("a card can be dragged by its title", async ({ page }) => {
  await anOpenProject(page, "TitleDrag");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const board = page.url();

  await dragBy(page, corkboard(page).locator(".name"), 60, 40);

  await expect(page, "dragging the title must not follow the link").toHaveURL(board);
  await expect(corkboard(page).locator(".pinned")).toHaveAttribute(
    "style",
    /left: 100px; top: 80px;/,
  );
});

test("a very long title scrolls inside its card instead of stretching it", async ({ page }) => {
  await anOpenProject(page, "LongTitle");
  await capture(
    page,
    "Ich habe eine sehr lange Idee. Von einem Buch. Allerdings ist es noch nicht so das ich mir sicher bin ob das eine gute Idee ist.",
  );
  await openTheBoard(page);
  await page.getByRole("button", { name: /^Pin Ich habe/ }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveCount(1);

  const shape = await card.evaluate((it) => {
    const name = it.querySelector(".name")!;

    return {
      cardHeight: (it as HTMLElement).offsetHeight,
      nameScroll: name.scrollHeight,
      nameClient: name.clientHeight,
    };
  });

  expect(shape.cardHeight, "the card keeps its size").toBeLessThan(120);
  expect(shape.nameScroll, "the whole title is still there to scroll to").toBeGreaterThan(
    shape.nameClient,
  );
});

test("a card being dragged rides above the ones pinned after it", async ({ page }) => {
  await anOpenProject(page, "OnTop");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  const first = corkboard(page).locator(".pinned").first();
  const box = await first.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 100, from.y + 5, { steps: 6 });

  await expect(first).toHaveClass(/carried/);
  const onTop = await page.evaluate(() => {
    const carried = document.querySelector(".pinned.carried")!;
    const box = carried.getBoundingClientRect();
    const middle = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);

    return carried.contains(middle);
  });
  expect(onTop, "the card under the cursor must be the one being dragged").toBe(true);

  await page.mouse.up();
  await expect(first).not.toHaveClass(/carried/);
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

test("selecting a card raises an action bar over it", async ({ page }) => {
  await anOpenProject(page, "Bar");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(bar(page)).toHaveCount(0);

  await cardNamed(page, "The loom remembers").click();

  await expect(bar(page)).toHaveAttribute("aria-label", "Actions for The loom remembers");
  for (const deed of ["Rename", "Open", "Unpin"]) {
    await expect(bar(page).getByRole("button", { name: `${deed} The loom remembers` })).toBeVisible();
  }

  await page.keyboard.press("Escape");

  await expect(bar(page)).toHaveCount(0);
});

test("the bar drops below a card that is too near the top edge", async ({ page }) => {
  await anOpenProject(page, "BarFlip");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /top: 40px/);

  await select(page, "The loom remembers");

  await expect(bar(page), "40px is not enough room for a 42px bar").toHaveClass(/below/);

  await card.focus();
  await page.keyboard.press("Shift+ArrowDown");
  await expect(card).toHaveAttribute("style", /top: 80px/);

  await expect(bar(page)).not.toHaveClass(/below/);
});

test("the bar steps aside while a card is being dragged", async ({ page }) => {
  await anOpenProject(page, "BarDrag");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  const box = await card.boundingBox();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + 80, from.y + 60, { steps: 6 });

  await expect(bar(page)).toHaveCount(0);

  await page.mouse.up();

  await expect(bar(page)).toHaveCount(1);
});

test("a card can be renamed from its bar, and the new title sticks", async ({ page }) => {
  await anOpenProject(page, "Renaming");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });
  await expect(field).toBeFocused();
  await expect(field).toHaveValue("The loom remembers");

  await field.fill("The loom forgets");
  await field.press("Enter");

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("escape abandons a rename and keeps the old title", async ({ page }) => {
  await anOpenProject(page, "RenameEscape");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });

  await field.fill("Thrown away");
  await field.press("Escape");

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom remembers");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom remembers");
});

test("arrow keys write into a title being renamed instead of moving the card", async ({ page }) => {
  await anOpenProject(page, "RenameKeys");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  const card = corkboard(page).locator(".pinned");
  await expect(card).toHaveAttribute("style", /left: 40px; top: 40px;/);

  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  await page.keyboard.press("ArrowRight");
  await page.keyboard.press("ArrowDown");

  await expect(card, "the card must stay put while its title is being edited").toHaveAttribute(
    "style",
    /left: 40px; top: 40px;/,
  );
});

test("the bar's open button opens the piece for writing", async ({ page }) => {
  await anOpenProject(page, "BarOpen");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");

  await page.getByRole("button", { name: "Open The loom remembers" }).click();

  await expect(page.locator(".surface .ProseMirror")).toBeVisible();
  await expect(page).toHaveURL(/\/pieces\/the-loom-remembers-piece_/);
});

test("clicking away from a rename keeps what was typed", async ({ page }) => {
  await anOpenProject(page, "RenameAway");
  await capture(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();

  await page.getByRole("textbox", { name: "Rename The loom remembers" }).fill("The loom forgets");
  await corkboard(page).click({ position: { x: 12, y: 320 } });

  await expect(corkboard(page).locator(".pinned-rename")).toHaveCount(0);
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");

  await page.reload();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
});

test("a rename survives the board changing underneath it", async ({ page }) => {
  await anOpenProject(page, "RenameSturdy");
  await capture(page, "The loom remembers");
  await capture(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", { name: "Rename The loom remembers" });
  await field.fill("Half typed");

  await page.evaluate(() => {
    (document.querySelector(".waiting button") as HTMLElement).click();
  });
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  await expect(field, "pinning another piece must not wipe the editor").toHaveValue("Half typed");
  await field.press("Enter");
  await expect(corkboard(page).locator(".name").first()).toHaveText("Half typed");
});
