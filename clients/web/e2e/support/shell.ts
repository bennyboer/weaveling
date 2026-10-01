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
    page.getByRole("list", { name: "Ideas" }).getByText(idea),
  ).toBeVisible();
}

export async function onTheBoard(page: Page) {
  await expect(page.getByRole("region", { name: "Board" })).toBeVisible();
}

export async function openTheBoard(page: Page) {
  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await onTheBoard(page);
}

export async function openTheOutline(page: Page) {
  await views(page).getByRole("link", { name: "Outline", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Add a section" }),
  ).toBeVisible();
}

export async function openThePool(page: Page) {
  await views(page).getByRole("link", { name: "Ideas", exact: true }).click();
  await expect(
    page.getByRole("heading", { name: "Ideas", exact: true }),
  ).toBeVisible();
}

export async function aLoosePassage(page: Page): Promise<string> {
  const project = new URL(page.url()).pathname.split("/")[2];
  const made = await page.request.post("http://127.0.0.1:3000/api/passages", {
    data: { project: project.split("-").pop() },
  });
  expect(made.status()).toBe(201);

  return (await made.json()).id;
}
