import {
  expect,
  type Locator,
  type Page,
  type Request,
} from "@playwright/test";

export const corkboard = (page: Page) =>
  page.getByRole("region", { name: "Board" });

export const waitingIdeas = (page: Page) =>
  page.getByRole("list", { name: "Ideas not on the board" });

export const bar = (page: Page) => corkboard(page).locator(".pinned-actions");

export const cardNamed = (page: Page, named: string) =>
  corkboard(page).locator(".pinned").filter({ hasText: named });

export async function select(page: Page, named: string) {
  await cardNamed(page, named).click();
  await expect(bar(page)).toHaveAttribute("aria-label", `Actions for ${named}`);
}

export async function openTheIdea(page: Page, named: string) {
  await select(page, named);
  await bar(page)
    .getByRole("button", { name: `Open ${named}` })
    .click();
}

export async function dragBy(page: Page, held: Locator, x: number, y: number) {
  const box = await held.boundingBox();
  expect(box).not.toBeNull();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + x, from.y + y, { steps: 8 });
  await page.mouse.up();
}

export const laidOut = (page: Page) =>
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

export const surface = (page: Page) => corkboard(page).locator(".surface");

export const zooming = (page: Page) =>
  corkboard(page).getByRole("toolbar", { name: "Zoom" });

export const seenAt = async (page: Page, named: string) => {
  const box = await cardNamed(page, named).boundingBox();
  const board = await corkboard(page).boundingBox();

  return {
    x: Math.round(box!.x - board!.x),
    y: Math.round(box!.y - board!.y),
    width: Math.round(box!.width),
    height: Math.round(box!.height),
  };
};

export async function panBy(page: Page, x: number, y: number) {
  const board = await corkboard(page).boundingBox();
  const from = {
    x: board!.x + board!.width - 40,
    y: board!.y + board!.height - 60,
  };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + x, from.y + y, { steps: 8 });
  await page.mouse.up();
}

export const boxOf = (page: Page) =>
  corkboard(page)
    .locator(".pinned")
    .first()
    .evaluate((it) => it.getAttribute("style"));

export async function pullBy(
  page: Page,
  side: string,
  named: string,
  x: number,
  y: number,
) {
  const grip = cardNamed(page, named).locator(`.grip.${side}`);
  const box = await grip.boundingBox();
  expect(box, `the ${side} grip should be there to grab`).not.toBeNull();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + x, from.y + y, { steps: 8 });
  await page.mouse.up();
}

export function boardSaves(page: Page): () => Promise<void> {
  let inFlight = 0;
  const writing = (request: Request) =>
    request.method() !== "GET" && request.url().includes("/api/boards");
  const done = (request: Request) => {
    if (writing(request)) inFlight -= 1;
  };

  page.on("request", (request) => {
    if (writing(request)) inFlight += 1;
  });
  page.on("requestfinished", done);
  page.on("requestfailed", done);

  return () =>
    expect
      .poll(() => inFlight, {
        message:
          "the board shows a change before it is saved, so a reload has to wait for the save",
      })
      .toBe(0);
}

export const boardOnServer = (page: Page) =>
  page.evaluate(async () => {
    const project = window.location.pathname.split("/")[2].split("-").pop();

    return fetch("/api/boards", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ project }),
    }).then((it) => it.json());
  });

export async function pinHeld(page: Page) {
  await expect(corkboard(page).locator(".pinned")).toHaveCount(1);
  await expect
    .poll(async () => (await boardOnServer(page)).ideas.length, {
      message: "the board shows the pin before the server has it",
    })
    .toBe(1);
}
