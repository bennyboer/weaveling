import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn, type Server } from "./support/accounts";

const A_PIXEL = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
  "base64",
);

type Pictures = {
  uploaded: Buffer[];
  shown: { picture: string; drop_previous: boolean }[];
  removed: string[];
  usedElsewhere: Set<string>;
};

/// Answers the routes of pictures and assets: uploads get new ids, an asset tells its uses, and the picture is shown and removed.
async function pictures(page: Page, server: Server): Promise<Pictures> {
  const told: Pictures = { uploaded: [], shown: [], removed: [], usedElsewhere: new Set() };

  await page.route(
    (url) => url.pathname === "/api/assets",
    (route) => {
      if (route.request().method() !== "POST") {
        return route.fallback();
      }
      told.uploaded.push(route.request().postDataBuffer() ?? Buffer.alloc(0));
      const id = `asset_new_${told.uploaded.length}`;

      return route.fulfill({
        status: 201,
        json: {
          id,
          version: 1,
          owner: { kind: "account", id: ADA.account },
          format: "jpeg",
          width: 1,
          height: 1,
          widths: [1],
          original_size: 600,
          total_size: 900,
          uses: [],
        },
      });
    },
  );
  await page.route(
    (url) => /^\/api\/assets\/[^/]+$/.test(url.pathname),
    (route) => {
      if (route.request().method() !== "GET") {
        return route.fallback();
      }
      const id = new URL(route.request().url()).pathname.split("/").pop() ?? "";
      const uses = [
        ...(id === server.picture
          ? [{ purpose: "account-picture", by: { kind: "account", id: ADA.account } }]
          : []),
        ...(told.usedElsewhere.has(id)
          ? [{ purpose: "idea-cover", by: { kind: "account", id: ADA.account } }]
          : []),
      ];

      return route.fulfill({
        json: {
          id,
          version: 1,
          owner: { kind: "account", id: ADA.account },
          format: "jpeg",
          width: 1,
          height: 1,
          widths: [1],
          original_size: 600,
          total_size: 900,
          uses,
        },
      });
    },
  );
  await page.route(
    (url) => /^\/api\/assets\/[^/]+\/content$/.test(url.pathname),
    (route) => route.fulfill({ contentType: "image/png", body: A_PIXEL }),
  );
  await page.route(
    (url) => url.pathname === "/api/accounts/current/picture",
    (route) => {
      if (route.request().method() === "PUT") {
        const asked = route.request().postDataJSON();
        told.shown.push(asked);
        server.picture = asked.picture;
      } else {
        told.removed.push(new URL(route.request().url()).search);
        server.picture = null;
      }

      return route.fulfill({ status: 204 });
    },
  );

  return told;
}

async function onHerAccountPage(page: Page, picture: string | null = null) {
  const server = await aServerWithAccounts(page);
  server.picture = picture;
  const told = await pictures(page, server);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(accountButton(page)).toBeVisible();
  await page.goto("/account");
  await expect(page.getByRole("heading", { name: "Ada Lovelace" })).toBeVisible();

  return told;
}

const accountButton = (page: Page) =>
  page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" });

async function chosen(page: Page, label: string, file = A_PIXEL, name = "me.png") {
  await page.getByLabel(label).setInputFiles({ name, mimeType: "image/png", buffer: file });
}

test("a first picture is uploaded as a square JPEG and shown at once, in the masthead too", async ({
  page,
}) => {
  // given: Ada on her account page, showing no picture
  const told = await onHerAccountPage(page);
  await expect(accountButton(page)).toHaveText("A");

  // when: she chooses a picture
  await chosen(page, "Choose a picture…");

  // then: it is shown, with nothing asked
  await expect(accountButton(page).locator("img")).toHaveAttribute(
    "src",
    "/api/assets/asset_new_1/content?width=96",
  );
  expect(told.shown).toEqual([{ picture: "asset_new_1", drop_previous: false }]);
  // and: what was uploaded is a JPEG made in the browser
  expect(told.uploaded[0].subarray(0, 2)).toEqual(Buffer.from([0xff, 0xd8]));
  // and: the page offers to change or remove it
  await expect(page.getByText("Change the picture…")).toBeVisible();
  await expect(page.getByRole("button", { name: "Remove the picture" })).toBeVisible();
});

