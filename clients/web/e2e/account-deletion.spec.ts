import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";
import { API } from "./support/shell";

async function onHerAccountPage(page: Page) {
  await page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" })
    .click();
  await page.getByRole("menuitem", { name: "Your account" }).click();
  await expect(page.getByRole("heading", { name: "Your account", level: 1 })).toBeVisible();
}

async function askedToDelete(page: Page, password: string) {
  const asking = page.getByRole("dialog", { name: "Delete your account?" });
  await asking.getByLabel("Your password, to be sure it’s you").fill(password);
  await asking.getByRole("button", { name: "Delete my account" }).click();
}

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

test("she deletes her account from its own page for her password, and signing in again shows it pending", async ({
  page,
}) => {
  // given: she is signed in
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: she opens her account from the account menu
  await onHerAccountPage(page);

  // then: it shows who she is
  await expect(page.getByText(ADA.email)).toBeVisible();

  // when: she asks to delete it with a wrong password
  await page.getByRole("button", { name: "Delete this account…" }).click();
  await expect(page.getByText("Your account and your work are kept 30 days")).toBeVisible();
  await askedToDelete(page, "not her password");

  // then: she is told it is not her password
  await expect(page.getByText("That is not your password.")).toBeVisible();

  // when: she asks with her password
  await askedToDelete(page, HER_PASSWORD);

  // then: she is signed out
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  expect(server.deletion?.by).toBe("holder");

  // when: she signs in again
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: her account shows as to be deleted
  await expect(page.getByRole("heading", { name: "Your account is to be deleted" })).toBeVisible();
});

test("the last owner of a shared project is shown it to hand over first, linking to its members", async ({
  page,
}) => {
  // given: she is the last owner of a project another is in
  const server = await aServerWithAccounts(page);
  const title = `The Weavers’ Guild ${Date.now()}`;
  const made = await page.request.post(`${API}/projects`, {
    data: { name: title, language: "en" },
  });
  expect(made.ok()).toBe(true);
  const { id } = await made.json();
  server.toHandOver = [id];
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: she asks to delete her account
  await onHerAccountPage(page);
  await page.getByRole("button", { name: "Delete this account…" }).click();
  await askedToDelete(page, HER_PASSWORD);

  // then: she is shown the project to hand over first, by its name
  const handingOver = page.getByRole("dialog", { name: "Hand these projects over first" });
  await expect(handingOver.getByText(title)).toBeVisible();
  // and: a way to its members
  await expect(handingOver.getByRole("link", { name: "Members" })).toHaveAttribute(
    "href",
    `/projects/${id}/members`,
  );
  // and: her account is not to be deleted
  expect(server.deletion).toBeNull();
});
