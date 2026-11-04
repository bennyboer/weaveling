import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject, openTheOutline, views } from "./support/shell";
import { aBookInTheWritingView, asked, backToWhereILeftOff, backToWhereYouLeftOffButton, beginning, flow, following, menuItem, openTheWritingView, projectOf, saveState, scene, sceneActions, theBookIsIn, titled } from "./support/writing";

test("the first time, Write lands at the start of the book", async ({
  page,
}) => {
  // when: a book is opened in the writing view for the first time
  const scenes = await aBookInTheWritingView(page, "FirstLanding", { "Part One": 1, "Part Two": 1 });

  // then: the cursor is at the start of the book
  await expect(scene(page, scenes["Part One"][0])).toBeFocused();
  // and: no way back is offered
  await expect(backToWhereYouLeftOffButton(page), "nothing was left off yet").toHaveCount(0);
  // and: typing goes there
  await page.keyboard.type("Begun");
  await expect(scene(page, scenes["Part One"][0])).toHaveText("Begun");
});

test("going back to where the author left off finds the very letter, after moving away and after a reload", async ({
  page,
}) => {
  // given: a long book written in far down, the cursor left within a word
  await aNewProject(page, "LeftOff");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const far = scenes["Part One"][11];
  const later = scene(page, far);
  await openTheWritingView(page);
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  await later.click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  for (let step = 0; step < 5; step++) {
    await page.keyboard.press("ArrowRight");
  }
  await expect(saveState(page)).toHaveText("All saved");

  // when: another view is opened and the writing view again
  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await openTheWritingView(page);
  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is at the very letter
  await expect(later).toBeFocused();
  await page.keyboard.type("X");
  await expect(later).toHaveText("AlphaX Beta");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the page is reloaded
  await page.reload();
  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is at the very letter
  await expect(later).toBeFocused();
  await page.keyboard.type("Y");
  await expect(later).toHaveText("AlphaXY Beta");
});

test("when the scene left off in has left the book, Write lands at its start", async ({
  page,
}) => {
  // given: the scene written in last taken out of the book
  await aNewProject(page, "LeftOffGone");
  const scenes = await aBookWithScenes(page, { "Part One": 1, "Part Two": 1 });
  const [first] = scenes["Part One"];
  const [later] = scenes["Part Two"];
  await openTheWritingView(page);
  await scene(page, later).click();
  await page.keyboard.type("Gone soon");
  await sceneActions(page, later).click();
  await menuItem(page, "Take out of the book").click();
  await expect(scene(page, later)).toHaveCount(0);
  await expect(saveState(page)).toHaveText("All saved");

  // when: the page is reloaded
  await page.reload();

  // then: the cursor is at the start of the book
  await expect(scene(page, first)).toBeFocused();
});

test("a book with no scenes yet lands on its first section's writing line", async ({
  page,
}) => {
  // when: a book with no scenes opens in the writing view
  await aBookInTheWritingView(page, "NothingWritten", { "Part One": 0 });

  // then: its first section's writing line has the cursor
  await expect(beginning(page)).toBeFocused();
});

