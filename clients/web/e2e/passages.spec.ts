import { test, expect, type Page, type WebSocket } from "@playwright/test";

const API = "http://127.0.0.1:3000/api";

const surface = (page: Page) => page.locator(".surface .ProseMirror");

const prose = (target: Page) =>
  surface(target).evaluate((node) => {
    const copy = node.cloneNode(true) as HTMLElement;
    copy.querySelectorAll(".ProseMirror-yjs-cursor").forEach((cursor) => cursor.remove());
    return copy.textContent ?? "";
  });

async function anOpenPassage(page: Page): Promise<string> {
  await page.goto("/");
  const created = page.waitForResponse(
    (response) =>
      response.url().endsWith("/api/passages") && response.request().method() === "POST",
  );
  await page.getByRole("button", { name: "Start writing" }).click();
  const response = await created;
  const { id } = await response.json();

  await expect(surface(page)).toBeVisible();
  await expect(page.getByText("Synced")).toBeVisible();

  return id;
}

test("the editor connects to the sync socket", async ({ page }) => {
  const passage = await anOpenPassage(page);

  expect(passage).toMatch(/^passage_/);
  await expect(page.getByText("Synced")).toBeVisible();
});

test("typed prose reaches the server's own projection", async ({ page, request }) => {
  const passage = await anOpenPassage(page);

  await surface(page).click();
  await page.keyboard.type("The loom stood silent.");

  await expect
    .poll(
      async () => {
        const response = await request.get(`${API}/passages/${passage}`);
        return (await response.json()).text;
      },
      { timeout: 10_000 },
    )
    .toContain("The loom stood silent.");
});

test("prose survives a reload", async ({ page, request }) => {
  const passage = await anOpenPassage(page);

  await surface(page).click();
  await page.keyboard.type("The warp was already strung.");
  await expect
    .poll(
      async () => {
        const response = await request.get(`${API}/passages/${passage}`);
        return (await response.json()).text;
      },
      { timeout: 10_000 },
    )
    .toContain("The warp was already strung.");

  await page.reload();

  await expect(page).toHaveURL(new RegExp(`passage=${passage}`));
  await expect(surface(page)).toContainText("The warp was already strung.");
});

test("two tabs on one passage converge", async ({ page, context }) => {
  const passage = await anOpenPassage(page);

  const second = await context.newPage();
  await second.goto(`/?passage=${passage}`);
  await expect(surface(second)).toBeVisible();
  await expect(second.getByText("Synced")).toBeVisible();

  await surface(page).click();
  await page.keyboard.type("She threaded the shuttle.");
  await expect(surface(second)).toContainText("She threaded the shuttle.", { timeout: 10_000 });

  await surface(second).click();
  await second.keyboard.press("Control+End");
  await second.keyboard.type(" The loom answered.");

  const woven = "She threaded the shuttle. The loom answered.";
  await expect
    .poll(async () => [await prose(page), await prose(second)], { timeout: 10_000 })
    .toEqual([woven, woven]);

  await second.close();
});

test("a passage the server does not know is dropped from the url", async ({ page }) => {
  await page.goto("/?passage=passage_00000000-0000-7000-8000-000000000000");

  await expect(page.getByRole("button", { name: "Start writing" })).toBeVisible();
  await expect(page).not.toHaveURL(/passage=/);
});

test("closing the passage tears down the editor and its socket", async ({ page }) => {
  const sockets: WebSocket[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/sync/")) {
      sockets.push(socket);
    }
  });

  await anOpenPassage(page);
  expect(sockets).toHaveLength(1);

  await page.getByRole("button", { name: "Stop writing" }).click();

  await expect.poll(() => sockets[0].isClosed(), { timeout: 10_000 }).toBe(true);
  await expect(surface(page)).toHaveCount(0);
  await expect(page).not.toHaveURL(/passage=/);
  await expect(page.getByRole("button", { name: "Start writing" })).toBeVisible();
});
