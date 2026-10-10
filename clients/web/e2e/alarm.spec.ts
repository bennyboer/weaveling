import { test, expect, type Page } from "@playwright/test";

const REFUSALS = "**/api/service/refusals";

const alarm = (page: Page) =>
  page
    .getByRole("contentinfo", { name: "Status" })
    .getByRole("img", { name: "Some changes could not be applied" });

const refused = (id: number, acknowledged = false) => ({
  id,
  listener: "catalogue-idea",
  routing: "idea.captured",
  attempts: 5,
  why: "the projection had not landed",
  occurred_at: "2026-10-10T09:00:00Z",
  given_up_at: "2026-10-10T09:01:00Z",
  acknowledged_at: acknowledged ? "2026-10-10T09:02:00Z" : null,
});

async function answering(page: Page, answer: () => object[]) {
  await page.route(REFUSALS, (route) => route.fulfill({ json: answer() }));
}

const asked = (page: Page) => page.waitForResponse(REFUSALS);

const A_BEAT = 500;

test("with nothing refused the alarm stays down", async ({ page }) => {
  const first = asked(page);

  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  await expect(alarm(page)).toHaveCount(0);
});

test("a refusal raises the alarm", async ({ page }) => {
  await answering(page, () => [refused(1)]);

  await page.goto("/");

  await expect(alarm(page)).toBeVisible();
});

test("refusals already acknowledged leave the alarm down", async ({ page }) => {
  await answering(page, () => [refused(1, true)]);
  const first = asked(page);

  await page.goto("/");
  await first;
  await page.waitForTimeout(A_BEAT);

  await expect(alarm(page)).toHaveCount(0);
});

test("a refusal arriving later raises the alarm within a few seconds", async ({
  page,
}) => {
  let answer: object[] = [];
  await answering(page, () => answer);
  const first = asked(page);
  await page.goto("/");
  await first;
  await expect(alarm(page)).toHaveCount(0);

  answer = [refused(1)];

  await expect(alarm(page), "the client asks every five seconds").toBeVisible({
    timeout: 7_000,
  });
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