test("landing far down the book brings the line left off into the middle of the window", async ({
  page,
}) => {
  // given: a long book written in half way down
  await aNewProject(page, "FarDown");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const middle = scene(page, scenes["Part One"][7]);
  await openTheWritingView(page);
  await middle.click();
  await page.keyboard.type("Half way.");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the page is reloaded
  await page.reload();
  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is there
  await expect(middle).toBeFocused();

  // and: its line is in the middle of the window
  const line = (await middle.locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(
    Math.abs(line.y + line.height / 2 - tall / 2),
    "the line sits in the middle third of the window",
  ).toBeLessThan(tall / 6);
});

test("even the book's last line can be brought to the middle of the window", async ({
  page,
}) => {
  // given: a long book written in at its very end
  await aNewProject(page, "LastLine");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const last = scene(page, scenes["Part One"][13]);
  await openTheWritingView(page);
  await last.click();
  await page.keyboard.type("At the very end.");
  await expect(saveState(page)).toHaveText("All saved");

  // when: the page is reloaded
  await page.reload();
  // and: the author goes back to where she left off
  await backToWhereILeftOff(page);
  // then: the cursor is there
  await expect(last).toBeFocused();

  // and: its line is in the middle of the window
  const line = (await last.locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(
    Math.abs(line.y + line.height / 2 - tall / 2),
    "there is room below the book to scroll its end up",
  ).toBeLessThan(tall / 6);
});

test("leaving the writing view leaves nothing behind that clicks or keys can trip over", async ({
  page,
}) => {
  // given: every panic on the page kept
  const panics: string[] = [];
  page.on("console", (message) => {
    if (message.type() === "error" && message.text().includes("panicked")) {
      panics.push(message.text());
    }
  });
  // and: a book open in the writing view
  await aBookInTheWritingView(page, "Leaving", { "Part One": 1 });

  // when: the outline is opened and clicked and keys are pressed
  await openTheOutline(page);
  await page.mouse.click(5, 300);
  await page.keyboard.press("Escape");
  await page.keyboard.press("Shift+F10");

  // then: nothing panics
  expect(panics).toEqual([]);
});

test("a scene followed from the outline opens the writing view at it, far down the book", async ({
  page,
}) => {
  // given: a long book with a named scene far down, open on the outline
  await aNewProject(page, "Followed");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const followed = scenes["Part One"][9];
  await titled(page, followed, "The tenth");
  await openTheOutline(page);

  // when: the scene is followed
  await page.locator(".leaf").getByRole("link", { name: "The tenth" }).click();

  // then: the writing view opens with the cursor in it
  await expect(scene(page, followed)).toBeFocused();
  // and: at its own address
  await expect(page, "the address is the view's own again").toHaveURL(
    /\/write$/,
  );
  // and: typing goes there
  await page.keyboard.type("Here.");
  await expect(scene(page, followed)).toHaveText("Here.");
  // and: its line is in the middle of the window
  const line = (await scene(page, followed).locator("p").boundingBox())!;
  const tall = page.viewportSize()!.height;
  expect(Math.abs(line.y + line.height / 2 - tall / 2)).toBeLessThan(tall / 6);
});

test("following the scene left off in lands where the author left off in it", async ({
  page,
}) => {
  // given: the cursor left within a word of a named scene
  await aNewProject(page, "FollowedBack");
  const scenes = await aBookWithScenes(page, { "Part One": 2 });
  const [, second] = scenes["Part One"];
  await titled(page, second, "The second");
  await openTheWritingView(page);
  await scene(page, second).click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  for (let step = 0; step < 5; step++) {
    await page.keyboard.press("ArrowRight");
  }
  await expect(saveState(page)).toHaveText("All saved");

  // when: the scene is followed from the outline
  await openTheOutline(page);
  await page.locator(".leaf").getByRole("link", { name: "The second" }).click();

  // then: the cursor is in it
  await expect(scene(page, second)).toBeFocused();
  // and: no way back is offered
  await expect(
    backToWhereYouLeftOffButton(page),
    "a scene followed to needs no way back",
  ).toHaveCount(0);
  // and: typing goes where it was left
  await page.keyboard.type("X");
  await expect(scene(page, second)).toHaveText("AlphaX Beta");
});

test("a project's details are fetched once while moving between its views", async ({
  page,
}) => {
  // given: a book in german
  await aNewProject(page, "OnceOnly");
  await aBookWithScenes(page, { "Part One": 1 });
  // and: every fetching of its details kept
  const project = projectOf(page);
  const asked: string[] = [];
  page.on("request", (request) => {
    if (request.method() === "GET" && request.url().endsWith(`/api/projects/${project}`)) {
      asked.push(request.url());
    }
  });
  await theBookIsIn(page, "de");

  // when: its views are opened one after another
  await openTheWritingView(page);
  await expect(flow(page)).toHaveAttribute("lang", "de");
  await openTheOutline(page);
  await openTheWritingView(page);
  await expect(flow(page)).toHaveAttribute("lang", "de");

  // then: its details were fetched once
  expect(
    asked,
    "the project being worked in is held while its views come and go",
  ).toHaveLength(1);
});

test("Write opens at the start of the book, and offers the way back to where the author left off", async ({
  page,
}) => {
  // given: a long book with a named scene far down, the cursor left within it
  await aNewProject(page, "OfferedBack");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const [first] = scenes["Part One"];
  const far = scenes["Part One"][11];
  await titled(page, far, "The far scene");
  await openTheWritingView(page);
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  await scene(page, far).click();
  await page.keyboard.type("Alpha Beta");
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowRight");
  await expect(saveState(page)).toHaveText("All saved");

  // when: another view is opened and the writing view again
  await views(page).getByRole("link", { name: "Board", exact: true }).click();
  await openTheWritingView(page);

  // then: the way back is offered, naming the scene
  await expect(backToWhereYouLeftOffButton(page)).toContainText("The far scene");
  // and: the book is open at its start, the cursor in no scene
  expect(
    await page.evaluate(() => window.scrollY),
    "the book is open at its start",
  ).toBe(0);
  await expect(scene(page, first)).not.toBeFocused();
  await expect(scene(page, far)).not.toBeFocused();

  // when: the way back is taken
  await backToWhereYouLeftOffButton(page).click();

  // then: the cursor is where it was left, in sight
  await expect(scene(page, far)).toBeFocused();
  await expect(scene(page, far)).toBeInViewport();
  // and: the offer goes
  await expect(backToWhereYouLeftOffButton(page), "taken, the offer goes").toHaveCount(0);
  await page.keyboard.type("X");
  await expect(scene(page, far)).toHaveText("AXlpha Beta");
});

test("putting the cursor in the book lets the offer go", async ({ page }) => {
  // given: the way back offered to a scene far down
  await aNewProject(page, "OfferLetGo");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const [first] = scenes["Part One"];
  const far = scenes["Part One"][11];
  await openTheWritingView(page);
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  await scene(page, far).click();
  await page.keyboard.type("Left here.");
  await expect(saveState(page)).toHaveText("All saved");
  await page.reload();
  await expect(backToWhereYouLeftOffButton(page)).toBeVisible();

  // when: the cursor is put in the first scene
  await scene(page, first).click();

  // then: the offer goes
  await expect(backToWhereYouLeftOffButton(page)).toHaveCount(0);
  // and: the cursor stays where it was put
  await expect(scene(page, first)).toBeFocused();
});

test("with the scene left off in already in sight, the way back is not offered", async ({
  page,
}) => {
  // given: the cursor left in the first scene
  await aNewProject(page, "InSight");
  const [first] = (await aBookWithScenes(page, { "Part One": 2 }))["Part One"];
  await openTheWritingView(page);
  await scene(page, first).click();
  await page.keyboard.type("Left here.");
  await expect(saveState(page)).toHaveText("All saved");
  // and: every showing of the way back kept
  await page.addInitScript(() => {
    new MutationObserver(() => {
      if (document.querySelector(".going-back")) {
        document.documentElement.dataset.offered = "once";
      }
    }).observe(document, { childList: true, subtree: true });
  });

  // when: the page is reloaded
  await page.reload();

  // then: the scene is in sight
  await expect(scene(page, first)).toBeInViewport();
  // and: the way back is never offered
  await expect(backToWhereYouLeftOffButton(page)).toHaveCount(0);
  expect(
    await page.evaluate(() => document.documentElement.dataset.offered ?? "never"),
    "the scene is right there to click into, so not even a flash of the offer",
  ).toBe("never");
});

test("the offer steps aside while its scene is in sight, and comes back once it is not", async ({
  page,
}) => {
  // given: the way back offered to a scene far down
  await aNewProject(page, "OfferAside");
  const scenes = await aBookWithScenes(page, { "Part One": 14 });
  const far = scenes["Part One"][11];
  await openTheWritingView(page);
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  await scene(page, far).click();
  await page.keyboard.type("Left here.");
  await expect(saveState(page)).toHaveText("All saved");
  await page.reload();
  await expect(backToWhereYouLeftOffButton(page)).toBeVisible();

  // when: the scene is scrolled into sight
  await flow(page).locator(`[data-place-of="${far}"]`).scrollIntoViewIfNeeded();
  // then: the offer steps aside
  await expect(backToWhereYouLeftOffButton(page)).toHaveCount(0);
  // when: the window is scrolled back to the start
  await page.evaluate(() => window.scrollTo(0, 0));

  // then: the offer comes back
  await expect(backToWhereYouLeftOffButton(page)).toBeVisible();
  // and: it still leads to the scene
  await backToWhereYouLeftOffButton(page).click();
  await expect(scene(page, far)).toBeFocused();
});
