import { expect, type Page } from "@playwright/test";

const aName = (of: string) =>
  `Project ${of} ${crypto.randomUUID().slice(0, 8)}`;

export const views = (page: Page) =>
  page.getByRole("navigation", { name: "Views" });

export async function aNewProject(page: Page, named: string): Promise<string> {
  const title = aName(named);

  await page.goto("/");
  await page.getByPlaceholder("A working title…").fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page.getByRole("link", { name: title }).click();

  await onTheBoard(page);

  return title;
}

export async function anOpenProject(
  page: Page,
  named: string,
): Promise<string> {
  const title = await aNewProject(page, named);

  await openThePool(page);

  return title;
}

export async function capture(page: Page, idea: string) {
  await page.getByRole("textbox", { name: "What is the idea?" }).fill(idea);
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect(
    page.getByRole("list", { name: "Pieces" }).getByText(idea),
  ).toBeVisible();
}

export async function onTheBoard(page: Page) {
  await expect(page.getByRole("region", { name: "Board" })).toBeVisible();
}

export async function openTheBoard(page: Page) {
  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await onTheBoard(page);
}

export async function openThePool(page: Page) {
  await views(page).getByRole("link", { name: "Pieces", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Pieces", exact: true }),
  ).toBeVisible();
}
