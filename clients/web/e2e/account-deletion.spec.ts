import { test, expect } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";

const ON_ITS_DAY = "2026-12-09T10:00:00Z";

test("signing in to an account she asked to delete tells when it goes, and keeping it signs her in", async ({
  page,
}) => {
  // given: an account its holder asked to be deleted
  const server = await aServerWithAccounts(page);
  server.deletion = { until: ON_ITS_DAY, by: "holder" };

  // when: she signs in
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: she is told when she asked and when it goes, and nothing of her work is shown
  await expect(page.getByRole("heading", { name: "Your account is to be deleted" })).toBeVisible();
  await expect(page.getByText("You asked for it on 9 Nov 2026. On 9 Dec 2026")).toBeVisible();
  await expect(page.getByRole("button", { name: "Notifications" })).toHaveCount(0);

  // when: she keeps her account
  await page.getByRole("button", { name: "Keep my account" }).click();

  // then: she is signed in as before
  await expect(page.getByRole("button", { name: "Notifications" })).toBeVisible();
  expect(server.keptAccount).toBe(1);
});

test("an account the operator marked offers no keeping, only signing out", async ({ page }) => {
  // given: an account the operator marked to be deleted
  const server = await aServerWithAccounts(page);
  server.deletion = { until: ON_ITS_DAY, by: "operator" };

  // when: she signs in
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: she is told who marked it and to ask them
  await expect(
    page.getByText(
      "Whoever operates this Weaveling instance marked your account to be deleted on 9 Dec 2026. Ask them if it should stay.",
    ),
  ).toBeVisible();
  // and: she cannot keep it herself
  await expect(page.getByRole("button", { name: "Keep my account" })).toHaveCount(0);

  // when: she signs out
  await page.getByRole("button", { name: "Sign out" }).click();

  // then: she is asked to sign in
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  expect(server.signedOut).toBe(1);
});

test("a page opened anew while deletion is pending shows the deletion again", async ({ page }) => {
  // given: she signed in to an account she asked to be deleted
  const server = await aServerWithAccounts(page);
  server.deletion = { until: ON_ITS_DAY, by: "holder" };
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Your account is to be deleted" })).toBeVisible();

  // when: the page is opened anew
  await page.reload();

  // then: it shows the deletion again, not her work
  await expect(page.getByRole("heading", { name: "Your account is to be deleted" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Notifications" })).toHaveCount(0);
});
