import { test, expect, type Page, type WebSocket } from "@playwright/test";

import { aNewProject, openTheBoard } from "./support/shell";

const API = "http://127.0.0.1:3000/api";

const surface = (page: Page) => page.locator(".surface .ProseMirror");

const prose = (target: Page) =>
  surface(target).evaluate((node) => {
    const copy = node.cloneNode(true) as HTMLElement;
    copy
      .querySelectorAll(".ProseMirror-yjs-cursor")
      .forEach((cursor) => cursor.remove());
    return copy.textContent ?? "";
  });

const idIn = (segment: string) => segment.split("-").pop() ?? segment;

async function aPassageBeingWritten(page: Page, named: string) {
  await aNewProject(page, named);

  const segments = new URL(page.url()).pathname.split("/");
  const project = segments[segments.length - 1];
  const made = await page.request.post(`${API}/passages`, {
    data: { project: idIn(project) },
  });
  expect(made.status()).toBe(201);
  const { id: passage } = await made.json();

  await page.goto(`/projects/${project}/passages/${passage}`);
  await expect(surface(page)).toBeVisible();
  await expect(page.getByText("Synced")).toBeVisible();

  return { address: page.url(), passage };
}

test("opening a passage connects the editor to the sync socket", async ({
  page,
}) => {
  await aPassageBeingWritten(page, "Connects");

  await expect(page.getByText("Synced")).toBeVisible();
});

test("typed prose reaches the server's own projection", async ({
  page,
  request,
}) => {
  const { passage } = await aPassageBeingWritten(page, "Projection");

  await surface(page).click();
  await page.keyboard.type("The loom stood silent.");

  await expect
    .poll(
      async () =>
        (await (await request.get(`${API}/passages/${passage}`)).json()).text,
      { timeout: 10_000 },
    )
    .toContain("The loom stood silent.");
});

test("prose survives a reload", async ({ page }) => {
  await aPassageBeingWritten(page, "Survives");

  await surface(page).click();
  await page.keyboard.type("She had not touched it since spring.");
  await expect(surface(page)).toContainText("since spring");

  await page.reload();

  await expect(surface(page)).toContainText("since spring");
});

test("two tabs on one passage converge", async ({ page, context }) => {
  const { address } = await aPassageBeingWritten(page, "Converge");

  const second = await context.newPage();
  await second.goto(address);
  await expect(surface(second)).toBeVisible();
  await expect(second.getByText("Synced")).toBeVisible();

  await surface(page).click();
  await page.keyboard.type("She threaded the shuttle.");
  await expect(surface(second)).toContainText("She threaded the shuttle.", {
    timeout: 10_000,
  });

  await surface(second).click();
  await second.keyboard.press("Control+End");
  await second.keyboard.type(" The loom answered.");

  const woven = "She threaded the shuttle. The loom answered.";
  await expect
    .poll(async () => [await prose(page), await prose(second)], {
      timeout: 10_000,
    })
    .toEqual([woven, woven]);

  await second.close();
});

test("leaving a passage tears down the editor and its socket", async ({
  page,
}) => {
  const sockets: WebSocket[] = [];
  page.on("websocket", (socket) => {
    if (socket.url().includes("/api/sync/")) {
      sockets.push(socket);
    }
  });

  await aPassageBeingWritten(page, "TearDown");
  expect(sockets).toHaveLength(1);

  await openTheBoard(page);

  await expect
    .poll(() => sockets[0].isClosed(), { timeout: 10_000 })
    .toBe(true);
  await expect(surface(page)).toHaveCount(0);
});

test("a passage the server does not know says so", async ({ page }) => {
  const { address } = await aPassageBeingWritten(page, "Missing");
  const elsewhere = address.replace(
    /passage_[0-9A-Za-z]{22}$/,
    "passage_0000000000000000000000",
  );

  await page.goto(elsewhere);

  await expect(page.getByRole("alert")).toBeVisible();
  await expect(surface(page)).toHaveCount(0);
});

const titleField = (page: Page) =>
  page.getByRole("textbox", { name: "Passage title" });

async function retitle(page: Page, title: string) {
  const saved = page.waitForResponse(
    (response) =>
      response.request().method() === "PATCH" &&
      response.url().includes("/api/passages/"),
  );
  await titleField(page).fill(title);
  await titleField(page).press("Tab");
  expect((await saved).status()).toBe(200);
}

test("a passage's title is kept", async ({ page }) => {
  await aPassageBeingWritten(page, "Titled");

  await retitle(page, "  The arrival  ");
  await page.reload();

  await expect(
    titleField(page),
    "the server trims the title, and what comes back is what the page shows",
  ).toHaveValue("The arrival");
});

test("retitling leaves the editor as it was", async ({ page }) => {
  await aPassageBeingWritten(page, "Connected");
  const editing = await surface(page).elementHandle();

  await retitle(page, "The arrival");

  expect(
    await editing!.evaluate((node) => node.isConnected),
    "the title lives in its own signal, so naming the passage must not rebuild the editor out from under the author",
  ).toBe(true);
  await expect(page.getByText("Synced", { exact: true })).toBeVisible();
});
