import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";
import { projectOf } from "./support/writing";

const masthead = (page: Page) => page.getByRole("heading", { level: 1 });

const detailsButton = (page: Page) =>
  page.getByRole("button", { name: "Project details" });

const details = (page: Page) =>
  page.getByRole("dialog", { name: "Project details" });

async function onServer(page: Page) {
  return (await page.request.get(`${API}/projects/${projectOf(page)}`)).json();
}

async function openTheDetails(page: Page) {
  await detailsButton(page).click();
  await expect(details(page)).toBeVisible();
}

test("the masthead names the project as it was written, not as its address spells it", async ({
  page,
}) => {
  const title = await aNewProject(page, "of the Ölmühle");

  await expect(masthead(page)).toHaveText(title);
});

test("the details open with what the project holds", async ({ page }) => {
  const title = await aNewProject(page, "Holding");
  const changed = await page.request.put(
    `${API}/projects/${projectOf(page)}/details`,
    { data: { language: "fr", description: "Deux villes." } },
  );
  expect(changed.ok()).toBe(true);
  await page.reload();

  await openTheDetails(page);

  await expect(details(page).getByLabel("Title")).toHaveValue(title);
  await expect(details(page).getByLabel("Language of the book")).toHaveValue(
    "fr",
  );
  await expect(details(page).getByLabel("Description")).toHaveValue(
    "Deux villes.",
  );
});

test("a project's title, language and description can be changed from inside it", async ({
  page,
}) => {
  await aNewProject(page, "Changing");
  const renamed = `Renamed ${crypto.randomUUID().slice(0, 8)}`;

  await openTheDetails(page);
  await details(page).getByLabel("Title").fill(renamed);
  await details(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "German (Deutsch)" });
  await details(page).getByLabel("Description").fill("Zwei Städte.");
  await details(page).getByRole("button", { name: "Save" }).click();

  await expect(details(page)).toHaveCount(0);
  await expect(
    masthead(page),
    "the masthead follows the new title at once",
  ).toHaveText(renamed);
  const found = await onServer(page);
  expect(found.name).toBe(renamed);
  expect(found.language).toBe("de");
  expect(found.description).toBe("Zwei Städte.");
});

test("Enter in the title saves the details", async ({ page }) => {
  await aNewProject(page, "Entering");
  const renamed = `Entered ${crypto.randomUUID().slice(0, 8)}`;

  await openTheDetails(page);
  await details(page).getByLabel("Title").fill(renamed);
  await details(page).getByLabel("Title").press("Enter");

  await expect(details(page)).toHaveCount(0);
  await expect(masthead(page)).toHaveText(renamed);
});

test("a title left as it was is not renamed", async ({ page }) => {
  await aNewProject(page, "Unrenamed");
  const renames: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "PATCH") {
      renames.push(request.url());
    }
  });

  await openTheDetails(page);
  await details(page).getByLabel("Description").fill("Only this.");
  await details(page).getByRole("button", { name: "Save" }).click();

  await expect(details(page)).toHaveCount(0);
  await expect
    .poll(async () => (await onServer(page)).description)
    .toBe("Only this.");
  expect(renames).toEqual([]);
});

test("the book takes its new language without leaving the writing view", async ({
  page,
}) => {
  await aNewProject(page, "Relanguaged");
  await aBookWithScenes(page, { "Part One": 1 });
  await views(page).getByRole("link", { name: "Write", exact: true }).click();
  const book = page.getByRole("article", { name: "The book" });
  await expect(book.locator(".scene .ProseMirror")).toHaveCount(1);

  await openTheDetails(page);
  await details(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "French (Français)" });
  await details(page).getByRole("button", { name: "Save" }).click();

  await expect(book).toHaveAttribute("lang", "fr");
});

test("a title the server refuses is told inside the dialog, which stays", async ({
  page,
}) => {
  const title = await aNewProject(page, "Refused");

  await openTheDetails(page);
  await details(page).getByLabel("Title").fill("a".repeat(201));
  await details(page).getByRole("button", { name: "Save" }).click();

  await expect(details(page).getByRole("alert")).toContainText(
    "at most 200 characters",
  );
  await expect(
    page.getByRole("alert"),
    "the problem is told beside the title it is about, nowhere else",
  ).toHaveCount(1);
  await expect(masthead(page)).toHaveText(title);

  await details(page).getByLabel("Title").fill("A shorter title");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("Escape leaves the details as they were", async ({ page }) => {
  const title = await aNewProject(page, "Escaping");

  await openTheDetails(page);
  await details(page).getByLabel("Title").fill("Never kept");
  await page.keyboard.press("Escape");

  await expect(details(page)).toHaveCount(0);
  await expect(masthead(page)).toHaveText(title);
  await openTheDetails(page);
  await expect(details(page).getByLabel("Title")).toHaveValue(title);
});

test("a blank title cannot be saved", async ({ page }) => {
  await aNewProject(page, "Blank");

  await openTheDetails(page);
  await details(page).getByLabel("Title").fill("   ");

  await expect(
    details(page).getByRole("button", { name: "Save" }),
  ).toBeDisabled();
});

test("a long name is cut short in the masthead, and told in full on hover", async ({
  page,
}) => {
  await aNewProject(page, "Long");
  const long = "Test ".repeat(30).trim();
  await openTheDetails(page);
  await details(page).getByLabel("Title").fill(long);
  await details(page).getByRole("button", { name: "Save" }).click();
  await expect(masthead(page)).toHaveAttribute("title", long);

  const seen = await page.evaluate(() => {
    const name = document.querySelector(".whose span") as HTMLElement;
    const button = document.querySelector(".project-details-button")!;
    const views = document.querySelector(".views")!;
    return {
      cut: name.scrollWidth > name.clientWidth,
      nameWidth: name.getBoundingClientRect().width,
      buttonBeforeViews:
        button.getBoundingClientRect().right <=
        views.getBoundingClientRect().left,
    };
  });
  expect(seen.cut, "the name is cut, not laid out in full").toBe(true);
  expect(seen.nameWidth).toBeLessThanOrEqual(24 * 16);
  expect(
    seen.buttonBeforeViews,
    "the details button stays beside the name, before the views",
  ).toBe(true);
});

test("the quotation marks follow the book's language until a style is chosen", async ({
  page,
}) => {
  await aNewProject(page, "Quoting");
  await openTheDetails(page);
  const marks = details(page).getByLabel("Quotation marks");
  await details(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "German (Deutsch)" });

  await expect(marks.locator("option").first()).toHaveText(
    "As the language has it, »…«",
  );
  await details(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "English" });
  await expect(marks.locator("option").first()).toHaveText(
    "As the language has it, “…”",
  );
  await details(page).getByRole("button", { name: "Save" }).click();
  await expect(details(page)).toHaveCount(0);

  const found = await onServer(page);
  expect(found.quotation, "following the language stores no style").toBe(null);
});

test("a quotation style chosen for the book is kept", async ({ page }) => {
  await aNewProject(page, "QuotingChosen");
  await openTheDetails(page);

  await details(page)
    .getByLabel("Quotation marks")
    .selectOption({ label: "„…“" });
  await details(page).getByRole("button", { name: "Save" }).click();
  await expect(details(page)).toHaveCount(0);

  expect((await onServer(page)).quotation).toBe("low-high");
  await page.reload();
  await openTheDetails(page);
  await expect(details(page).getByLabel("Quotation marks")).toHaveValue(
    "low-high",
  );
});
