import { test, expect, type Page } from "@playwright/test";

import { aBookShaped, aBookWithScenes } from "./support/book";
import { aLooseScene, aNewProject, API } from "./support/shell";
import { aBookInTheWritingView, asked, beginning, flow, heading, holding, holdingWhatIsWritten, menuItem, openTheWritingView, projectsOutline, savedTitles, saveState, scene, sceneActions, sceneOnServer, statusNote, theBook, wordCount } from "./support/writing";

async function savingUntilReleased(
  page: Page,
  held: Awaited<ReturnType<typeof holding>>,
) {
  await expect.poll(held.asked).toBeGreaterThan(0);
  await page.waitForTimeout(1000);
  await expect(
    saveState(page),
    "the change has not reached the server yet",
  ).toHaveText("Saving…");

  held.release();
  await expect(saveState(page)).toHaveText("All saved");
}

test("the status note sits in the status bar, at its right", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "Noted", { "Part One": 1 });

  const note = (await statusNote(page).boundingBox())!;
  const bar = (await page.locator(".status-bar").boundingBox())!;
  const wide = page.viewportSize()!.width;

  expect(note.y).toBeGreaterThanOrEqual(bar.y);
  expect(note.y + note.height).toBeLessThanOrEqual(bar.y + bar.height + 0.5);
  expect(wide - (note.x + note.width)).toBeLessThanOrEqual(16);
});

test("the status note makes room for the alarm beside it", async ({ page }) => {
  await page.route("**/api/service/refusals", (route) =>
    route.fulfill({
      json: [
        {
          id: 1,
          listener: "catalogue-idea",
          routing: "idea.captured",
          attempts: 5,
          why: "the projection had not landed",
          plainly: "A change to an idea did not reach the list of ideas.",
          occurred_at: "2026-10-10T09:00:00Z",
          given_up_at: "2026-10-10T09:01:00Z",
          acknowledged_at: null,
        },
      ],
    }),
  );
  await aBookInTheWritingView(page, "Alarmed", { "Part One": 1 });
  const alarm = page.getByRole("button", {
    name: "Changes that could not be applied",
  });
  await expect(alarm).toBeVisible();

  const note = (await statusNote(page).boundingBox())!;
  const raised = (await alarm.boundingBox())!;

  expect(note.x + note.width).toBeLessThan(raised.x);
});

test("the status note says Saving… until the server has stored what was written", async ({
  page,
}) => {
  const sync = await holdingWhatIsWritten(page);
  const scenes = await aBookInTheWritingView(page, "Saving", { "Part One": 1 });
  const written = scenes["Part One"][0];
  await expect(saveState(page)).toHaveText("All saved");

  sync.hold();
  await scene(page, written).click();
  await page.keyboard.type("Held back.");
  await expect(saveState(page)).toHaveText("Saving…");
  await expect.poll(sync.held).toBeGreaterThan(0);
  await page.waitForTimeout(1000);
  await expect(
    saveState(page),
    "nothing written has reached the server yet",
  ).toHaveText("Saving…");

  sync.release();
  await expect(saveState(page)).toHaveText("All saved");
  const stored = await (
    await page.request.get(`${API}/scenes/${written}`)
  ).json();
  expect(stored.text, "All saved only once it is stored").toContain(
    "Held back.",
  );
});

test("a scene begun by typing is all saved once its first words are stored", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "Begun saved", { "Part One": 0 });

  await beginning(page).click();
  await page.keyboard.type("The loom stood silent.");
  await expect(flow(page).locator(".scene .ProseMirror")).toContainText(
    "The loom stood silent.",
  );

  await expect(saveState(page)).toHaveText("All saved");
  const [begun] = (await projectsOutline(page)).sections[0].attachments;
  const stored = await (
    await page.request.get(`${API}/scenes/${begun.id}`)
  ).json();
  expect(stored.text, "All saved only once it is stored").toContain(
    "The loom stood silent.",
  );
});

