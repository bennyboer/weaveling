import { test, expect } from "@playwright/test";

import {
  boardSaves,
  corkboard,
  laidOut,
  select,
  waitingIdeas,
} from "./support/board";
import {
  anOpenProject,
  captureIdea,
  uncatalogued,
  onTheBoard,
  openTheBoard,
  openThePool,
} from "./support/shell";

test("a project's board can be opened from its pool", async ({ page }) => {
  // given: a project open on its pool
  await anOpenProject(page, "Board");

  // when: its board is opened
  await openTheBoard(page);

  // then: the board shows
  await expect(page).toHaveURL(
    /\/projects\/project-[a-z0-9-]+-project_[0-9A-Za-z]{22}\/board$/,
  );
});

test("a captured idea waits beside the board until it is pinned", async ({
  page,
}) => {
  // given: a captured idea
  await anOpenProject(page, "Waiting");
  await captureIdea(page, "The loom remembers");

  // when: the board is opened
  await openTheBoard(page);

  // then: the idea waits beside it, not on it
  await expect(
    waitingIdeas(page).getByText("The loom remembers"),
  ).toBeVisible();
  await expect(corkboard(page).getByText("The loom remembers")).toHaveCount(0);
});

test("pinning an idea puts it on the board and takes it off the waiting list", async ({
  page,
}) => {
  // given: an idea waiting beside the board
  await anOpenProject(page, "Pinning");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);

  // when: it is pinned
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // then: it is on the board
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
  // and: no longer waits
  await expect(waitingIdeas(page).getByText("The loom remembers")).toHaveCount(
    0,
  );
});

test("a pinned idea is placed somewhere on the board", async ({ page }) => {
  // given: an idea waiting beside the board
  await anOpenProject(page, "Placed");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  // when: it is pinned
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // then: its card sits within the board
  const card = corkboard(page).locator(".pinned").first();

  await expect(card).toBeVisible();
  const box = await card.boundingBox();
  const surface = await corkboard(page).boundingBox();
  expect(box).not.toBeNull();
  expect(surface).not.toBeNull();
  expect(box!.x).toBeGreaterThanOrEqual(surface!.x);
  expect(box!.y).toBeGreaterThanOrEqual(surface!.y);
});

test("pinned ideas survive a reload", async ({ page }) => {
  // given: an idea pinned to the board
  await anOpenProject(page, "Survives");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  // when: the board is reloaded
  await saved();
  await page.reload();

  // then: the idea is still pinned
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
  await expect(waitingIdeas(page).getByText("The loom remembers")).toHaveCount(
    0,
  );
});

test("reopening the board finds the same board rather than a new one", async ({
  page,
}) => {
  // given: an idea pinned to the board
  await anOpenProject(page, "Reopen");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  // when: the board is left and opened again
  await openThePool(page);
  await openTheBoard(page);

  // then: the idea is still pinned
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();
});

test("several pinned ideas all appear, each in its own spot", async ({
  page,
}) => {
  // given: six ideas waiting beside the board
  await anOpenProject(page, "Several");
  const names = ["One", "Two", "Three", "Four", "Five", "Six"];
  for (const name of names) {
    await captureIdea(page, name);
  }
  await openTheBoard(page);
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(
    names.length,
  );

  // when: all are pinned at once
  await page.evaluate(() => {
    for (const button of document.querySelectorAll(".waiting button")) {
      (button as HTMLElement).click();
    }
  });

  // then: each lands in a spot of its own
  await expect(corkboard(page).locator(".pinned")).toHaveCount(names.length);
  const spots = await corkboard(page)
    .locator(".pinned")
    .evaluateAll((cards) => cards.map((card) => card.getAttribute("style")));
  expect(new Set(spots).size, `six pins landed on ${spots.join(" / ")}`).toBe(
    names.length,
  );
});

test("pins still in flight keep their spots while slow answers come back", async ({
  page,
}) => {
  // given: six ideas waiting beside the board
  await anOpenProject(page, "Slow answers");
  const names = ["One", "Two", "Three", "Four", "Five", "Six"];
  for (const name of names) {
    await captureIdea(page, name);
  }
  await openTheBoard(page);
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(
    names.length,
  );
  // and: a slow server
  await page.route("**/api/boards/**", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 300));
    await route.continue();
  });

  // when: all are pinned one after another
  for (const name of names) {
    await page.getByRole("button", { name: `Pin ${name}` }).click();
    await page.waitForTimeout(120);
  }

  // then: each keeps a spot of its own
  await expect(corkboard(page).locator(".pinned")).toHaveCount(names.length);
  await page.waitForTimeout(500);
  const spots = await corkboard(page)
    .locator(".pinned")
    .evaluateAll((cards) => cards.map((card) => card.getAttribute("style")));
  expect(
    new Set(spots).size,
    `an answer for an earlier pin must not wipe out the pins still on their way: ${spots.join(" / ")}`,
  ).toBe(names.length);
});

