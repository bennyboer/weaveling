import { test, expect, type Page } from "@playwright/test";

const aTitle = () => `The Loom ${crypto.randomUUID().slice(0, 8)}`;

const titleField = (page: Page) => page.getByPlaceholder("A working title…");

const newProject = (page: Page) =>
  page.getByRole("dialog", { name: "New project" });

const rowFor = (page: Page, title: string) =>
  page.getByRole("listitem").filter({ hasText: title });

const insideTheProject = (page: Page) =>
  expect(page).toHaveURL(/\/projects\/[a-z0-9-]+-project_/);

const backToTheList = (page: Page) =>
  page.getByRole("link", { name: "All projects" }).click();

async function aProjectCalled(page: Page, title: string) {
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();
  await insideTheProject(page);
  await backToTheList(page);
  await expect(rowFor(page, title)).toBeVisible();
}

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test("a new project opens at once, and the list has it when coming back", async ({
  page,
}) => {
  const title = aTitle();

  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  await insideTheProject(page);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(title);
  await expect(
    page.getByRole("article", { name: "The book" }),
    "a project opens on the writing view",
  ).toBeVisible();
  await expect(page).toHaveURL(/\/write$/);
  await backToTheList(page);
  await expect(rowFor(page, title)).toBeVisible();
});

test("Enter asks for the project's details, and Enter again creates it", async ({
  page,
}) => {
  const title = aTitle();

  await titleField(page).fill(title);
  await titleField(page).press("Enter");
  await expect(newProject(page).getByLabel("Title")).toBeFocused();
  await expect(newProject(page).getByLabel("Title")).toHaveValue(title);
  await page.keyboard.press("Enter");

  await insideTheProject(page);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(title);
});

test("coming back to the list after making a project stays on the list", async ({
  page,
}) => {
  const title = aTitle();

  await aProjectCalled(page, title);
  await page.evaluate(() => new Promise((settled) => setTimeout(settled, 300)));

  await expect(page).toHaveURL(/\/$/);
  await expect(titleField(page)).toHaveValue("");
});

test("a name the server rejects leaves the problem on screen", async ({
  page,
}) => {
  await titleField(page).fill("a".repeat(201));
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  await expect(newProject(page).getByRole("alert")).toContainText(
    "at most 200 characters",
  );
  await expect(
    page.getByRole("alert"),
    "the problem is told beside the title it is about, not behind the dialog",
  ).toHaveCount(1);
  await expect(
    newProject(page),
    "the dialog stays, so the title can be put right",
  ).toBeVisible();

  await newProject(page).getByLabel("Title").fill("A shorter title");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a project made while the server is out of reach says so", async ({
  page,
}) => {
  await page.route("**/api/projects", (route) =>
    route.request().method() === "POST" ? route.abort() : route.fallback(),
  );
  await titleField(page).fill("Unreachable");
  await page.getByRole("button", { name: "Create", exact: true }).click();

  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  await expect(newProject(page).getByRole("alert")).toContainText(
    "Could not reach the server. Is it running?",
  );
});

test("a list fetched while the server is out of reach says so", async ({
  page,
}) => {
  await page.route("**/api/projects", (route) => route.abort());

  await page.reload();

  await expect(page.getByRole("alert")).toContainText(
    "Could not reach the server. Is it running?",
  );
});

test("a blank title cannot be submitted", async ({ page }) => {
  await titleField(page).fill("   ");

  await expect(
    page.getByRole("button", { name: "Create", exact: true }),
  ).toBeDisabled();
});

test("a project can be renamed", async ({ page }) => {
  const title = aTitle();
  const rewoven = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;
  await aProjectCalled(page, title);

  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(rewoven);
  await page.getByRole("button", { name: "Save" }).click();

  await expect(rowFor(page, rewoven)).toBeVisible();
  await expect(rowFor(page, title)).toHaveCount(0);
});

