import { test, expect, type Page } from "@playwright/test";
import { routingLive } from "./support/live";
import { API } from "./support/shell";

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
  // given: a working title
  const title = aTitle();

  // when: a project is created by it
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: it opens at once on the writing view
  await insideTheProject(page);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(title);
  await expect(
    page.getByRole("article", { name: "The book" }),
    "a project opens on the writing view",
  ).toBeVisible();
  await expect(page).toHaveURL(/\/write$/);
  // when: the list is gone back to
  await backToTheList(page);
  // then: it has the project
  await expect(rowFor(page, title)).toBeVisible();
});

test("Enter asks for the project's details, and Enter again creates it", async ({
  page,
}) => {
  // given: a working title
  const title = aTitle();

  // when: enter is pressed
  await titleField(page).fill(title);
  await titleField(page).press("Enter");
  // then: the project's details are asked for
  await expect(newProject(page).getByLabel("Title")).toBeFocused();
  await expect(newProject(page).getByLabel("Title")).toHaveValue(title);
  // when: enter is pressed again
  await page.keyboard.press("Enter");

  // then: the project is created and opened
  await insideTheProject(page);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(title);
});

test("coming back to the list after making a project stays on the list", async ({
  page,
}) => {
  // given: a working title
  const title = aTitle();

  // when: a project is made by it and the list gone back to
  await aProjectCalled(page, title);
  await page.evaluate(() => new Promise((settled) => setTimeout(settled, 300)));

  // then: the list stays, the title field empty
  await expect(page).toHaveURL(/\/$/);
  await expect(titleField(page)).toHaveValue("");
});

test("a name the server rejects leaves the problem on screen", async ({
  page,
}) => {
  // when: a project is created with a title too long
  await titleField(page).fill("a".repeat(201));
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: the problem is told in the dialog alone
  await expect(newProject(page).getByRole("alert")).toContainText(
    "at most 200 characters",
  );
  await expect(
    page.getByRole("alert"),
    "the problem is told beside the title it is about, not behind the dialog",
  ).toHaveCount(1);
  // and: the dialog stays
  await expect(
    newProject(page),
    "the dialog stays, so the title can be put right",
  ).toBeVisible();

  // when: the title is shortened
  await newProject(page).getByLabel("Title").fill("A shorter title");
  // then: the problem goes
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a project made while the server is out of reach says so", async ({
  page,
}) => {
  // given: the server out of reach
  await page.route("**/api/projects", (route) =>
    route.request().method() === "POST" ? route.abort() : route.fallback(),
  );
  // and: a project's details asked for
  await titleField(page).fill("Unreachable");
  await page.getByRole("button", { name: "Create", exact: true }).click();

  // when: it is created
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: it says so
  await expect(newProject(page).getByRole("alert")).toContainText(
    "Could not reach the server. Please try again in a moment.",
  );
});

test("a list fetched while the server is out of reach says so", async ({
  page,
}) => {
  // given: the server out of reach
  await page.route("**/api/projects", (route) => route.abort());

  // when: the list is fetched
  await page.reload();

  // then: it says so
  await expect(page.getByRole("alert")).toContainText(
    "Could not reach the server. Please try again in a moment.",
  );
});

test("a blank title cannot be submitted", async ({ page }) => {
  // when: a blank title is typed
  await titleField(page).fill("   ");

  // then: it cannot be submitted
  await expect(
    page.getByRole("button", { name: "Create", exact: true }),
  ).toBeDisabled();
});

test("a project can be renamed", async ({ page }) => {
  // given: a project in the list
  const title = aTitle();
  const rewoven = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;
  await aProjectCalled(page, title);

  // when: it is renamed
  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(rewoven);
  await page.getByRole("button", { name: "Save" }).click();

  // then: the list has it under its new name
  await expect(rowFor(page, rewoven)).toBeVisible();
  await expect(rowFor(page, title)).toHaveCount(0);
});

test("moving to the trash asks first, and cancelling keeps the project", async ({
  page,
}) => {
  // given: a project in the list
  const title = aTitle();
  await aProjectCalled(page, title);

  // when: it is moved to the trash
  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Move to trash" }).click();
  // then: confirming is asked first
  await expect(page.getByRole("dialog", { name: "Move this project to the trash?" })).toContainText(
    `“${title}” can be restored from the trash for 30 days. Then it is gone for good.`,
  );
  // when: it is cancelled
  await page.getByRole("button", { name: "Cancel" }).click();

  // then: the project stays
  await expect(rowFor(page, title)).toBeVisible();
});

