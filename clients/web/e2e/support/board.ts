import { expect, type Locator, type Page } from "@playwright/test";

const aName = (of: string) => `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

export const corkboard = (page: Page) => page.getByRole("region", { name: "Board" });

export const waiting = (page: Page) => page.getByRole("list", { name: "Pieces not on the board" });

export const bar = (page: Page) => corkboard(page).locator(".pinned-actions");

export const cardNamed = (page: Page, named: string) =>
  corkboard(page).locator(".pinned").filter({ hasText: named });

export async function select(page: Page, named: string) {
  await cardNamed(page, named).click();
  await expect(bar(page)).toHaveAttribute("aria-label", `Actions for ${named}`);
}

export async function anOpenProject(page: Page, named: string): Promise<string> {
  const title = aName(named);

  await page.goto("/");
  await page.getByPlaceholder("A working title…").fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page.getByRole("link", { name: title }).click();

  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();

  return title;
}

export async function capture(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(page.getByRole("list", { name: "Pieces" }).getByText(idea)).toBeVisible();
}

export async function openTheBoard(page: Page) {
  await page.getByRole("link", { name: "Open the board" }).click();
  await expect(page.getByRole("heading", { name: "Board", exact: true })).toBeVisible();
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

export const boxOf = (page: Page) =>
  corkboard(page)
    .locator(".pinned")
    .first()
    .evaluate((it) => it.getAttribute("style"));

export async function pullBy(page: Page, side: string, named: string, x: number, y: number) {
  const grip = cardNamed(page, named).locator(`.grip.${side}`);
  const box = await grip.boundingBox();
  expect(box, `the ${side} grip should be there to grab`).not.toBeNull();
  const from = { x: box!.x + box!.width / 2, y: box!.y + box!.height / 2 };

  await page.mouse.move(from.x, from.y);
  await page.mouse.down();
  await page.mouse.move(from.x + x, from.y + y, { steps: 8 });
  await page.mouse.up();
}
