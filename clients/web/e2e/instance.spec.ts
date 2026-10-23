import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  GRACE,
  HEDY,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  type Server,
} from "./support/accounts";

async function anOperatorWith(
  page: Page,
  listed = [HEDY, GRACE],
): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.operator = true;
  server.listed = listed.map((account) => ({ ...account, locks: [] }));
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page
      .getByRole("banner")
      .getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
  await page.goto("/instance");

  return server;
}

const row = (page: Page, name: string) =>
  page.getByRole("listitem").filter({ hasText: name });

test("the operator sees every account with what it is", async ({ page }) => {
  await anOperatorWith(page);

  await expect(page.getByRole("heading", { name: "Accounts" })).toBeVisible();
  await expect(page.getByRole("listitem")).toHaveCount(3);
  await expect(row(page, "Hedy Lamarr")).toContainText("hedy@example.com");
  await expect(row(page, "Hedy Lamarr")).toContainText("Address not confirmed");
  await expect(row(page, "Grace Hopper")).not.toContainText(
    "Address not confirmed",
  );
  await expect(row(page, "Ada Lovelace")).toContainText("Operator");
  await expect(row(page, "Grace Hopper")).toContainText(
    "Signed up on 22 Oct 2026",
  );
  await expect(
    row(page, "Ada Lovelace").getByRole("button", { name: "Lock…" }),
  ).toHaveCount(0);
});

test("an operator locks an account for a reason and unlocks it again", async ({
  page,
}) => {
  const server = await anOperatorWith(page);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Sent spam to every member.");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();

  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  await expect(row(page, "Grace Hopper")).toContainText(
    "Sent spam to every member.",
  );
  expect(server.listed[1].locks[0].reason).toBe("Sent spam to every member.");

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Unlock" })
    .click();

  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Lock…" }),
  ).toBeVisible();
  expect(server.listed[1].locks).toEqual([]);
});

test("a lock without a reason is refused and the dialog stays", async ({
  page,
}) => {
  const server = await anOperatorWith(page);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("   ");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();

  await expect(dialog.getByRole("alert")).toHaveText(
    "Locking an account needs a reason.",
  );
  await expect(dialog).toBeVisible();
  expect(server.listed[1].locks).toEqual([]);
});

test("the dialog's hint stands apart from the reason it asks for", async ({
  page,
}) => {
  await anOperatorWith(page);
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });

  const hint = await dialog.getByText("is signed out at once").boundingBox();
  const label = await dialog.getByText("Reason", { exact: true }).boundingBox();

  expect(hint && label).toBeTruthy();
  expect(label!.y - (hint!.y + hint!.height)).toBeGreaterThanOrEqual(16);
});

test("cancelling the dialog locks nobody", async ({ page }) => {
  const server = await anOperatorWith(page);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Changed my mind.");
  await dialog.getByRole("button", { name: "Cancel" }).click();

  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");
  expect(server.listed[1].locks).toEqual([]);
});

test("a long list comes a page at a time", async ({ page }) => {
  const server = await aServerWithAccounts(page);
  server.operator = true;
  server.listed = [HEDY, GRACE];
  server.pageSize = 1;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page
      .getByRole("banner")
      .getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
  await page.goto("/instance");

  await expect(page.getByRole("listitem")).toHaveCount(1);
  await page.getByRole("button", { name: "Show more" }).click();
  await expect(page.getByRole("listitem")).toHaveCount(2);
  await page.getByRole("button", { name: "Show more" }).click();

  await expect(page.getByRole("listitem")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);
});
