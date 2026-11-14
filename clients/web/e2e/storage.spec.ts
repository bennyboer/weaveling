import { test, expect, type Page, type Route } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";

const MB = 1024 * 1024;

const A_PIXEL = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

type Upload = {
  id: string;
  owner: { kind: "project" | "account"; id: string };
  project_name: string | null;
  format: string;
  width: number;
  height: number;
  total_size: number;
  uploaded_at: string;
  deletable: boolean;
  uses?: { purpose: string; by: { kind: string; id: string } }[];
};

type Shelf = {
  uploads: Upload[];
  most: number | null;
  pageSize: number;
  asked: string[];
  deleted: string[];
};

const anUpload = (
  id: string,
  total_size: number,
  uploaded_at: string,
  into: { project: string; name: string | null } | null,
  deletable = true,
): Upload => ({
  id,
  owner: into ? { kind: "project", id: into.project } : { kind: "account", id: "account_ada" },
  project_name: into?.name ?? null,
  format: "png",
  width: 1280,
  height: 1024,
  total_size,
  uploaded_at,
  deletable,
});

const THE_PORTRAIT = anUpload("asset_portrait", 512 * 1024, "2026-11-10T12:00:00Z", null);
const THE_MAP = anUpload("asset_map", MB, "2026-11-12T12:00:00Z", {
  project: "project_loom",
  name: "The Loom",
});

/// Answers the assets routes from the shelf: the uploads a page at a time in the order asked, the usage, deleting, and every thumbnail.
async function aShelf(page: Page, uploads: Upload[], most: number | null = 2 * MB): Promise<Shelf> {
  const shelf: Shelf = { uploads, most, pageSize: 50, asked: [], deleted: [] };
  const used = () => shelf.uploads.reduce((sum, upload) => sum + upload.total_size, 0);

  await page.route(
    (url) => url.pathname === "/api/assets",
    (route: Route) => {
      const url = new URL(route.request().url());
      const sort = url.searchParams.get("sort") ?? "newest";
      shelf.asked.push(url.search);
      const sorted = [...shelf.uploads].sort((one, other) =>
        sort === "largest"
          ? other.total_size - one.total_size
          : other.uploaded_at.localeCompare(one.uploaded_at),
      );
      const after = url.searchParams.get("after");
      const from = after ? sorted.findIndex((upload) => upload.id === after) + 1 : 0;
      const listed = sorted.slice(from, from + shelf.pageSize);
      const more = from + shelf.pageSize < sorted.length;

      return route.fulfill({
        json: { uploads: listed, next: more ? listed[listed.length - 1].id : null },
      });
    },
  );
  await page.route(
    (url) => url.pathname === "/api/assets/usage",
    (route) => route.fulfill({ json: { used: used(), most: shelf.most } }),
  );
  await page.route(
    (url) => /^\/api\/assets\/[^/]+$/.test(url.pathname),
    (route) => {
      if (route.request().method() !== "DELETE") {
        return route.fallback();
      }
      const id = new URL(route.request().url()).pathname.split("/").pop() ?? "";
      shelf.deleted.push(id);
      shelf.uploads = shelf.uploads.filter((upload) => upload.id !== id);

      return route.fulfill({ status: 204 });
    },
  );
  await page.route(
    (url) => /^\/api\/assets\/[^/]+\/content$/.test(url.pathname),
    (route) => route.fulfill({ contentType: "image/png", body: A_PIXEL }),
  );

  return shelf;
}

async function signedIn(page: Page) {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
}

const uploads = (page: Page) => page.getByRole("main").getByRole("listitem");

test("the account menu leads to the storage, which lists every upload with what it uses", async ({
  page,
}) => {
  // given: Ada signed in, with a portrait of her own and a map in the loom
  await aShelf(page, [THE_PORTRAIT, THE_MAP]);
  await signedIn(page);

  // when: she opens her storage from the account menu
  await page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" }).click();
  await page.getByRole("menu").getByRole("menuitem", { name: "Storage" }).click();

  // then: she sees how much she uses of her limit
  await expect(page.getByRole("heading", { name: "Storage" })).toBeVisible();
  await expect(page.getByRole("main")).toContainText("1.5 MB of 2 MB used");
  // and: her uploads, the newest first, each with where it is, its size and its day
  await expect(uploads(page)).toHaveCount(2);
  await expect(uploads(page).nth(0)).toContainText("In The Loom");
  await expect(uploads(page).nth(0)).toContainText("1 MB · uploaded on 12 Nov 2026");
  await expect(uploads(page).nth(1)).toContainText("Your own file");
  await expect(uploads(page).nth(1)).toContainText("512 KB · uploaded on 10 Nov 2026");
});

