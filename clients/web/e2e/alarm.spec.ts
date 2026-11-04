import { test, expect, type Page } from "@playwright/test";

import { routingLive } from "./support/live";

const REFUSALS = "**/api/service/refusals";

const A_BEAT = 500;

type Refused = ReturnType<typeof refused>;

const alarm = (page: Page) =>
  page
    .getByRole("contentinfo", { name: "Status" })
    .getByRole("button", { name: "Changes that could not be applied" });

const dialog = (page: Page) =>
  page.getByRole("dialog", { name: "Changes that could not be applied" });

const entries = (page: Page) =>
  dialog(page)
    .getByRole("list", { name: "Refused changes" })
    .getByRole("listitem");

const refused = (
  id: number,
  acknowledged = false,
  plainly:
    string | null = "A change to an idea did not reach the list of ideas.",
) => ({
  id,
  listener: "catalogue-idea",
  routing: "idea.captured",
  attempts: 5,
  why: "the projection had not landed",
  plainly,
  occurred_at: "2026-10-10T09:00:00Z",
  given_up_at: "2026-10-10T09:01:00Z",
  acknowledged_at: acknowledged ? "2026-10-10T09:02:00Z" : null,
  may_be_tried_again: true,
});

async function aServerAnswering(page: Page, answer: () => Refused[]) {
  await page.route(REFUSALS, (route) => route.fulfill({ json: answer() }));
}

async function aServerHolding(page: Page, start: Refused[]) {
  let held = start;
  const actedOn = (url: string) => Number(url.split("/").at(-2));

  await aServerAnswering(page, () => held);
  await page.route(`${REFUSALS}/*/retry`, (route) => {
    const id = actedOn(route.request().url());
    held = held.filter((refusal) => refusal.id !== id);
    return route.fulfill({ status: 204 });
  });
  await page.route(`${REFUSALS}/*/acknowledge`, (route) => {
    const id = actedOn(route.request().url());
    held = held.map((refusal) =>
      refusal.id === id
        ? { ...refusal, acknowledged_at: refused(id, true).acknowledged_at }
        : refusal,
    );
    return route.fulfill({ status: 204 });
  });
}

const asked = (page: Page) => page.waitForResponse(REFUSALS);

async function told(page: Page) {
  const live = await routingLive(page, { withheld: ["refusals"] });
  let changes = 0;

  return () => {
    changes += 1;
    live.tell("refusals", { version: changes, event: { name: "REFUSALS_CHANGED" } });
  };
}

async function lookingAtTheRefusals(page: Page) {
  await page.goto("/");
  await alarm(page).click();
  await expect(dialog(page)).toBeVisible();
}

test("with nothing refused there is no alarm", async ({ page }) => {
  // given: nothing refused
  const first = asked(page);

  // when: the app is opened
  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  // then: there is no alarm
  await expect(alarm(page)).toHaveCount(0);
});

test("a refusal raises the alarm", async ({ page }) => {
  // given: a refused change
  await aServerAnswering(page, () => [refused(1)]);

  // when: the app is opened
  await page.goto("/");

  // then: the alarm is raised
  await expect(alarm(page)).toHaveClass(/raised/);
});

test("the alarm sits at the very right of the status bar", async ({ page }) => {
  // given: a refused change
  await aServerAnswering(page, () => [refused(1)]);

  // when: the app is opened
  await page.goto("/");

  // then: the alarm sits at the right edge of the status bar
  const seen = (await alarm(page).boundingBox())!;
  const wide = page.viewportSize()!.width;
  expect(wide - (seen.x + seen.width)).toBeLessThanOrEqual(12);
});

test("refusals already acknowledged keep the alarm quiet but within reach", async ({
  page,
}) => {
  // given: a refused change already acknowledged
  await aServerAnswering(page, () => [refused(1, true)]);
  const first = asked(page);

  // when: the app is opened
  await page.goto("/");
  await first;

  // then: the alarm is there but not raised
  await expect(alarm(page)).toBeVisible();
  await page.waitForTimeout(A_BEAT);
  expect(await alarm(page).getAttribute("class")).not.toContain("raised");
});

test("a refusal arriving later raises the alarm within a few seconds", async ({
  page,
}) => {
  // given: the app open with nothing refused
  let answer: Refused[] = [];
  await aServerAnswering(page, () => answer);
  const tell = await told(page);
  const first = asked(page);
  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);
  await expect(alarm(page)).toHaveCount(0);

  // when: a change is refused and the client is told
  answer = [refused(1)];
  tell();

  // then: the alarm is raised
  await expect(alarm(page), "told of it, the client asks at once").toHaveClass(
    /raised/,
  );
});

