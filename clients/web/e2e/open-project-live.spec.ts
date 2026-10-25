import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API } from "./support/shell";
import { flow, openTheWritingView, projectOf } from "./support/writing";

const masthead = (page: Page) => page.getByRole("heading", { level: 1 });

async function anotherAuthor(
  page: Page,
  method: "patch" | "put" | "delete",
  path: string,
  data?: object,
  headers?: Record<string, string>,
) {
  const done = await page.request[method](`${API}${path}`, { data, headers });
  expect(done.ok(), await done.text()).toBe(true);

  return done;
}

function described(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    const path = new URL(request.url()).pathname;
    const open = new URL(page.url()).pathname.split("/")[2]?.split("-").pop();
    if (
      request.method() === "GET" &&
      open &&
      path === `/api/projects/${open}`
    ) {
      times += 1;
    }
  });

  return () => times;
}

async function anOpenProjectFollowedLive(page: Page, named: string) {
  const asks = described(page);
  const followed = page.waitForEvent("websocket", (socket) =>
    socket.url().includes("/api/projects/live?project="),
  );
  const title = await aNewProject(page, named);
  await followed;
  await expect(masthead(page)).toHaveText(title);
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return { title, asks, project: projectOf(page) };
}

test("the open project's name follows another author's rename without asking again", async ({
  page,
}) => {
  const { asks, project } = await anOpenProjectFollowedLive(
    page,
    "OpenRenamed",
  );
  const renamed = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;

  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });

  await expect(masthead(page)).toHaveText(renamed);
  expect(asks()).toBe(1);
});

test("the book's language follows another author's change of the details", async ({
  page,
}) => {
  const { asks, project } = await anOpenProjectFollowedLive(
    page,
    "OpenDetails",
  );
  await openTheWritingView(page);
  await expect(flow(page)).toHaveAttribute("lang", "en");
  const now = await (
    await page.request.get(`${API}/projects/${project}`)
  ).json();

  await anotherAuthor(
    page,
    "put",
    `/projects/${project}/details`,
    { language: "de", quotation: null, description: "" },
    { "If-Match": `"${now.version}"` },
  );

  await expect(flow(page)).toHaveAttribute("lang", "de");
  expect(asks()).toBe(1);
});

test("an open project another author deletes says so, and is followed no longer", async ({
  page,
}) => {
  let closed = false;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/projects/live?project=")) {
      socket.on("close", () => {
        closed = true;
      });
    }
  });
  const { project } = await anOpenProjectFollowedLive(page, "OpenDeleted");
  await openTheWritingView(page);

  await anotherAuthor(page, "delete", `/projects/${project}`);

  await expect(page.getByRole("alert")).toContainText(
    "That project no longer exists.",
  );
  await expect
    .poll(() => closed, {
      message: "a project that is gone has nothing more to tell",
    })
    .toBe(true);
});

test("a change missed on the way asks for the project again, and it comes out right", async ({
  page,
}) => {
  let passed = 0;
  await page.routeWebSocket(/\/api\/projects\/live\?project=/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      passed += 1;
      if (passed !== 1) {
        socket.send(frame);
      }
    });
  });
  const asks = described(page);
  const title = await aNewProject(page, "OpenMissed");
  await expect(masthead(page)).toHaveText(title);
  await expect.poll(asks).toBe(1);
  const project = projectOf(page);
  const missed = `Missed ${crypto.randomUUID().slice(0, 8)}`;
  const renamed = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;

  await anotherAuthor(page, "patch", `/projects/${project}`, { name: missed });
  await expect.poll(() => passed).toBe(1);
  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });

  await expect(masthead(page)).toHaveText(renamed);
  expect(
    asks(),
    "the rename after the missed one does not follow on, so it asks again",
  ).toBe(2);
});

test("a change told while the project is still on its way asks for it again", async ({
  page,
}) => {
  const title = await aNewProject(page, "OpenInFlight");
  const project = projectOf(page);
  const renamed = `Rewoven ${crypto.randomUUID().slice(0, 8)}`;
  let heard = 0;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/projects/live?project=")) {
      socket.on("framereceived", () => {
        heard += 1;
      });
    }
  });
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let answered = false;
  await page.route(new RegExp(`/api/projects/${project}$`), async (route) => {
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
  await page.reload();
  await expect.poll(() => answered).toBe(true);

  await anotherAuthor(page, "patch", `/projects/${project}`, { name: renamed });
  await expect.poll(() => heard).toBe(1);
  release();

  await expect(
    masthead(page),
    "the first answer was read before the rename, and the rename was told before it arrived",
  ).toHaveText(renamed);
  expect(title).not.toBe(renamed);
});

test("my details answered after another author's newer rename leave their name", async ({
  page,
}) => {
  const { project } = await anOpenProjectFollowedLive(page, "OpenOvertaken");
  const theirs = `Theirs ${crypto.randomUUID().slice(0, 8)}`;
  let heard = 0;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/projects/live?project=")) {
      socket.on("framereceived", () => {
        heard += 1;
      });
    }
  });
  await page.reload();
  await expect.poll(() => heard).toBe(0);
  await page.route(
    new RegExp(`/api/projects/${project}/details$`),
    async (route) => {
      const response = await route.fetch();
      await expect.poll(() => heard).toBe(1);
      await anotherAuthor(page, "patch", `/projects/${project}`, {
        name: theirs,
      });
      await expect.poll(() => heard).toBe(2);
      await route.fulfill({ response });
    },
  );

  await page.getByRole("button", { name: "Project details" }).click();
  const details = page.getByRole("dialog", { name: "Project details" });
  await details.getByLabel("Description").fill("Two cities.");
  await details.getByRole("button", { name: "Save" }).click();

  await expect(details).toBeHidden();
  await expect(
    masthead(page),
    "my answer is older than their rename, told before it came",
  ).toHaveText(theirs);
});
