import { test, expect, type Page } from "@playwright/test";

import { API } from "./support/shell";
import { withAccounts } from "./support/signed-in";
import { routingLive } from "./support/live";

const aTitle = (of: string) => `${of} ${crypto.randomUUID().slice(0, 8)}`;

const rowFor = (page: Page, title: string) =>
  page.getByRole("listitem").filter({ hasText: title });

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "delete",
  path: string,
  data?: object,
) {
  const done = await page.request[method](`${API}${path}`, { data });
  expect(done.ok(), await done.text()).toBe(true);

  return done;
}

async function aProject(page: Page, name: string) {
  const started = await anotherAuthor(page, "post", "/projects", {
    name,
    language: "en",
  });

  return (await started.json()).id as string;
}

function listed(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    if (
      request.method() === "GET" &&
      new URL(request.url()).pathname === "/api/projects"
    ) {
      times += 1;
    }
  });

  return () => times;
}

async function aWorkspaceHearingOnly(
  page: Page,
  named: string,
  { dropping = 0 } = {},
) {
  const mine = new Set<string>();
  let passed = 0;
  await routingLive(page, {
    heard: {
      projects: (told, pass, about) => {
        if (about.mine !== "true") {
          pass();
          return;
        }
        const { frame } = JSON.parse(told);
        if (
          frame.event?.name === "STARTED" &&
          String(frame.event.project_name).startsWith(named)
        ) {
          mine.add(frame.of);
        }
        if (!mine.has(frame.of)) {
          return;
        }
        passed += 1;
        if (passed !== dropping) {
          pass();
        }
      },
    },
  });
  const asks = listed(page);
  await page.goto("/");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);
  await expect(
    page
      .getByRole("listitem")
      .first()
      .or(page.getByText("No projects yet.", { exact: false })),
    "the list has arrived, so a change told now is not one told while it is on its way",
  ).toBeVisible();

  return { asks, mine, passed: () => passed };
}

test("a project another author starts joins the list without the list asked for again", async ({
  page,
}) => {
  // given: the workspace followed live
  const title = aTitle("Started");
  const { asks } = await aWorkspaceHearingOnly(page, title);

  // when: another author starts a project
  await aProject(page, title);

  // then: it joins the list
  await expect(rowFor(page, title)).toBeVisible();
  // and: the list is not asked for again
  expect(asks()).toBe(1);
});

test("a project another author renames or deletes changes in the list as it happens", async ({
  page,
}) => {
  // given: the workspace followed live, a project listed
  const title = aTitle("Changed");
  const renamed = aTitle("Rewoven");
  const project = await aProject(page, title);
  const { asks, mine } = await aWorkspaceHearingOnly(page, title);
  mine.add(project);
  await expect(rowFor(page, title)).toBeVisible();

  // when: another author renames it
  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });
  // then: it is renamed in the list
  await expect(rowFor(page, renamed)).toBeVisible();
  // when: they delete it
  await anotherAuthor(page, "delete", `/projects/${project}`);

  // then: it leaves the list
  await expect(rowFor(page, renamed)).toHaveCount(0);
  // and: the list was not asked for again
  expect(asks()).toBe(1);
});

test("a change missed on the way asks for the list again, and the list comes out right", async ({
  page,
}) => {
  // given: the workspace followed live, missing the next change
  test.skip(
    withAccounts,
    "with accounts a project's start is not told on this channel, its owner not listed yet; the members' channel tells it",
  );
  const title = aTitle("Missed");
  const renamed = aTitle("Rewoven");
  const { asks, passed } = await aWorkspaceHearingOnly(page, title, {
    dropping: 1,
  });

  // when: another author starts a project and renames it
  const project = await aProject(page, title);
  await expect.poll(passed).toBeGreaterThanOrEqual(1);
  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });

  // then: it is listed under its new name
  await expect(
    rowFor(page, renamed),
    "the start that was missed is there too, so the list was asked for again",
  ).toBeVisible();
  // and: the list was asked for again
  expect(asks()).toBeGreaterThan(1);
});

