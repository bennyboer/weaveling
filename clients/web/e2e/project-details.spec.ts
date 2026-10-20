import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, views } from "./support/shell";
import { aBookWithScenes } from "./support/book";
import { projectOf, theBookIsIn } from "./support/writing";

const masthead = (page: Page) => page.getByRole("heading", { level: 1 });

const projectDetailsButton = (page: Page) =>
  page.getByRole("button", { name: "Project details" });

const projectDetails = (page: Page) =>
  page.getByRole("dialog", { name: "Project details" });

async function onServer(page: Page) {
  return (await page.request.get(`${API}/projects/${projectOf(page)}`)).json();
}

const changeTheMarks = (page: Page) =>
  projectDetails(page).getByRole("button", { name: "Change the marks" });

async function openTheProjectDetails(page: Page) {
  await projectDetailsButton(page).click();
  await expect(projectDetails(page)).toBeVisible();
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

  await openTheProjectDetails(page);

  await expect(projectDetails(page).getByLabel("Title")).toHaveValue(title);
  await expect(
    projectDetails(page).getByLabel("Language of the book"),
  ).toHaveValue("fr");
  await expect(projectDetails(page).getByLabel("Description")).toHaveValue(
    "Deux villes.",
  );
});

test("a project's title, language and description can be changed from inside it", async ({
  page,
}) => {
  await aNewProject(page, "Changing");
  const renamed = `Renamed ${crypto.randomUUID().slice(0, 8)}`;

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill(renamed);
  await projectDetails(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "Dutch (Nederlands)" });
  await projectDetails(page).getByLabel("Description").fill("Twee steden.");
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page)).toBeHidden();
  await expect(
    masthead(page),
    "the masthead follows the new title at once",
  ).toHaveText(renamed);
  const found = await onServer(page);
  expect(found.name).toBe(renamed);
  expect(found.language).toBe("nl");
  expect(found.description).toBe("Twee steden.");
});

test("Enter in the title saves the details", async ({ page }) => {
  await aNewProject(page, "Entering");
  const renamed = `Entered ${crypto.randomUUID().slice(0, 8)}`;

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill(renamed);
  await projectDetails(page).getByLabel("Title").press("Enter");

  await expect(projectDetails(page)).toBeHidden();
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

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Description").fill("Only this.");
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page)).toBeHidden();
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

  await openTheProjectDetails(page);
  await projectDetails(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "French (Français)" });
  await projectDetails(page).getByRole("button", { name: "Save" }).click();
  await changeTheMarks(page).click();

  await expect(book).toHaveAttribute("lang", "fr");
});

test("a title the server refuses is told inside the dialog, which stays", async ({
  page,
}) => {
  const title = await aNewProject(page, "Refused");

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill("a".repeat(201));
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page).getByRole("alert")).toHaveText(
    "A title can be at most 200 characters long; this one has 201.",
  );
  await expect(
    page.getByRole("alert"),
    "the problem is told beside the title it is about, nowhere else",
  ).toHaveCount(1);
  await expect(masthead(page)).toHaveText(title);

  await projectDetails(page).getByLabel("Title").fill("A shorter title");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a refusal the client cannot word is told as the server put it", async ({
  page,
}) => {
  await aNewProject(page, "Unworded");
  await page.route("**/api/projects/*", (route) =>
    route.request().method() === "PATCH"
      ? route.fulfill({
          status: 409,
          contentType: "application/problem+json",
          json: {
            type: "project-in-a-mood",
            status: 409,
            detail: "the project is not in the mood",
          },
        })
      : route.continue(),
  );

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill("Another title");
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page).getByRole("alert")).toHaveText(
    "the project is not in the mood",
  );
});

test("details changed elsewhere meanwhile are not overwritten", async ({
  page,
}) => {
  await aNewProject(page, "Elsewhere");
  await openTheProjectDetails(page);
  const meanwhile = await page.request.put(
    `${API}/projects/${projectOf(page)}/details`,
    { data: { description: "Written in another tab." } },
  );
  expect(meanwhile.ok()).toBe(true);

  await projectDetails(page).getByLabel("Description").fill("Written here.");
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page).getByRole("alert")).toContainText(
    "changed elsewhere meanwhile",
  );
  expect((await onServer(page)).description).toBe("Written in another tab.");
});

test("Escape leaves the details as they were", async ({ page }) => {
  const title = await aNewProject(page, "Escaping");

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill("Never kept");
  await page.keyboard.press("Escape");

  await expect(projectDetails(page)).toBeHidden();
  await expect(masthead(page)).toHaveText(title);
  await openTheProjectDetails(page);
  await expect(projectDetails(page).getByLabel("Title")).toHaveValue(title);
});

test("a blank title cannot be saved", async ({ page }) => {
  await aNewProject(page, "Blank");

  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill("   ");

  await expect(
    projectDetails(page).getByRole("button", { name: "Save" }),
  ).toBeDisabled();
});