test("deleting asks first, and cancelling keeps the project", async ({
  page,
}) => {
  const title = aTitle();
  await aProjectCalled(page, title);

  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await expect(page.getByRole("dialog")).toContainText("cannot be undone");
  await page.getByRole("button", { name: "Cancel" }).click();

  await expect(rowFor(page, title)).toBeVisible();
});

test("confirming a delete removes the project", async ({ page }) => {
  const title = aTitle();
  await aProjectCalled(page, title);

  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Delete" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Delete" })
    .click();

  await expect(rowFor(page, title)).toHaveCount(0);
});

test("a project's controls stay inside its row", async ({ page }) => {
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  const controls = row.locator(".actions");
  await expect(controls).toHaveCSS("position", "relative");
  const inside = await controls.evaluate((it) => {
    const mine = it.getBoundingClientRect();
    const around = it.closest("li")!.getBoundingClientRect();

    return mine.top >= around.top - 1 && mine.bottom <= around.bottom + 1;
  });
  expect(inside, "board styling must not float these over the page").toBe(true);
});

test("renaming a project shows Save and Cancel beside the field", async ({
  page,
}) => {
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  await row.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();

  const editing = page
    .getByRole("listitem")
    .filter({ has: page.getByRole("textbox") });
  await expect(editing.getByRole("button", { name: "Save" })).toBeVisible();
  await expect(editing.getByRole("button", { name: "Cancel" })).toBeVisible();

  const apart = await editing.evaluate((it) => {
    const [first, second] = [...it.querySelectorAll(".actions button")].map(
      (b) => b.getBoundingClientRect(),
    );

    return second.left >= first.right - 1;
  });
  expect(apart, "Save and Cancel must not sit on top of each other").toBe(true);
});

test("a project row spreads its title, stamp and menu across the width", async ({
  page,
}) => {
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  const laid = await row.evaluate((it) => {
    const around = it.getBoundingClientRect();
    const name = it.querySelector(".name")!.getBoundingClientRect();
    const stamp = it.querySelector(".stamp")!.getBoundingClientRect();
    const menu = it.querySelector(".actions")!.getBoundingClientRect();

    return {
      titleSpans: Math.round(name.width / around.width),
      stampAfterTitle: Math.round(stamp.left - name.right) >= 0,
      menuLast: Math.round(around.right - menu.right) < 8,
    };
  });

  expect(laid.stampAfterTitle, "the stamp follows the title").toBe(true);
  expect(laid.menuLast, "the menu sits at the far end of the row").toBe(true);
  expect(
    laid.titleSpans,
    "the title takes the room the other two leave",
  ).toBeGreaterThan(0);
});

test("a new project is given its book's language and a description", async ({
  page,
}) => {
  const title = aTitle();
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();

  await newProject(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "German (Deutsch)" });
  await newProject(page)
    .getByLabel("Description")
    .fill("A weaver's daughter, and a war.");
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  await insideTheProject(page);
  let made: { id: string } | undefined;
  await expect
    .poll(async () => {
      const listed = await (await page.request.get("/api/projects")).json();
      made = listed.find((one: { name: string }) => one.name === title);

      return made !== undefined;
    })
    .toBe(true);
  const found = await (
    await page.request.get(`/api/projects/${made!.id}`)
  ).json();
  expect(found.language).toBe("de");
  expect(found.description).toBe("A weaver's daughter, and a war.");
});

test("the language offered at first is the browser's own", async ({ page }) => {
  await titleField(page).fill(aTitle());
  await page.getByRole("button", { name: "Create", exact: true }).click();

  await expect(newProject(page).getByLabel("Language of the book")).toHaveValue(
    "en",
  );
});

test("cancelling the new project's details creates nothing and keeps the title", async ({
  page,
}) => {
  const title = aTitle();
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();

  await page.keyboard.press("Escape");

  await expect(newProject(page)).toBeHidden();
  await expect(titleField(page)).toHaveValue(title);
  await expect(rowFor(page, title)).toHaveCount(0);
});
