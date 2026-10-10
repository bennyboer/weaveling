import { test, expect, type Page } from "@playwright/test";

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
});

async function answering(page: Page, answer: () => Refused[]) {
  await page.route(REFUSALS, (route) => route.fulfill({ json: answer() }));
}

async function aServerHolding(page: Page, start: Refused[]) {
  let held = start;
  const actedOn = (url: string) => Number(url.split("/").at(-2));

  await answering(page, () => held);
  await page.route(`${REFUSALS}/*/retry`, (route) => {
    const id = actedOn(route.request().url());
    held = held.filter((refusal) => refusal.id !== id);
    return route.fulfill({ status: 204 });
  });
  await page.route(`${REFUSALS}/*/acknowledge`, (route) => {
    const id = actedOn(route.request().url());
    held = held.map((refusal) =>
      refusal.id === id ? refused(id, true, refusal.plainly) : refusal,
    );
    return route.fulfill({ status: 204 });
  });
}

const asked = (page: Page) => page.waitForResponse(REFUSALS);

async function lookingAtTheRefusals(page: Page) {
  await page.goto("/");
  await alarm(page).click();
  await expect(dialog(page)).toBeVisible();
}

test("with nothing refused there is no alarm", async ({ page }) => {
  const first = asked(page);

  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  await expect(alarm(page)).toHaveCount(0);
});

test("a refusal raises the alarm", async ({ page }) => {
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  await expect(alarm(page)).toHaveClass(/raised/);
});

test("the alarm sits at the very right of the status bar", async ({ page }) => {
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  const seen = (await alarm(page).boundingBox())!;
  const wide = page.viewportSize()!.width;
  expect(wide - (seen.x + seen.width)).toBeLessThanOrEqual(12);
});

test("refusals already acknowledged keep the alarm quiet but within reach", async ({
  page,
}) => {
  await answering(page, () => [refused(1, true)]);
  const first = asked(page);

  await page.goto("/");
  await first;

  await expect(alarm(page)).toBeVisible();
  await page.waitForTimeout(A_BEAT);
  expect(await alarm(page).getAttribute("class")).not.toContain("raised");
});

test("a refusal arriving later raises the alarm within a few seconds", async ({
  page,
}) => {
  let answer: Refused[] = [];
  await answering(page, () => answer);
  const first = asked(page);
  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);
  await expect(alarm(page)).toHaveCount(0);

  answer = [refused(1)];

  await expect(alarm(page), "the client asks every five seconds").toHaveClass(
    /raised/,
    { timeout: 7_000 },
  );
});

test("a new refusal pulses, then settles, and is not pulsed for again", async ({
  page,
}) => {
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  await expect(alarm(page)).toHaveClass(/pulsing/);
  await expect(alarm(page)).not.toHaveClass(/pulsing/, { timeout: 3_000 });
  await asked(page);
  await page.waitForTimeout(A_BEAT);
  expect(
    await alarm(page).getAttribute("class"),
    "a refusal already heard must not keep drawing the eye",
  ).not.toContain("pulsing");
});

test("the alarm does not flash for someone who asked for less motion", async ({
  page,
}) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  await expect(alarm(page)).toBeVisible();
  expect(
    await alarm(page).evaluate((seen) => getComputedStyle(seen).animationName),
  ).toBe("none");
});

test("the alarm opens what was refused, in plain words", async ({ page }) => {
  await aServerHolding(page, [refused(1)]);

  await lookingAtTheRefusals(page);

  await expect(entries(page)).toHaveCount(1);
  await expect(entries(page)).toContainText(
    "A change to an idea did not reach the list of ideas.",
  );
  await expect(
    entries(page).getByText("the projection had not landed"),
    "the technical reason stays folded away until asked for",
  ).toBeHidden();

  await entries(page).getByText("Details").click();

  await expect(
    entries(page).getByText("the projection had not landed"),
  ).toBeVisible();
  await expect(entries(page).getByText("catalogue-idea")).toBeVisible();
});

test("a refusal nobody has words for still says something", async ({
  page,
}) => {
  await aServerHolding(page, [refused(1, false, null)]);

  await lookingAtTheRefusals(page);

  await expect(entries(page)).toContainText(
    "A change did not reach everywhere it should have.",
  );
});