test("the account page leads to the storage too", async ({ page }) => {
  // given: Ada signed in on her account page
  await aShelf(page, [THE_PORTRAIT]);
  await signedIn(page);
  await page.goto("/account");

  // when: she follows the link to her storage
  await page.getByRole("main").getByRole("link", { name: "Your media" }).click();

  // then: the storage is shown
  await expect(page).toHaveURL(/\/account\/storage$/);
  await expect(uploads(page)).toHaveCount(1);
});

test("the uploads are sorted by size when asked, and pages go on with Show more", async ({
  page,
}) => {
  // given: Ada with three uploads, two to a page
  const shelf = await aShelf(page, [
    THE_PORTRAIT,
    THE_MAP,
    anUpload("asset_old_large", Math.round(1.8 * MB), "2026-11-01T12:00:00Z", null),
  ]);
  shelf.pageSize = 2;
  await signedIn(page);
  await page.goto("/account/storage");
  await expect(uploads(page)).toHaveCount(2);

  // when: she sorts them by size
  await page.getByLabel("Sort by").selectOption({ label: "Largest first" });

  // then: the largest come first, and the server was asked for that order
  await expect(uploads(page).nth(0)).toContainText("1.8 MB");
  await expect(uploads(page).nth(1)).toContainText("1 MB");
  expect(shelf.asked).toContain("?sort=largest");
  // when: she asks for more
  await page.getByRole("button", { name: "Show more" }).click();
  // then: the third follows, and there is nothing more to show
  await expect(uploads(page)).toHaveCount(3);
  await expect(uploads(page).nth(2)).toContainText("512 KB");
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);
});

test("an upload is deleted after asking, and the storage used goes down", async ({ page }) => {
  // given: Ada with her portrait and the map
  const shelf = await aShelf(page, [THE_PORTRAIT, THE_MAP]);
  await signedIn(page);
  await page.goto("/account/storage");

  // when: she deletes the map
  await uploads(page).nth(0).getByRole("button", { name: "Delete" }).click();
  const asking = page.getByRole("dialog", { name: "Delete this file?" });
  await expect(asking).toContainText("The file is deleted for good.");
  await asking.getByRole("button", { name: "Delete the file" }).click();

  // then: it is gone, and only the portrait counts
  await expect(asking).toBeHidden();
  await expect(uploads(page)).toHaveCount(1);
  await expect(uploads(page).nth(0)).toContainText("Your own file");
  await expect(page.getByRole("main")).toContainText("512 KB of 2 MB used");
  expect(shelf.deleted).toEqual(["asset_map"]);
});

test("cancelling keeps the upload", async ({ page }) => {
  // given: Ada with her portrait
  const shelf = await aShelf(page, [THE_PORTRAIT]);
  await signedIn(page);
  await page.goto("/account/storage");

  // when: she starts deleting it and cancels
  await uploads(page).nth(0).getByRole("button", { name: "Delete" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Cancel" }).click();

  // then: it stays, and nothing was deleted
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(uploads(page)).toHaveCount(1);
  expect(shelf.deleted).toEqual([]);
});

test("an upload in a project she can no longer write in says why it cannot be deleted", async ({
  page,
}) => {
  // given: Ada with a map in a project she only reads now, its name unknown
  await aShelf(page, [
    anUpload("asset_map", MB, "2026-11-12T12:00:00Z", { project: "project_loom", name: null }, false),
  ]);
  await signedIn(page);

  // when: she opens her storage
  await page.goto("/account/storage");

  // then: the upload is shown without a delete button, and says why
  await expect(uploads(page).nth(0)).toContainText("In a project");
  await expect(uploads(page).nth(0)).toContainText("Only authors of the project can delete it.");
  await expect(uploads(page).nth(0).getByRole("button", { name: "Delete" })).toHaveCount(0);
});

test("with nothing uploaded, it says so, and without a limit only the use is told", async ({
  page,
}) => {
  // given: Ada with nothing uploaded and no limit
  await aShelf(page, [], null);
  await signedIn(page);

  // when: she opens her storage
  await page.goto("/account/storage");

  // then: it says nothing is uploaded yet, and how much is used
  await expect(page.getByText("You have not uploaded anything yet.")).toBeVisible();
  await expect(page.getByRole("main")).toContainText("0 B used");
  await expect(page.getByRole("meter")).toHaveCount(0);
});

test("an upload in use as her picture says so and cannot be deleted", async ({ page }) => {
  // given: Ada's portrait, shown as her account picture
  await aShelf(page, [
    {
      ...THE_PORTRAIT,
      deletable: false,
      uses: [{ purpose: "account-picture", by: { kind: "account", id: "account_ada" } }],
    },
  ]);
  await signedIn(page);

  // when: she opens her storage
  await page.goto("/account/storage");

  // then: the portrait says it is her picture, without a delete button
  await expect(uploads(page).nth(0)).toContainText("In use as your account picture.");
  await expect(uploads(page).nth(0).getByRole("button", { name: "Delete" })).toHaveCount(0);
});