test("confirming moves the project to the trash, from where it is restored", async ({
  page,
}) => {
  // given: a project in the list
  const title = aTitle();
  await aProjectCalled(page, title);

  // when: it is moved to the trash and confirmed
  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Move to trash" }).click();
  await page
    .getByRole("dialog")
    .getByRole("button", { name: "Move to trash" })
    .click();

  // then: it leaves the list
  await expect(rowFor(page, title)).toHaveCount(0);
  // and: the way to the trash sits in the line of the heading
  const toTheTrash = page.getByRole("link", { name: /^Trash, \d+ projects?$/ });
  await expect(
    page
      .locator(".workspace-heading")
      .filter({ has: page.getByRole("heading", { name: "Your projects" }) })
      .getByRole("link", { name: /^Trash, \d+ projects?$/ }),
    "the way to the trash sits in the line of the heading",
  ).toBeVisible();
  await expect(toTheTrash).toHaveAttribute("title", /^Trash, \d+ projects?$/);
  // when: the trash is opened
  await toTheTrash.click();
  // then: the project is there, restorable
  await expect(page).toHaveURL(/\/trash$/);
  const trashed = page
    .getByRole("list", { name: "Trash" })
    .getByRole("listitem")
    .filter({ hasText: title });
  await expect(trashed).toContainText("Can be restored until");

  // when: it is restored
  await trashed.getByRole("button", { name: "Restore" }).click();

  // then: it leaves the trash
  await expect(trashed).toHaveCount(0);
  // and: is back in the list
  await page.getByRole("navigation", { name: "Path" }).getByRole("link", { name: "Your projects" }).click();
  await expect(rowFor(page, title)).toBeVisible();
});

test("a project's controls stay inside its row", async ({ page }) => {
  // given: a project in the list
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  // then: its controls stay inside its row
  const controls = row.locator(".actions");
  await expect(controls).toHaveCSS("position", "relative");
  const inside = await controls.evaluate((it) => {
    const mine = it.getBoundingClientRect();
    const around = it.closest("li")!.getBoundingClientRect();

    return mine.top >= around.top - 1 && mine.bottom <= around.bottom + 1;
  });
  expect(inside, "board styling must not float these over the page").toBe(true);
});

async function renaming(page: Page, from: string, to: string) {
  await rowFor(page, from)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(to);
  await page.getByRole("button", { name: "Save" }).click();
}