test("replacing a picture used for nothing else asks whether to keep it or delete it", async ({
  page,
}) => {
  // given: Ada showing a picture
  const told = await onHerAccountPage(page, "asset_old");

  // when: she chooses another and deletes the old one
  await chosen(page, "Change the picture…");
  const asking = page.getByRole("dialog", { name: "What should happen to your previous picture?" });
  await asking.getByRole("button", { name: "Delete it" }).click();

  // then: the new one is shown, the old one to be deleted
  await expect(asking).toBeHidden();
  expect(told.shown).toEqual([{ picture: "asset_new_1", drop_previous: true }]);
  await expect(accountButton(page).locator("img")).toHaveAttribute(
    "src",
    "/api/assets/asset_new_1/content?width=96",
  );
});

test("keeping the previous picture as a file shows the new one and deletes nothing", async ({
  page,
}) => {
  // given: Ada showing a picture
  const told = await onHerAccountPage(page, "asset_old");

  // when: she chooses another and keeps the old one
  await chosen(page, "Change the picture…");
  await page.getByRole("dialog").getByRole("button", { name: "Keep it as a file" }).click();

  // then: the new one is shown, the old one kept
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(told.shown).toEqual([{ picture: "asset_new_1", drop_previous: false }]);
});

test("cancelling the question leaves the picture as it was", async ({ page }) => {
  // given: Ada showing a picture
  const told = await onHerAccountPage(page, "asset_old");

  // when: she chooses another but cancels the question
  await chosen(page, "Change the picture…");
  await page.getByRole("dialog").getByRole("button", { name: "Cancel" }).click();

  // then: nothing is changed
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(told.shown).toEqual([]);
  await expect(accountButton(page).locator("img")).toHaveAttribute(
    "src",
    "/api/assets/asset_old/content?width=96",
  );
});

test("a previous picture used elsewhere too is kept without asking", async ({ page }) => {
  // given: Ada showing a picture that is used elsewhere too
  const told = await onHerAccountPage(page, "asset_old");
  told.usedElsewhere.add("asset_old");

  // when: she chooses another
  await chosen(page, "Change the picture…");

  // then: it is shown at once, the old one kept
  await expect(accountButton(page).locator("img")).toHaveAttribute(
    "src",
    "/api/assets/asset_new_1/content?width=96",
  );
  expect(told.shown).toEqual([{ picture: "asset_new_1", drop_previous: false }]);
  await expect(page.getByRole("dialog")).toHaveCount(0);
});

test("removing the picture asks, and the initial is shown again", async ({ page }) => {
  // given: Ada showing a picture
  const told = await onHerAccountPage(page, "asset_old");

  // when: she removes it and deletes the file
  await page.getByRole("button", { name: "Remove the picture" }).click();
  await page
    .getByRole("dialog", { name: "Remove your picture?" })
    .getByRole("button", { name: "Remove and delete the file" })
    .click();

  // then: it is removed, to be deleted, and her initial is shown again
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(told.removed).toEqual(["?drop=true"]);
  await expect(accountButton(page)).toHaveText("A");
  await expect(page.getByText("Choose a picture…")).toBeVisible();
});

test("a file that is no image is refused before anything is uploaded", async ({ page }) => {
  // given: Ada on her account page
  const told = await onHerAccountPage(page);

  // when: she chooses a file that only claims to be an image
  await chosen(page, "Choose a picture…", Buffer.from("not an image at all"), "notes.png");

  // then: she is told, and nothing was uploaded
  await expect(page.getByRole("alert")).toHaveText("This file could not be read as an image.");
  expect(told.uploaded).toEqual([]);
});