test("a new refusal pulses, then settles, and is not pulsed for again", async ({
  page,
}) => {
  // given: a refused change
  await aServerAnswering(page, () => [refused(1)]);
  const tell = await told(page);

  // when: the app is opened
  await page.goto("/");

  // then: the alarm pulses, then settles
  await expect(alarm(page)).toHaveClass(/pulsing/);
  await expect(alarm(page)).not.toHaveClass(/pulsing/, { timeout: 3_000 });
  // when: the client is told the refusals changed
  const again = asked(page);
  tell();
  await again;
  await page.waitForTimeout(A_BEAT);
  // then: the alarm does not pulse again for the same refusal
  expect(
    await alarm(page).getAttribute("class"),
    "a refusal already heard must not keep drawing the eye",
  ).not.toContain("pulsing");
});

test("the alarm does not flash for someone who asked for less motion", async ({
  page,
}) => {
  // given: someone who asked for less motion
  await page.emulateMedia({ reducedMotion: "reduce" });
  // and: a refused change
  await aServerAnswering(page, () => [refused(1)]);

  // when: the app is opened
  await page.goto("/");

  // then: the alarm shows without animating
  await expect(alarm(page)).toBeVisible();
  expect(
    await alarm(page).evaluate((seen) => getComputedStyle(seen).animationName),
  ).toBe("none");
});

test("the alarm opens what was refused, in plain words", async ({ page }) => {
  // given: a refused change
  await aServerHolding(page, [refused(1)]);

  // when: the alarm is opened
  await lookingAtTheRefusals(page);

  // then: the change is listed in plain words, its technical reason folded away
  await expect(entries(page)).toHaveCount(1);
  await expect(entries(page)).toContainText(
    "A change to an idea did not reach the list of ideas.",
  );
  await expect(
    entries(page).getByText("the projection had not landed"),
    "the technical reason stays folded away until asked for",
  ).toBeHidden();

  // when: its details are opened
  await entries(page).getByText("Details").click();

  // then: the technical reason shows
  await expect(
    entries(page).getByText("the projection had not landed"),
  ).toBeVisible();
  await expect(entries(page).getByText("catalogue-idea")).toBeVisible();
});

test("a refusal nobody has words for still says something", async ({
  page,
}) => {
  // given: a refused change without plain words
  await aServerHolding(page, [refused(1, false, null)]);

  // when: the alarm is opened
  await lookingAtTheRefusals(page);

  // then: a general sentence stands in for them
  await expect(entries(page)).toContainText(
    "A change did not reach everywhere it should have.",
  );
});

test("trying again sends the change on and takes it off the list", async ({
  page,
}) => {
  // given: two refused changes listed
  await aServerHolding(page, [refused(1), refused(2)]);
  await lookingAtTheRefusals(page);
  const retried = page.waitForRequest(`${REFUSALS}/1/retry`);

  // when: the first is tried again
  await entries(page)
    .first()
    .getByRole("button", { name: "Try again" })
    .click();

  // then: it is sent on and leaves the list
  expect((await retried).method()).toBe("POST");
  await expect(entries(page)).toHaveCount(1);
});

test("trying the last one again leaves nothing to look at, and no alarm", async ({
  page,
}) => {
  // given: a single refused change listed
  await aServerHolding(page, [refused(1)]);
  await lookingAtTheRefusals(page);

  // when: it is tried again
  await entries(page).getByRole("button", { name: "Try again" }).click();

  // then: nothing is left to look at
  await expect(dialog(page)).toContainText("Nothing is left to look at.");
  // and: there is no alarm
  await expect(alarm(page)).toHaveCount(0);
});

test("acknowledging keeps the change listed, greyed, and quiets the alarm", async ({
  page,
}) => {
  // given: a refused change listed
  await aServerHolding(page, [refused(1)]);
  await lookingAtTheRefusals(page);

  // when: it is acknowledged
  await entries(page).getByRole("button", { name: "Acknowledge" }).click();

  // then: it stays listed, greyed, and can still be tried again
  await expect(entries(page)).toHaveClass(/acknowledged/);
  await expect(
    entries(page).getByRole("button", { name: "Acknowledge" }),
  ).toHaveCount(0);
  await expect(
    entries(page).getByRole("button", { name: "Try again" }),
    "acknowledged is not given up on, so it can still be tried again",
  ).toBeVisible();
  // and: the alarm is quiet
  expect(await alarm(page).getAttribute("class")).not.toContain("raised");
});

