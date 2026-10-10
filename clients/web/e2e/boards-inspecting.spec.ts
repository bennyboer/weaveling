import { test, expect, type Page } from "@playwright/test";

import { cardNamed, corkboard, select } from "./support/board";
import { anOpenProject, captureIdea, openTheBoard } from "./support/shell";

const API = "http://127.0.0.1:3000/api";

const inspector = (page: Page) =>
  page.getByRole("region", { name: "Inspector" });

const nameField = (page: Page) =>
  inspector(page).getByRole("textbox", { name: "Idea name" });

async function aBoardWith(page: Page, named: string, ideas: string[]) {
  await anOpenProject(page, named);
  for (const idea of ideas) {
    await captureIdea(page, idea);
  }
  await openTheBoard(page);
  for (const idea of ideas) {
    await page.getByRole("button", { name: `Pin ${idea}` }).click();
    await expect(cardNamed(page, idea)).toBeVisible();
  }
}

test("nothing is inspected until a card is selected", async ({ page }) => {
  await aBoardWith(page, "Unselected", ["The loom remembers"]);

  await expect(inspector(page)).toHaveCount(0);
});

test("selecting a card inspects its idea", async ({ page }) => {
  await aBoardWith(page, "Inspecting", ["The loom remembers"]);

  await select(page, "The loom remembers");

  await expect(nameField(page)).toHaveValue("The loom remembers");
  await expect(inspector(page).getByText("Not in the book yet")).toBeVisible();
});

test("the bare board puts the inspector away", async ({ page }) => {
  await aBoardWith(page, "PutAway", ["The loom remembers"]);
  await select(page, "The loom remembers");
  await expect(inspector(page)).toBeVisible();

  await corkboard(page).click({ position: { x: 5, y: 5 } });

  await expect(inspector(page)).toHaveCount(0);
});

test("selecting another card inspects that one instead", async ({ page }) => {
  await aBoardWith(page, "Switching", [
    "The loom remembers",
    "She never returned",
  ]);
  await select(page, "The loom remembers");
  await expect(nameField(page)).toHaveValue("The loom remembers");

  await select(page, "She never returned");

  await expect(nameField(page)).toHaveValue("She never returned");
});

test("renaming in the inspector renames the card", async ({ page }) => {
  await aBoardWith(page, "RenameDocked", ["The loom remembers"]);
  await select(page, "The loom remembers");

  await nameField(page).fill("The loom forgets");
  await nameField(page).press("Enter");

  await expect(cardNamed(page, "The loom forgets")).toBeVisible();
  await expect(nameField(page)).toHaveValue("The loom forgets");
});

test("renaming the card in place renames it in the inspector", async ({
  page,
}) => {
  await aBoardWith(page, "RenameCard", ["The loom remembers"]);
  await select(page, "The loom remembers");
  await expect(nameField(page)).toHaveValue("The loom remembers");

  await page.getByRole("button", { name: "Rename The loom remembers" }).click();
  const field = page.getByRole("textbox", {
    name: "Rename The loom remembers",
  });
  await field.fill("The loom forgets");
  await field.press("Enter");

  await expect(corkboard(page).locator(".name")).toHaveText("The loom forgets");
  await expect(nameField(page)).toHaveValue("The loom forgets");
});

test("the inspector shows where the selected idea appears", async ({
  page,
}) => {
  await aBoardWith(page, "Appears", ["The loom remembers"]);
  const project = new URL(page.url()).pathname.split("/")[2].split("-").pop();
  const ideas = await (
    await page.request.get(`${API}/ideas?project=${project}`)
  ).json();
  const made = await page.request.post(`${API}/scenes`, {
    data: { project },
  });
  const { id: scene } = await made.json();
  await page.request.patch(`${API}/scenes/${scene}`, {
    data: { title: "The clearing" },
  });
  await page.request.post(`${API}/scenes/${scene}/ideas`, {
    data: { idea: ideas[0].id },
  });

  await expect(async () => {
    await page.reload();
    await select(page, "The loom remembers");
    await expect(
      inspector(page).getByRole("link", { name: "The clearing" }),
    ).toBeVisible({ timeout: 1_000 });
  }).toPass();
});
