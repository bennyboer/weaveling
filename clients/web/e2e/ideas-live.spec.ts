import { test, expect, type Page } from "@playwright/test";

import { aNewProject, API, catalogued, openThePool } from "./support/shell";
import { projectOf } from "./support/writing";

const ideas = (page: Page) => page.getByRole("list", { name: "Ideas" });

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

async function anIdea(page: Page, title: string) {
  const captured = await anotherAuthor(page, "post", "/ideas", {
    project: projectOf(page),
    title,
  });

  return (await captured.json()).id as string;
}

function asked(page: Page): () => number {
  let times = 0;
  page.on("request", (request) => {
    if (request.method() === "GET" && /\/api\/ideas\?project=/.test(request.url())) {
      times += 1;
    }
  });

  return () => times;
}

async function aPoolFollowedLive(page: Page) {
  const asks = asked(page);
  let heard = 0;
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/ideas/live?project=")) {
      socket.on("framereceived", () => {
        heard += 1;
      });
    }
  });
  const followed = page.waitForEvent("websocket", (socket) =>
    socket.url().includes("/api/ideas/live?project="),
  );
  await openThePool(page);
  await followed;
  await expect.poll(asks, { message: "asked once, as it connects" }).toBe(1);

  return { asks, heard: () => heard };
}

test("an idea another author captures joins the pool without the ideas asked for again", async ({
  page,
}) => {
  await aNewProject(page, "PoolLiveCaptured");
  const { asks } = await aPoolFollowedLive(page);

  await anIdea(page, "The loom remembers");

  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();
  expect(asks()).toBe(1);
});

test("an idea another author renames or discards changes in the pool as it happens", async ({
  page,
}) => {
  await aNewProject(page, "PoolLiveChanged");
  const loom = await anIdea(page, "The loom remembers");
  await catalogued(page, ["The loom remembers"]);
  const { asks } = await aPoolFollowedLive(page);
  await expect(ideas(page).getByText("The loom remembers")).toBeVisible();

  await anotherAuthor(page, "patch", `/ideas/${loom}`, { title: "The weft" });
  await expect(ideas(page).getByText("The weft")).toBeVisible();
  await anotherAuthor(page, "delete", `/ideas/${loom}`);

  await expect(ideas(page).getByText("The weft")).toHaveCount(0);
  expect(asks()).toBe(1);
});

test("my own capture told back to me is not shown twice", async ({ page }) => {
  await aNewProject(page, "PoolLiveMine");
  const { heard } = await aPoolFollowedLive(page);

  await page.getByRole("textbox", { name: "What is the idea?" }).fill("The shuttle");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await expect.poll(heard).toBe(1);

  await expect(ideas(page).getByText("The shuttle")).toHaveCount(1);
});

test("a change missed on the way asks for the ideas again, and the pool comes out right", async ({
  page,
}) => {
  let passed = 0;
  await page.routeWebSocket(/\/api\/ideas\/live\?project=/, (socket) => {
    const server = socket.connectToServer();
    server.onMessage((frame) => {
      passed += 1;
      if (passed !== 1) {
        socket.send(frame);
      }
    });
  });
  await aNewProject(page, "PoolLiveMissed");
  const asks = asked(page);
  await openThePool(page);
  await expect.poll(asks).toBe(1);

  const shuttle = await anIdea(page, "The shuttle");
  await anotherAuthor(page, "patch", `/ideas/${shuttle}`, { title: "The heddle" });

  await expect.poll(() => passed).toBe(2);
  await expect(
    ideas(page).getByText("The heddle"),
    "the capture that was missed is there too, so the ideas were asked for again",
  ).toBeVisible();
  expect(asks()).toBeGreaterThan(1);
});