test("trying again sends the change on and takes it off the list", async ({
  page,
}) => {
  await aServerHolding(page, [refused(1), refused(2)]);
  await lookingAtTheRefusals(page);
  const retried = page.waitForRequest(`${REFUSALS}/1/retry`);

  await entries(page)
    .first()
    .getByRole("button", { name: "Try again" })
    .click();

  expect((await retried).method()).toBe("POST");
  await expect(entries(page)).toHaveCount(1);
});

test("trying the last one again leaves nothing to look at, and no alarm", async ({
  page,
}) => {
  await aServerHolding(page, [refused(1)]);
  await lookingAtTheRefusals(page);

  await entries(page).getByRole("button", { name: "Try again" }).click();

  await expect(dialog(page)).toContainText("Nothing is left to look at.");
  await expect(alarm(page)).toHaveCount(0);
});

test("acknowledging keeps the change listed, greyed, and quiets the alarm", async ({
  page,
}) => {
  await aServerHolding(page, [refused(1)]);
  await lookingAtTheRefusals(page);

  await entries(page).getByRole("button", { name: "Acknowledge" }).click();

  await expect(entries(page)).toHaveClass(/acknowledged/);
  await expect(
    entries(page).getByRole("button", { name: "Acknowledge" }),
  ).toHaveCount(0);
  await expect(
    entries(page).getByRole("button", { name: "Try again" }),
    "acknowledged is not given up on, so it can still be tried again",
  ).toBeVisible();
  expect(await alarm(page).getAttribute("class")).not.toContain("raised");
});

test("the dialog closes by its button, by Escape and by clicking outside", async ({
  page,
}) => {
  await aServerHolding(page, [refused(1)]);

  await lookingAtTheRefusals(page);
  await dialog(page).getByRole("button", { name: "Close" }).click();
  await expect(dialog(page)).toHaveCount(0);

  await alarm(page).click();
  await page.keyboard.press("Escape");
  await expect(dialog(page)).toHaveCount(0);

  await alarm(page).click();
  await page.mouse.click(5, 5);
  await expect(dialog(page)).toHaveCount(0);
});

const note = (page: Page) =>
  page
    .getByRole("status")
    .filter({ hasText: "Some changes could not be applied." });

test("a new refusal brings a note pointing at the alarm", async ({ page }) => {
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  await expect(note(page)).toBeVisible();
});

test("refusals arriving together bring a single note", async ({ page }) => {
  await answering(page, () => [refused(1), refused(2), refused(3)]);

  await page.goto("/");

  await expect(note(page)).toHaveCount(1);
});

test("refusals already acknowledged bring no note", async ({ page }) => {
  await answering(page, () => [refused(1, true)]);
  const first = asked(page);

  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  await expect(note(page)).toHaveCount(0);
});

test("the note opens the refusals and goes away", async ({ page }) => {
  await aServerHolding(page, [refused(1)]);
  await page.goto("/");

  await note(page).getByRole("button", { name: "Show" }).click();

  await expect(dialog(page)).toBeVisible();
  await expect(note(page)).toHaveCount(0);
});

test("a dismissed note stays away for refusals already heard of", async ({
  page,
}) => {
  await answering(page, () => [refused(1)]);
  await page.goto("/");

  await note(page).getByRole("button", { name: "Dismiss" }).click();
  await expect(note(page)).toHaveCount(0);
  await asked(page);
  await page.waitForTimeout(A_BEAT);

  await expect(note(page)).toHaveCount(0);
  await expect(
    alarm(page),
    "dismissing the note is not acknowledging what was refused",
  ).toHaveClass(/raised/);
});

test("a refusal arriving after a dismissal brings the note back", async ({
  page,
}) => {
  let answer = [refused(1)];
  await answering(page, () => answer);
  await page.goto("/");
  await note(page).getByRole("button", { name: "Dismiss" }).click();

  answer = [refused(1), refused(2)];

  await expect(note(page)).toBeVisible({ timeout: 7_000 });
});