const versionIn = (tag: string | undefined) =>
  Number((tag ?? "").replace(/"/g, ""));

test("a project renamed twice close together keeps the later name, sent after the first is answered", async ({
  page,
}) => {
  // given: a project in the list
  const title = aTitle();
  const first = `Warp ${crypto.randomUUID().slice(0, 8)}`;
  const second = `Weft ${crypto.randomUUID().slice(0, 8)}`;
  await aProjectCalled(page, title);
  // and: the answer to the first rename held back
  const sentOn: (string | undefined)[] = [];
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let holding = true;
  await page.route(
    (url) => /^\/api\/projects\/[^/]+$/.test(url.pathname),
    async (route) => {
      if (route.request().method() !== "PATCH") {
        return route.fallback();
      }
      sentOn.push(route.request().headers()["if-match"]);
      const answer = await route.fetch();
      if (holding) {
        holding = false;
        await held;
      }
      await route.fulfill({ response: answer });
    },
  );

  // when: it is renamed
  await renaming(page, title, first);
  await expect.poll(() => sentOn.length).toBe(1);
  // and: renamed again before the first is answered
  await renaming(page, first, second);

  // then: the later name shows at once
  await expect(rowFor(page, second)).toBeVisible();
  // and: nothing more is sent while the first is on its way
  await page.waitForTimeout(300);
  expect(sentOn, "one change at a time goes out per project").toHaveLength(1);

  // when: the first is answered
  release();

  // then: the later name is sent on the version the first left
  await expect.poll(() => sentOn.length).toBe(2);
  expect(versionIn(sentOn[1])).toBe(versionIn(sentOn[0]) + 1);
  // and: the list keeps the later name
  await expect(rowFor(page, second)).toBeVisible();
  await expect(rowFor(page, first)).toHaveCount(0);
  // and: so does the server
  await expect
    .poll(async () =>
      (await (await page.request.get(`${API}/projects`)).json()).some(
        (project: { name: string }) => project.name === second,
      ),
    )
    .toBe(true);
});

test("renaming a project shows Save and Cancel beside the field", async ({
  page,
}) => {
  // given: a project in the list
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  // when: it is renamed
  await row.getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();

  // then: save and cancel show beside the field, apart
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
  // given: a project in the list
  const title = aTitle();
  await aProjectCalled(page, title);
  const row = rowFor(page, title);

  // then: its row spreads title, stamp and menu across the width
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
  // given: a project's details asked for
  const title = aTitle();
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();

  // when: it is created with a language and a description
  await newProject(page)
    .getByLabel("Language of the book")
    .selectOption({ label: "German (Deutsch)" });
  await newProject(page)
    .getByLabel("Description")
    .fill("A weaver's daughter, and a war.");
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: the project holds both
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
  // when: a project's details are asked for
  await titleField(page).fill(aTitle());
  await page.getByRole("button", { name: "Create", exact: true }).click();

  // then: the browser's language is offered
  await expect(newProject(page).getByLabel("Language of the book")).toHaveValue(
    "en",
  );
});

test("cancelling the new project's details creates nothing and keeps the title", async ({
  page,
}) => {
  // given: a project's details asked for
  const title = aTitle();
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();

  // when: they are cancelled
  await page.keyboard.press("Escape");

  // then: nothing is created, the title kept
  await expect(newProject(page)).toBeHidden();
  await expect(titleField(page)).toHaveValue(title);
  await expect(rowFor(page, title)).toHaveCount(0);
});

async function notFoundAtFirst(page: Page, times: number) {
  let missing = times;
  let made: string | null = null;
  await page.route(
    (url) => url.pathname === "/api/projects",
    async (route) => {
      if (route.request().method() !== "POST") {
        return route.fallback();
      }
      const answered = await route.fetch();
      made = (await answered.json()).id;
      return route.fulfill({ response: answered });
    },
  );
  await page.route(
    (url) => made !== null && url.pathname === `/api/projects/${made}`,
    (route) =>
      route.request().method() === "GET" && missing-- > 0
        ? route.fulfill({
            status: 404,
            contentType: "application/problem+json",
            body: JSON.stringify({
              type: "not-found",
              status: 404,
              detail: "not yet",
            }),
          })
        : route.fallback(),
  );
}

test("a new project that cannot be opened at once is waited for while it is set up", async ({
  page,
}) => {
  // given: a new project that cannot be opened at once
  await notFoundAtFirst(page, 5);
  // and: its details asked for
  const title = aTitle();

  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  // then: nothing is said to be set up yet
  await expect(
    newProject(page).getByRole("status"),
    "nothing is set up before it is asked for",
  ).toHaveCount(0);
  // when: it is created
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: it is waited for while it is set up, not made twice
  await expect(newProject(page).getByRole("status")).toHaveText(
    "Setting up your project…",
  );
  await expect(
    newProject(page).getByRole("button", { name: "Create project" }),
    "it is not made twice while it is waited for",
  ).toBeDisabled();
  // and: then opened
  await insideTheProject(page);
});

test("a new project that never opens says so, and is in the list all the same", async ({
  page,
}) => {
  // given: a new project that never opens
  test.setTimeout(30_000);
  await notFoundAtFirst(page, 1_000);
  await routingLive(page, { withheld: ["projects"] });
  await page.goto("/");
  const title = aTitle();

  // when: it is created
  await titleField(page).fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await newProject(page)
    .getByRole("button", { name: "Create project" })
    .click();

  // then: it says so
  await expect(newProject(page).getByRole("alert")).toContainText(
    "cannot be opened yet",
    { timeout: 15_000 },
  );
  await expect(newProject(page).getByRole("status")).toHaveCount(0);
  await expect(page).toHaveURL(/\/$/);
  // and: the list has it all the same
  await newProject(page).getByRole("button", { name: "Cancel" }).click();
  await expect(
    rowFor(page, title),
    "listed as soon as it is made, before any channel tells of it",
  ).toBeVisible();
});