test("a change too old to try again says so and offers only acknowledging", async ({
  page,
}) => {
  // given: a refused change and one too old to try again
  await aServerHolding(page, [
    refused(1),
    { ...refused(2), may_be_tried_again: false },
  ]);
  // when: the alarm is opened
  await lookingAtTheRefusals(page);
  await expect(entries(page)).toHaveCount(2);

  // then: the old one says so and offers only acknowledging
  await expect(
    entries(page).nth(1).getByRole("button", { name: "Try again" }),
  ).toHaveCount(0);
  await expect(entries(page).nth(1)).toContainText(
    "This change happened over thirty days ago and is not tried again",
  );
  await expect(
    entries(page).nth(1).getByRole("button", { name: "Acknowledge" }),
  ).toBeVisible();
  // and: the other can still be tried again
  await expect(
    entries(page).first().getByRole("button", { name: "Try again" }),
  ).toBeVisible();
  await expect(entries(page).first()).not.toContainText("thirty days");
});

test("the dialog closes by its button, by Escape and by clicking outside", async ({
  page,
}) => {
  // given: a refused change
  await aServerHolding(page, [refused(1)]);

  // when: the dialog is opened and closed by its button
  await lookingAtTheRefusals(page);
  await dialog(page).getByRole("button", { name: "Close" }).click();
  // then: it closes
  await expect(dialog(page)).toHaveCount(0);

  // when: it is opened and escape pressed
  await alarm(page).click();
  await page.keyboard.press("Escape");
  // then: it closes
  await expect(dialog(page)).toHaveCount(0);

  // when: it is opened and somewhere outside clicked
  await alarm(page).click();
  await page.mouse.click(5, 5);
  // then: it closes
  await expect(dialog(page)).toHaveCount(0);
});

const note = (page: Page) =>
  page
    .getByRole("status")
    .filter({ hasText: "Some changes could not be applied." });

test("a new refusal brings a note pointing at the alarm", async ({ page }) => {
  // given: a new refused change
  await aServerAnswering(page, () => [refused(1)]);

  // when: the app is opened
  await page.goto("/");

  // then: a note points at the alarm
  await expect(note(page)).toBeVisible();
});

test("refusals arriving together bring a single note", async ({ page }) => {
  // given: three refused changes
  await aServerAnswering(page, () => [refused(1), refused(2), refused(3)]);

  // when: the app is opened
  await page.goto("/");

  // then: a single note shows
  await expect(note(page)).toHaveCount(1);
});

test("refusals already acknowledged bring no note", async ({ page }) => {
  // given: a refused change already acknowledged
  await aServerAnswering(page, () => [refused(1, true)]);
  const first = asked(page);

  // when: the app is opened
  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  // then: no note shows
  await expect(note(page)).toHaveCount(0);
});

test("the note opens the refusals and goes away", async ({ page }) => {
  // given: the app open with a refused change
  await aServerHolding(page, [refused(1)]);
  await page.goto("/");

  // when: the note is followed
  await note(page).getByRole("button", { name: "Show" }).click();

  // then: the refusals open
  await expect(dialog(page)).toBeVisible();
  // and: the note goes away
  await expect(note(page)).toHaveCount(0);
});

test("a dismissed note stays away for refusals already heard of", async ({
  page,
}) => {
  // given: the app open with a refused change
  await aServerAnswering(page, () => [refused(1)]);
  const tell = await told(page);
  await page.goto("/");

  // when: its note is dismissed
  await note(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(note(page)).toHaveCount(0);
  // and: the client is told the refusals changed
  const again = asked(page);
  tell();
  await again;
  await page.waitForTimeout(A_BEAT);

  // then: the note stays away
  await expect(note(page)).toHaveCount(0);
  // and: the alarm stays raised
  await expect(
    alarm(page),
    "dismissing the note is not acknowledging what was refused",
  ).toHaveClass(/raised/);
});

test("a refusal arriving after a dismissal brings the note back", async ({
  page,
}) => {
  // given: the app open with a refused change whose note was dismissed
  let answer = [refused(1)];
  await aServerAnswering(page, () => answer);
  const tell = await told(page);
  await page.goto("/");
  await note(page).getByRole("button", { name: "Dismiss" }).click();

  // when: another change is refused and the client is told
  answer = [refused(1), refused(2)];
  tell();

  // then: the note is back
  await expect(note(page)).toBeVisible();
});

test("the alarm asks again when it is told the refusals changed, and not otherwise", async ({
  page,
}) => {
  // given: nothing refused yet
  let asking = 0;
  await page.route(REFUSALS, (route) => {
    asking += 1;
    return route.fulfill({ json: asking > 1 ? [refused(7)] : [] });
  });
  const tell = await told(page);

  // when: the app is opened and nothing is told
  await page.goto("/");
  await expect.poll(() => asking).toBe(1);
  await page.waitForTimeout(6000);
  // then: it asks once only
  expect(asking, "nothing is asked for while nothing is told").toBe(1);

  // when: it is told the refusals changed
  tell();

  // then: it asks again and raises the alarm
  await expect(alarm(page)).toBeVisible();
  expect(asking).toBe(2);
});