test("a rename answered after a newer one was told leaves the newer name", async ({
  page,
}) => {
  // given: the workspace followed live, a project listed
  const title = aTitle("Overtaken");
  const mine = aTitle("Mine");
  const theirs = aTitle("Theirs");
  const project = await aProject(page, title);
  const { passed, mine: heard } = await aWorkspaceHearingOnly(page, title);
  heard.add(project);
  // and: another author renaming it while my rename is answered
  await page.route(new RegExp(`/api/projects/${project}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    await anotherAuthor(page, "patch", `/projects/${project}`, {
      name: theirs,
    });
    await expect.poll(passed).toBe(2);
    await route.fulfill({ response });
  });

  // when: it is renamed here
  await rowFor(page, title)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(mine);
  await page.getByRole("button", { name: "Save" }).click();

  // then: it keeps their newer name
  await expect(rowFor(page, theirs)).toBeVisible();
  await page.waitForTimeout(300);
  await expect(
    rowFor(page, mine),
    "my rename's answer is older than theirs, told before it came",
  ).toHaveCount(0);
});

test("a row being renamed keeps what is typed while another project changes", async ({
  page,
}) => {
  // given: the workspace followed live, a project of it being renamed
  const title = aTitle("Typing");
  const loom = `${title} Loom`;
  const weft = `${title} Weft`;
  const typed = aTitle("Typed");
  const theirs = aTitle("Theirs");
  const ours = await aProject(page, loom);
  const other = await aProject(page, weft);
  const { mine } = await aWorkspaceHearingOnly(page, title);
  mine.add(ours);
  mine.add(other);
  await rowFor(page, loom)
    .getByRole("button", { name: "More actions" })
    .click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  const field = page.getByRole("listitem").getByRole("textbox");
  await field.fill(typed);

  // when: another author renames another project
  await anotherAuthor(page, "patch", `/projects/${other}`, { name: theirs });
  await expect(rowFor(page, theirs)).toBeVisible();

  // then: what is typed stays
  await expect(
    field,
    "another project's change leaves this row as it is",
  ).toHaveValue(typed);
  // and: is saved
  await page.getByRole("button", { name: "Save" }).click();
  await expect(rowFor(page, typed)).toBeVisible();
});

test("a change told while the list is still on its way asks for it again", async ({
  page,
}) => {
  // given: a project
  const title = aTitle("InFlight");
  const renamed = aTitle("Rewoven");
  const project = await aProject(page, title);
  // and: the workspace opened, its list still on its way
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answered = false;
  await page.route(/\/api\/projects$/, async (route) => {
    if (route.request().method() !== "GET") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    if (!answered) {
      answered = true;
      await held;
    }
    await route.fulfill({ response });
  });
  const mine = new Set([project]);
  let passed = 0;
  await routingLive(page, {
    heard: {
      projects: (told, pass, about) => {
        if (about.mine !== "true") {
          pass();
          return;
        }
        if (mine.has(JSON.parse(told).frame.of)) {
          passed += 1;
          pass();
        }
      },
    },
  });
  await page.goto("/");
  await expect.poll(() => answered).toBe(true);

  // when: another author renames it before the list arrives
  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });
  await expect.poll(() => passed).toBe(1);
  release();

  // then: it is listed under its new name
  await expect(
    rowFor(page, renamed),
    "the first answer was read before the rename, and the rename was told before it arrived",
  ).toBeVisible();
});

test("with accounts a project one starts is told on the page's one socket, under its own subscription", async ({
  page,
}) => {
  // given: the workspace followed live, its socket watched
  test.skip(!withAccounts, "the members are followed only where there are accounts");
  const told: string[] = [];
  const asked: string[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().endsWith("/api/live")) {
      socket.on("framesent", (frame) => asked.push(String(frame.payload)));
      socket.on("framereceived", (frame) => told.push(String(frame.payload)));
    }
  });
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Your projects" })).toBeVisible();
  await expect
    .poll(
      () => asked.map((said) => JSON.parse(said)).some((said) => said.topic === "members"),
      { message: "subscribed before starting" },
    )
    .toBe(true);

  // when: a project is started
  const made = await page.request.post(`${API}/projects`, {
    data: { name: aTitle("One socket"), language: "en" },
  });
  const project = (await made.json()).id;

  // then: its founding is told on that socket
  await expect
    .poll(() =>
      told
        .map((said) => JSON.parse(said))
        .find((said) => said.told === "frame" && said.frame.of === project),
    )
    .toMatchObject({ frame: { event: { name: "FOUNDED" } } });
});