test("a long name is cut short in the masthead, and told in full on hover", async ({
  page,
}) => {
  await aNewProject(page, "Long");
  const long = "Test ".repeat(30).trim();
  await openTheProjectDetails(page);
  await projectDetails(page).getByLabel("Title").fill(long);
  await projectDetails(page).getByRole("button", { name: "Save" }).click();
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
  await openTheProjectDetails(page);
  const marks = projectDetails(page).getByLabel("Quotation marks");
  await projectDetails(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "German (Deutsch)" });

  await expect(marks.locator("option").first()).toHaveText(
    "As the language has it, »…«",
  );
  await projectDetails(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "English" });
  await expect(marks.locator("option").first()).toHaveText(
    "As the language has it, “…”",
  );
  await projectDetails(page).getByRole("button", { name: "Save" }).click();
  await expect(projectDetails(page)).toBeHidden();

  const found = await onServer(page);
  expect(found.quotation, "following the language stores no style").toBe(null);
});

test("a quotation style chosen for the book is kept", async ({ page }) => {
  await aNewProject(page, "QuotingChosen");
  await openTheProjectDetails(page);

  await projectDetails(page)
    .getByLabel("Quotation marks")
    .selectOption({ label: "„…“" });
  await projectDetails(page).getByRole("button", { name: "Save" }).click();
  await changeTheMarks(page).click();
  await projectDetails(page).getByRole("button", { name: "Close" }).click();
  await expect(projectDetails(page)).toBeHidden();

  expect((await onServer(page)).quotation).toBe("low-high");
  await page.reload();
  await openTheProjectDetails(page);
  await expect(projectDetails(page).getByLabel("Quotation marks")).toHaveValue(
    "low-high",
  );
});

test("new quotation marks are asked about first, and Back keeps the details open", async ({
  page,
}) => {
  await aNewProject(page, "AskedFirst");
  await openTheProjectDetails(page);
  await projectDetails(page)
    .getByLabel("Quotation marks")
    .selectOption({ label: "„…“" });

  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(projectDetails(page)).toContainText(
    "Changing the quotation marks from “…” to „…“ rewrites them in every scene of the book.",
  );
  expect(
    (await onServer(page)).quotation,
    "nothing is saved before it is confirmed",
  ).toBe(null);

  await projectDetails(page).getByRole("button", { name: "Back" }).click();

  await expect(projectDetails(page).getByLabel("Quotation marks")).toHaveValue(
    "low-high",
  );
});

test("details that leave the marks as they look are saved without asking", async ({
  page,
}) => {
  await aNewProject(page, "NotAsked");
  await theBookIsIn(page, "de");
  await openTheProjectDetails(page);
  await projectDetails(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "Danish (Dansk)" });

  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await expect(
    projectDetails(page),
    "the marks stay »…«, so the dialog closes without asking",
  ).toBeHidden();
  expect((await onServer(page)).language).toBe("da");
});

test("confirmed, the dialog follows the book being rewritten until every scene is done", async ({
  page,
}) => {
  let tell: (event: object) => void = () => {};
  await page.routeWebSocket(/\/api\/scenes\/quotation-restylings\/live\?project=/, (socket) => {
    tell = (event) => socket.send(JSON.stringify({ version: 1, event }));
  });
  await aNewProject(page, "Followed");
  await aBookWithScenes(page, { "Part One": 3 });
  await page.route("**/api/scenes/quotation-restylings/*", (route) =>
    route.fulfill({
      json: { change: 1_000_000, total: 3, done: 1, finished: false },
    }),
  );
  await openTheProjectDetails(page);
  await projectDetails(page)
    .getByLabel("Quotation marks")
    .selectOption({ label: "»…«" });
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await changeTheMarks(page).click();

  const bar = projectDetails(page).getByRole("progressbar", {
    name: "Rewriting the quotation marks",
  });
  await expect(projectDetails(page)).toContainText(
    "Rewriting the quotation marks: 1 of 3 scenes.",
  );
  await expect(bar).toHaveAttribute("value", "1");
  await expect(bar).toHaveAttribute("max", "3");
  tell({ change: 1_000_000, total: 3, done: 2, finished: false });
  await expect(projectDetails(page)).toContainText(
    "Rewriting the quotation marks: 2 of 3 scenes.",
  );
  tell({ change: 1_000_000, total: 3, done: 3, finished: true });
  await expect(projectDetails(page)).toContainText(
    "Every scene now quotes »…«.",
  );
  await expect(bar).toHaveAttribute("value", "1");
  await expect(bar).toHaveAttribute("max", "1");
});

test("with nothing standing in for the server, the dialog follows a real rewriting to its end", async ({
  page,
}) => {
  await aNewProject(page, "FollowedForReal");
  await aBookWithScenes(page, { "Part One": 3 });
  await openTheProjectDetails(page);
  await projectDetails(page)
    .getByLabel("Quotation marks")
    .selectOption({ label: "»…«" });
  await projectDetails(page).getByRole("button", { name: "Save" }).click();

  await changeTheMarks(page).click();

  await expect(projectDetails(page)).toContainText(
    "Every scene now quotes »…«.",
  );
});
