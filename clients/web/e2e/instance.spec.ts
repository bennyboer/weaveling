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
  // given: an operator looking at the accounts
  await anOperatorWith(page);

  // then: every account is listed with what it is
  await expect(
    page
      .getByRole("navigation", { name: "Sections" })
      .getByRole("link", { name: "Accounts" }),
  ).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("main").getByRole("listitem")).toHaveCount(3);
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
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they lock an account for a reason
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Sent spam to every member.");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();

  // then: it is locked, the reason shown
  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  await expect(row(page, "Grace Hopper")).toContainText(
    "Sent spam to every member.",
  );
  expect(server.listed[1].locks[0].reason).toBe("Sent spam to every member.");

  // when: they unlock it
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Unlock" })
    .click();

  // then: it is no longer locked
  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Lock…" }),
  ).toBeVisible();
  expect(server.listed[1].locks).toEqual([]);
});

test("each change to an account says the version it was made on", async ({
  page,
}) => {
  // given: an operator looking at an account at version five
  await anOperatorWith(page, [HEDY, { ...GRACE, version: 5 } as typeof GRACE]);
  const madeOn: (string | null)[] = [];
  page.on("request", (request) => {
    if (/\/lock$/.test(new URL(request.url()).pathname)) {
      madeOn.push(request.headers()["if-match"] ?? null);
    }
  });

  // when: they lock it
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Sent spam to every member.");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  // and: unlock it again
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Unlock" })
    .click();
  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");

  // then: the lock was made on version five, the unlock on the one after
  await expect.poll(() => madeOn).toEqual(['"5"', '"6"']);
});

test("a lock without a reason is refused and the dialog stays", async ({
  page,
}) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they lock an account without a reason
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("   ");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();

  // then: it is refused and the dialog stays
  await expect(dialog.getByRole("alert")).toHaveText(
    "Locking an account needs a reason.",
  );
  await expect(dialog).toBeVisible();
  expect(server.listed[1].locks).toEqual([]);
});

test("the dialog's hint stands apart from the reason it asks for", async ({
  page,
}) => {
  // given: an operator locking an account
  await anOperatorWith(page);
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });

  // then: the dialog's hint stands apart from the reason
  const hint = await dialog.getByText("is signed out at once").boundingBox();
  const label = await dialog.getByText("Reason", { exact: true }).boundingBox();

  expect(hint && label).toBeTruthy();
  expect(label!.y - (hint!.y + hint!.height)).toBeGreaterThanOrEqual(16);
});

test("cancelling the dialog locks nobody", async ({ page }) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they begin locking an account and cancel
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Lock…" })
    .click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Changed my mind.");
  await dialog.getByRole("button", { name: "Cancel" }).click();

  // then: nobody is locked
  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");
  expect(server.listed[1].locks).toEqual([]);
});

test("a long list comes a page at a time", async ({ page }) => {
  // given: an operator and more accounts than fit a page
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

  // then: the first page is listed
  await expect(page.getByRole("main").getByRole("listitem")).toHaveCount(1);
  // when: more is asked for
  await page.getByRole("button", { name: "Show more" }).click();
  // then: the next page follows
  await expect(page.getByRole("main").getByRole("listitem")).toHaveCount(2);
  // when: more is asked for again
  await page.getByRole("button", { name: "Show more" }).click();

  // then: all are listed and no more is offered
  await expect(page.getByRole("main").getByRole("listitem")).toHaveCount(3);
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);
});

test("an operator makes another account an operator", async ({ page }) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they make another account an operator
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Make operator" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Make Grace Hopper an operator",
  });
  await dialog.getByRole("button", { name: "Make operator" }).click();

  // then: it is an operator, no longer offered locking
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
  // given: an operator looking at the accounts, with another operator
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, operator: true },
  ]);

  // when: they take the role away from the other
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Take the operator role from Grace Hopper",
  });
  await dialog.getByRole("button", { name: "Take away the role" }).click();

  // then: the other is no longer an operator
  await expect(dialog).toHaveCount(0);
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Make operator" }),
  ).toBeVisible();
  expect(server.listed[1].operator).toBe(false);
});

test("the last operator is told the role stays", async ({ page }) => {
  // given: the last operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they begin taking the role from themselves
  await row(page, "Ada Lovelace")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Take the operator role from Ada Lovelace",
  });
  // then: they are warned
  await expect(dialog).toContainText(
    "only another operator can give the role back",
  );
  // when: they go on
  await dialog.getByRole("button", { name: "Take away the role" }).click();

  // then: they are told the role stays
  await expect(dialog.getByRole("alert")).toHaveText(
    "The last operator keeps the role, or nobody could manage this Weaveling instance.",
  );
  expect(server.operator).toBe(true);
});

test("an operator giving up the role is no longer shown the instance", async ({
  page,
}) => {
  // given: an operator looking at the accounts, with another operator
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, operator: true },
  ]);

  // when: they give up the role
  await row(page, "Ada Lovelace")
    .getByRole("button", { name: "Take away the operator role" })
    .click();
  await page
    .getByRole("dialog", { name: "Take the operator role from Ada Lovelace" })
    .getByRole("button", { name: "Take away the role" })
    .click();

  // then: they are no longer shown the instance
  await expect(page).toHaveURL(/\/setup$/);
  await expect(page.getByRole("main")).toContainText(
    "This Weaveling instance is set up already",
  );
  expect(server.operator).toBe(false);
});

test("an unconfirmed or locked account is not offered the role", async ({
  page,
}) => {
  // given: an operator looking at an unconfirmed and a locked account
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

  // then: neither is offered the role
  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  await expect(
    row(page, "Hedy Lamarr").getByRole("button", { name: "Make operator" }),
  ).toHaveCount(0);
  await expect(
    row(page, "Grace Hopper").getByRole("button", { name: "Make operator" }),
  ).toHaveCount(0);
});

test("cancelling leaves the role as it was", async ({ page }) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they begin making another an operator and cancel
  await row(page, "Grace Hopper")
    .getByRole("button", { name: "Make operator" })
    .click();
  await page
    .getByRole("dialog", { name: "Make Grace Hopper an operator" })
    .getByRole("button", { name: "Cancel" })
    .click();

  // then: the role is as it was
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(server.listed[1].operator).toBe(false);
});

test("every row keeps its buttons beside what it tells, marked or not", async ({
  page,
}) => {
  // given: a narrow window
  await page.setViewportSize({ width: 700, height: 800 });
  // and: an operator looking at the accounts
  await anOperatorWith(page);

  // then: every row keeps its buttons beside what it tells, aligned
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
