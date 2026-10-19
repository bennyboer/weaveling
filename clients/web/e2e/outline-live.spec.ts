import { test, expect, type Page } from "@playwright/test";

import { aBookShaped } from "./support/book";
import { aNewProject, aSceneMade, API, openTheOutline } from "./support/shell";
import { savedTitles, titled } from "./support/writing";

async function anOutlineFollowedLive(page: Page, named: string) {
  await aNewProject(page, named);
  const book = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  let opened = 0;
  page.on("response", (response) => {
    if (
      response.request().method() === "POST" &&
      response.url().endsWith("/api/outlines")
    ) {
      opened += 1;
    }
  });
  await openTheOutline(page);
  await expect
    .poll(() => opened, {
      message:
        "the outline is loaded, and loaded again once it is followed live",
    })
    .toBeGreaterThanOrEqual(2);

  return { ...book, reloads: () => opened };
}

async function anotherAuthor(
  page: Page,
  method: "post" | "patch" | "delete",
  path: string,
  data?: object,
) {
  const done = await page.request[method](`${API}${path}`, { data });
  expect(done.ok(), await done.text()).toBe(true);
}

const titleOf = (page: Page, named: string) =>
  page.getByRole("textbox", { name: `Section ${named}` });

test("a section another author adds appears in the outline", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveAdded",
  );

  await anotherAuthor(page, "post", `/outlines/${outline}/sections`, {
    title: "Part Three",
    under: null,
    after: ids["Part Two"],
  });

  await expect(titleOf(page, "Part Three")).toBeVisible();
});

test("a scene another author attaches is named in the outline", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveNamed",
  );
  const scene = await aSceneMade(page);
  await titled(page, scene, "The arrival");

  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: scene },
    section: ids["Part One"],
    after: null,
  });

  await expect(
    page.locator(".leaf").getByRole("link", { name: "The arrival" }),
  ).toBeVisible();
});

test("a title I am typing is not overwritten by another author's rename", async ({
  page,
}) => {
  const { outline, ids, reloads } = await anOutlineFollowedLive(
    page,
    "OutlineLiveTyping",
  );
  await titleOf(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(", drafted");
  const before = reloads();

  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => reloads()).toBeGreaterThan(before);

  await expect(page.locator("input.title:focus")).toHaveValue(
    "Part One, drafted",
  );
});

test("a title I only sat in takes another author's rename without writing the old one back", async ({
  page,
}) => {
  const { outline, ids, reloads } = await anOutlineFollowedLive(
    page,
    "OutlineLiveSatIn",
  );
  await titleOf(page, "Part One").click();
  const before = reloads();
  await anotherAuthor(
    page,
    "patch",
    `/outlines/${outline}/sections/${ids["Part One"]}`,
    { title: "The Beginning" },
  );
  await expect.poll(() => reloads()).toBeGreaterThan(before);

  await page.keyboard.press("Escape");

  await expect(titleOf(page, "The Beginning")).toHaveValue("The Beginning");
  await page.waitForTimeout(500);
  expect(await savedTitles(page)).toEqual(["The Beginning", "Part Two"]);
});

test("a scene the listing does not know yet is asked for once, not over and over", async ({
  page,
}) => {
  const { outline, ids } = await anOutlineFollowedLive(
    page,
    "OutlineLiveLagging",
  );
  const lagging = await aSceneMade(page);
  let listed = 0;
  await page.route(/\/api\/scenes\?project=/, async (route) => {
    listed += 1;
    const answer = await route.fetch();
    const page_ = await answer.json();
    page_.scenes = page_.scenes.filter(
      (held: { id: string }) => held.id !== lagging,
    );
    await route.fulfill({ response: answer, json: page_ });
  });

  await anotherAuthor(page, "post", `/outlines/${outline}/attachments`, {
    attachment: { kind: "scene", id: lagging },
    section: ids["Part One"],
    after: null,
  });
  await expect.poll(() => listed).toBeGreaterThan(0);
  await page.waitForTimeout(1500);

  expect(
    listed,
    "asked again once for the change that brought it, not in a loop until the listing catches up",
  ).toBeLessThanOrEqual(2);
});
