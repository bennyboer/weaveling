import { test, expect, type Page } from "@playwright/test";

import { API } from "./support/shell";

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
    if (request.method() === "GET" && new URL(request.url()).pathname === "/api/projects") {
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
  await page.routeWebSocket(/\/api\/projects\/live$/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      const told = JSON.parse(String(frame));
      if (told.event?.name === "STARTED" && String(told.event.project_name).startsWith(named)) {
        mine.add(told.of);
      }
      if (!mine.has(told.of)) {
        return;
      }
      passed += 1;
      if (passed !== dropping) {
        socket.send(frame);
      }
    });
  });
  const asks = listed(page);
  await page.goto("/");
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return { asks, mine, passed: () => passed };
}

test("a project another author starts joins the list without the list asked for again", async ({
  page,
}) => {
  const title = aTitle("Started");
  const { asks } = await aWorkspaceHearingOnly(page, title);

  await aProject(page, title);

  await expect(rowFor(page, title)).toBeVisible();
  expect(asks()).toBe(1);
});

test("a project another author renames or deletes changes in the list as it happens", async ({
  page,
}) => {
  const title = aTitle("Changed");
  const renamed = aTitle("Rewoven");
  const project = await aProject(page, title);
  const { asks, mine } = await aWorkspaceHearingOnly(page, title);
  mine.add(project);
  await expect(rowFor(page, title)).toBeVisible();

  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });
  await expect(rowFor(page, renamed)).toBeVisible();
  await anotherAuthor(page, "delete", `/projects/${project}`);

  await expect(rowFor(page, renamed)).toHaveCount(0);
  expect(asks()).toBe(1);
});

test("a change missed on the way asks for the list again, and the list comes out right", async ({
  page,
}) => {
  const title = aTitle("Missed");
  const renamed = aTitle("Rewoven");
  const { asks, passed } = await aWorkspaceHearingOnly(page, title, { dropping: 1 });

  const project = await aProject(page, title);
  await expect.poll(passed).toBeGreaterThanOrEqual(1);
  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });

  await expect(
    rowFor(page, renamed),
    "the start that was missed is there too, so the list was asked for again",
  ).toBeVisible();
  expect(asks()).toBeGreaterThan(1);
});

test("a rename answered after a newer one was told leaves the newer name", async ({
  page,
}) => {
  const title = aTitle("Overtaken");
  const mine = aTitle("Mine");
  const theirs = aTitle("Theirs");
  const project = await aProject(page, title);
  const { passed, mine: heard } = await aWorkspaceHearingOnly(page, title);
  heard.add(project);
  await page.route(new RegExp(`/api/projects/${project}$`), async (route) => {
    if (route.request().method() !== "PATCH") {
      await route.continue();
      return;
    }
    const response = await route.fetch();
    await anotherAuthor(page, "patch", `/projects/${project}`, { name: theirs });
    await expect.poll(passed).toBe(2);
    await route.fulfill({ response });
  });

  await rowFor(page, title).getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  await page.getByRole("listitem").getByRole("textbox").fill(mine);
  await page.getByRole("button", { name: "Save" }).click();

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
  await rowFor(page, loom).getByRole("button", { name: "More actions" }).click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  const field = page.getByRole("listitem").getByRole("textbox");
  await field.fill(typed);

  await anotherAuthor(page, "patch", `/projects/${other}`, { name: theirs });
  await expect(rowFor(page, theirs)).toBeVisible();

  await expect(field, "another project's change leaves this row as it is").toHaveValue(typed);
  await page.getByRole("button", { name: "Save" }).click();
  await expect(rowFor(page, typed)).toBeVisible();
});

test("a change told while the list is still on its way asks for it again", async ({
  page,
}) => {
  const title = aTitle("InFlight");
  const renamed = aTitle("Rewoven");
  const project = await aProject(page, title);
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
  await page.routeWebSocket(/\/api\/projects\/live$/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      if (mine.has(JSON.parse(String(frame)).of)) {
        passed += 1;
        socket.send(frame);
      }
    });
  });
  await page.goto("/");
  await expect.poll(() => answered).toBe(true);

  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });
  await expect.poll(() => passed).toBe(1);
  release();

  await expect(
    rowFor(page, renamed),
    "the first answer was read before the rename, and the rename was told before it arrived",
  ).toBeVisible();
});