test("a heading being typed is saving from the first key until the server has its title", async ({
  page,
}) => {
  await aNewProject(page, "SavingHeading");
  await aBookShaped(page, [{ title: "Part One" }]);
  await openTheWritingView(page);
  const held = await holding(page, "PATCH", "**/api/outlines/*/sections/*");

  await heading(page, "Part One").click();
  await page.keyboard.press("End");
  await page.keyboard.type(" and more");
  await expect(saveState(page)).toHaveText("Saving…");
  expect(held.asked(), "saving before the title is even sent").toBe(0);

  await savingUntilReleased(page, held);
  expect(await savedTitles(page)).toEqual(["Part One and more"]);
});

test("a scene renamed in the margin is saving until the server has its title", async ({
  page,
}) => {
  await aNewProject(page, "SavingTitle");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  const [arrival] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, arrival)).toBeVisible();
  const held = await holding(page, "PATCH", "**/api/scenes/*");

  await sceneActions(page, arrival).click();
  await menuItem(page, "Rename").click();
  await flow(page)
    .getByRole("textbox", { name: "Scene title" })
    .fill("The departure");
  await page.keyboard.press("Enter");
  await expect(saveState(page)).toHaveText("Saving…");

  await savingUntilReleased(page, held);
  expect((await (await sceneOnServer(page, arrival)).json()).title).toBe(
    "The departure",
  );
});

test("a change to the book's shape is saving until the server has made it", async ({
  page,
}) => {
  await aNewProject(page, "SavingShape");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [first, second] = scenes["Part One"];
  await openTheWritingView(page);
  await expect(scene(page, first)).toBeVisible();
  const held = await holding(
    page,
    "DELETE",
    "**/api/outlines/*/attachments/**",
  );

  await sceneActions(page, first).click();
  await menuItem(page, "Take out of the book").click();
  await expect(saveState(page)).toHaveText("Saving…");

  await savingUntilReleased(page, held);
  expect(await theBook(page)).toEqual([["Part One", [second]]]);
});

test("the word count follows the section being written in, and counts the book on a click", async ({
  page,
}) => {
  const scenes = await aBookInTheWritingView(page, "Counting", { "Part One": 1, "Part Two": 1 });

  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("The loom stood silent.");
  await expect(wordCount(page)).toHaveText("4 words in “Part One”");

  await scene(page, scenes["Part Two"][0]).click();
  await page.keyboard.type("Ende");
  await expect(wordCount(page)).toHaveText("1 word in “Part Two”");

  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("5 words in the book");
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("1 word in “Part Two”");
  await expect(saveState(page)).toHaveText("All saved");

  await page.reload();
  await heading(page, "Part One").click();
  await expect(
    wordCount(page),
    "what was written before counts too",
  ).toHaveText("4 words in “Part One”");
});

test("a section's word count takes in the sections nested in it", async ({
  page,
}) => {
  await aNewProject(page, "CountingNested");
  const { outline, ids } = await aBookShaped(page, [
    { title: "Part One" },
    { title: "Chapter 1", under: "Part One" },
    { title: "Part Two", after: "Part One" },
  ]);
  const placed: Record<string, string> = {};
  for (const title of ["Part One", "Chapter 1", "Part Two"]) {
    placed[title] = await aLooseScene(page);
    await page.request.post(`${API}/outlines/${outline}/attachments`, {
      data: {
        attachment: { kind: "scene", id: placed[title] },
        section: ids[title],
        after: null,
      },
    });
  }
  await openTheWritingView(page);

  await scene(page, placed["Part One"]).click();
  await page.keyboard.type("one two three");
  await scene(page, placed["Chapter 1"]).click();
  await page.keyboard.type("four five");
  await expect(wordCount(page)).toHaveText("2 words in “Chapter 1”");
  await scene(page, placed["Part Two"]).click();
  await page.keyboard.type("six");

  await heading(page, "Part One").click();
  await expect(wordCount(page)).toHaveText("5 words in “Part One”");
  await wordCount(page).click();
  await expect(wordCount(page)).toHaveText("6 words in the book");
});

test("before the cursor is anywhere, the word count counts the first section", async ({
  page,
}) => {
  await aBookInTheWritingView(page, "CountingFirst", { "Part One": 1, "Part Two": 0 });

  await expect(wordCount(page)).toHaveText("0 words in “Part One”");
});