test("a spot freed by unpinning is handed to the next idea", async ({
  page,
}) => {
  // given: two ideas pinned to the board
  await anOpenProject(page, "Reused");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await page.getByRole("button", { name: "Pin She never returned" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);

  // when: the first is unpinned and pinned again
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();

  // then: the two still sit in spots of their own
  await expect(corkboard(page).locator(".pinned")).toHaveCount(2);
  const spots = await corkboard(page)
    .locator(".pinned")
    .evaluateAll((cards) => cards.map((card) => card.getAttribute("style")));
  expect(new Set(spots).size, `two pins landed on ${spots.join(" / ")}`).toBe(
    2,
  );
});

test("a discarded idea leaves the board", async ({ page }) => {
  // given: an idea pinned to the board, another waiting
  await anOpenProject(page, "Discarded");
  await captureIdea(page, "The loom remembers");
  await captureIdea(page, "She never returned");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).getByText("The loom remembers")).toBeVisible();

  // when: the pinned idea is discarded
  const board = page.url();
  const listed = await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const ideas = await fetch(`/api/ideas?project=${project}`).then((it) =>
      it.json(),
    );
    const going = ideas.find(
      (idea: { title: string }) => idea.title === "The loom remembers",
    );
    await fetch(`/api/ideas/${going.id}`, { method: "DELETE" });

    return going.id;
  });
  expect(listed).toBeTruthy();
  await uncatalogued(page, ["The loom remembers"]);

  // and: the board is opened again
  await page.goto(board);

  // then: nothing is pinned
  await onTheBoard(page);
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  // and: the other idea still waits
  await expect(
    waitingIdeas(page).getByText("She never returned"),
  ).toBeVisible();
});

test("an untitled idea can be pinned", async ({ page }) => {
  // given: an untitled idea
  await anOpenProject(page, "Nameless");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "Ideas" }).getByText("Untitled"),
  ).toBeVisible();
  await openTheBoard(page);

  // when: it is pinned
  await page.getByRole("button", { name: "Pin Untitled" }).click();

  // then: it is on the board
  await expect(corkboard(page).getByText("Untitled")).toBeVisible();
});

test("a placement whose idea was never captured draws nothing", async ({
  page,
}) => {
  // given: an idea pinned to the board
  await anOpenProject(page, "Dangling");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  await expect
    .poll(
      () =>
        page.evaluate(async () => {
          const project = window.location.pathname
            .split("/")[2]
            .split("-")
            .pop();
          const board = await fetch("/api/boards", {
            method: "POST",
            headers: { "content-type": "application/json" },
            body: JSON.stringify({ project }),
          }).then((it) => it.json());

          return board.ideas.length;
        }),
      { message: "the board shows the pin before the server has it" },
    )
    .toBe(1);

  // and: a placement of an idea never captured
  await page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();
    const board = await fetch("/api/boards", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project }),
    }).then((it) => it.json());

    await fetch(`/api/boards/${board.id}/ideas`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        idea: "idea_000000000000000000000A",
        spot: { x: 520, y: 40 },
      }),
    });
  });

  // when: the board is reloaded
  await page.reload();
  await onTheBoard(page);

  // then: only the captured idea is drawn
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  // and: nothing is reported failed
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a pinned idea can be unpinned back to the waiting list", async ({
  page,
}) => {
  // given: an idea pinned to the board
  await anOpenProject(page, "Unpinning");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);

  // when: it is unpinned
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();

  // then: it leaves the board
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  // and: waits beside it again
  await expect(
    waitingIdeas(page).getByText("The loom remembers"),
  ).toBeVisible();
});

test("an unpinned idea stays off the board after a reload", async ({
  page,
}) => {
  // given: an idea pinned and unpinned again
  await anOpenProject(page, "UnpinLasts");
  await captureIdea(page, "The loom remembers");
  const saved = boardSaves(page);
  await openTheBoard(page);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await select(page, "The loom remembers");
  await page.getByRole("button", { name: "Unpin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);

  // when: the board is reloaded
  await saved();
  await page.reload();
  await onTheBoard(page);

  // then: the idea is off the board, waiting beside it
  await expect(corkboard(page).locator(".pinned")).toHaveCount(0);
  await expect(
    waitingIdeas(page).getByText("The loom remembers"),
  ).toBeVisible();
});

test("freshly pinned ideas land inside the board as it is first shown", async ({
  page,
}) => {
  // given: six ideas waiting beside a fresh board
  await anOpenProject(page, "Layout");
  const names = ["One", "Two", "Three", "Four", "Five", "Six"];
  for (const name of names) {
    await captureIdea(page, name);
  }
  await openTheBoard(page);

  // when: all are pinned
  for (const name of names) {
    await page.getByRole("button", { name: `Pin ${name}` }).click();
  }
  await expect(corkboard(page).locator(".pinned")).toHaveCount(names.length);

  // then: each can be seen without scrolling
  const board = await laidOut(page);
  for (const card of board.cards) {
    expect(
      card.right,
      `${card.name} should not need scrolling to be seen`,
    ).toBeLessThanOrEqual(board.client.width);
    expect(
      card.bottom,
      `${card.name} should not need scrolling to be seen`,
    ).toBeLessThanOrEqual(board.client.height);
  }
});

test("nothing can be pinned until the board has actually arrived", async ({
  page,
}) => {
  // given: a captured idea
  await anOpenProject(page, "Loading");
  await captureIdea(page, "The loom remembers");

  // and: a board slow to arrive
  let held: (() => void) | undefined;
  const opening = new Promise<void>((release) => {
    held = release;
  });
  await page.route("**/api/boards", async (route) => {
    await opening;
    await route.continue();
  });

  // when: the board is opened
  await openTheBoard(page);

  // then: nothing can be pinned yet
  await expect(
    waitingIdeas(page).getByRole("button"),
    "a pin clicked before the board exists would be swallowed",
  ).toHaveCount(0);

  // when: the board arrives
  held!();

  // then: the idea can be pinned
  await expect(waitingIdeas(page).getByRole("button")).toHaveCount(1);
  await page.getByRole("button", { name: "Pin The loom remembers" }).click();
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
});
