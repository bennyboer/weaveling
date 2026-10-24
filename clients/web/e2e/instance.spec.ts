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
  server.listed = listed.map((account) => ({
    ...account,
    locks: [...account.locks],
  }));
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

  await expect(
    page
      .getByRole("navigation", { name: "Sections" })
      .getByRole("link", { name: "Accounts" }),
  ).toHaveAttribute("aria-current", "page");
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

test("an operator makes another account an operator", async ({ page }) => {
  const server = await anOperatorWith(page);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Make operator" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Make Grace Hopper an operator",
  });
  await dialog.getByRole("button", { name: "Make operator" }).click();

  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Operator");
  await expect(
    row(page, "Grace Hopper").getByRole("button", {
      name: "Take away the operator role",
    }),
  ).toBeVisible();
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Lock…" }),
  ).toHaveCount(0);
  expect(server.listed[1].operator).toBe(true);
});

test("an operator takes the role away from another", async ({ page }) => {
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, operator: true },
  ]);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Take the operator role from Grace Hopper",
  });
  await dialog.getByRole("button", { name: "Take away the role" }).click();

  await expect(dialog).toHaveCount(0);
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Make operator" }),
  ).toBeVisible();
  expect(server.listed[1].operator).toBe(false);
});

test("the last operator is told the role stays", async ({ page }) => {
  const server = await anOperatorWith(page);

  await row(page, "Ada Lovelace")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Take the operator role from Ada Lovelace",
  });
  await expect(dialog).toContainText(
    "only another operator can give the role back",
  );
  await dialog.getByRole("button", { name: "Take away the role" }).click();

  await expect(dialog.getByRole("alert")).toHaveText(
    "The last operator keeps the role, or nobody could manage this Weaveling instance.",
  );
  expect(server.operator).toBe(true);
});

test("an operator giving up the role is no longer shown the instance", async ({
  page,
}) => {
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, operator: true },
  ]);

  await row(page, "Ada Lovelace")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  await page
    .getByRole("dialog", { name: "Take the operator role from Ada Lovelace" })
    .getByRole("button", { name: "Take away the role" })
    .click();

  await expect(page).toHaveURL(/\/setup$/);
  await expect(page.getByRole("main")).toContainText(
    "This Weaveling instance is set up already",
  );
  expect(server.operator).toBe(false);
});

test("an unconfirmed or locked account is not offered the role", async ({
  page,
}) => {
  await anOperatorWith(page, [
    HEDY,
    {
      ...GRACE,
      locks: [
        {
          kind: "operator",
          reason: "Sent spam.",
          since: "2026-10-23T09:30:00Z",
        },
      ],
    },
  ]);

  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  await expect(
    row(page, "Hedy Lamarr").getByRole("button", { name: "Make operator" }),
  ).toHaveCount(0);
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Make operator" }),
  ).toHaveCount(0);
});

test("cancelling leaves the role as it was", async ({ page }) => {
  const server = await anOperatorWith(page);

  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Make operator" })
    .click();
  await page
    .getByRole("dialog", { name: "Make Grace Hopper an operator" })
    .getByRole("button", { name: "Cancel" })
    .click();

  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(server.listed[1].operator).toBe(false);
});

test("every row keeps its buttons beside what it tells, marked or not", async ({
  page,
}) => {
  await page.setViewportSize({ width: 700, height: 800 });
  await anOperatorWith(page);

  const rightEdges: number[] = [];
  for (const name of ["Hedy Lamarr", "Grace Hopper", "Ada Lovelace"]) {
    const who = await row(page, name).locator(".who").boundingBox();
    const actions = await row(page, name).locator(".actions").boundingBox();
    expect(who && actions).toBeTruthy();
    expect(actions!.y, `${name}'s buttons stand beside the infos`).toBeLessThan(
      who!.y + who!.height,
    );
    rightEdges.push(Math.round(actions!.x + actions!.width));
  }

  expect(new Set(rightEdges).size, `right edges ${rightEdges}`).toBe(1);
});
