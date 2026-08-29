import { test, expect, type Page, type WebSocket } from "@playwright/test";

const API = "http://127.0.0.1:3000/api";

const surface = (page: Page) => page.locator(".surface .ProseMirror");

const prose = (target: Page) =>
  surface(target).evaluate((node) => {
    const copy = node.cloneNode(true) as HTMLElement;
    copy.querySelectorAll(".ProseMirror-yjs-cursor").forEach((cursor) => cursor.remove());
    return copy.textContent ?? "";
  });

const idIn = (segment: string) => segment.split("-").pop() ?? segment;

async function aPieceBeingWritten(page: Page, named: string) {
  const title = `Project ${named} ${crypto.randomUUID().slice(0, 8)}`;

  await page.goto("/");
  await page.getByPlaceholder("A working title…").fill(title);
  await page.getByRole("button", { name: "Create", exact: true }).click();
  await page.getByRole("link", { name: title }).click();

  await page.getByRole("textbox", { name: "What is the idea?" }).fill("The loom");
  await page.getByRole("button", { name: "Capture", exact: true }).click();
  await page.getByRole("list", { name: "Pieces" }).getByRole("link", { name: "The loom" }).click();

  await expect(surface(page)).toBeVisible();
  await expect(page.getByText("Synced")).toBeVisible();

  const segments = new URL(page.url()).pathname.split("/");

  return { address: page.url(), piece: idIn(segments[segments.length - 1]) };
}

test("opening a piece connects the editor to the sync socket", async ({ page }) => {
  await aPieceBeingWritten(page, "Connects");

  await expect(page.getByText("Synced")).toBeVisible();
});

test("a piece is given its passage the first time it is opened", async ({ page, request }) => {
  const { piece } = await aPieceBeingWritten(page, "Attached");

  const found = await (await request.get(`${API}/pieces/${piece}`)).json();

  expect(found.passage).toMatch(/^passage_/);
});

test("typed prose reaches the server's own projection", async ({ page, request }) => {
  const { piece } = await aPieceBeingWritten(page, "Projection");
  const { passage } = await (await request.get(`${API}/pieces/${piece}`)).json();

  await surface(page).click();
  await page.keyboard.type("The loom stood silent.");

  await expect
    .poll(
      async () => (await (await request.get(`${API}/passages/${passage}`)).json()).text,
      { timeout: 10_000 },
    )
    .toContain("The loom stood silent.");
});

test("two tabs on one piece converge", async ({ page, context }) => {
  const { address } = await aPieceBeingWritten(page, "Converge");

  const second = await context.newPage();
  await second.goto(address);
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

test("leaving a piece tears down the editor and its socket", async ({ page }) => {
  const sockets: WebSocket[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/sync/")) {
      sockets.push(socket);
    }
  });

  await aPieceBeingWritten(page, "TearDown");
  expect(sockets).toHaveLength(1);

  await page.getByRole("link", { name: "Back to the pool" }).click();

  await expect.poll(() => sockets[0].isClosed(), { timeout: 10_000 }).toBe(true);
  await expect(surface(page)).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Pieces" })).toBeVisible();
});

test("a piece the server does not know says so", async ({ page }) => {
  const { address } = await aPieceBeingWritten(page, "Missing");
  const elsewhere = address.replace(/piece_[0-9A-Za-z]{22}$/, "piece_0000000000000000000000");

  await page.goto(elsewhere);

  await expect(page.getByRole("alert")).toBeVisible();
  await expect(surface(page)).toHaveCount(0);
});
