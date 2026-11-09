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

async function offered(page: Page, name: string, action: string) {
  const opening = row(page, name).getByRole("button", {
    name: `Actions for ${name}`,
  });
  if ((await opening.getAttribute("aria-expanded")) !== "true") {
    await opening.click();
  }

  return row(page, name).getByRole("menuitem", { name: action });
}

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
  await expect(await offered(page, "Ada Lovelace", "Lock…")).toHaveCount(0);
});

test("an operator locks an account for a reason and unlocks it again", async ({
  page,
}) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they lock an account for a reason
  await (await offered(page, "Grace Hopper", "Lock…")).click();
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
  await (await offered(page, "Grace Hopper", "Unlock")).click();

  // then: it is no longer locked
  await expect(row(page, "Grace Hopper")).not.toContainText("Locked since");
  await expect(await offered(page, "Grace Hopper", "Lock…")).toBeVisible();
  expect(server.listed[1].locks).toEqual([]);
});

test("an operator sees how someone signs in, and turns off their authenticator app, told what follows", async ({
  page,
}) => {
  // given: an operator looking at the accounts, one of them with two-step sign-in by the app
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, second_factor: true },
  ]);
  await expect(row(page, "Grace Hopper")).toContainText("Two-step sign-in");

  // when: they open how Grace signs in
  await (await offered(page, "Grace Hopper", "Ways of signing in")).click();
  const dialog = page.getByRole("dialog", {
    name: "How Grace Hopper signs in",
  });

  // then: the password is always kept, the app and the recovery codes listed
  await expect(dialog.getByRole("listitem").nth(0)).toContainText("Password");
  await expect(dialog.getByRole("listitem").nth(0)).toContainText(
    "always kept",
  );
  await expect(dialog).toContainText("Authenticator app");
  await expect(dialog).toContainText("10 of 10 left");
  // when: they turn the app off
  await dialog.getByRole("button", { name: "Turn off the app" }).click();
  const sure = dialog.getByRole("alertdialog", {
    name: "Turning off the authenticator app",
  });
  // then: they are told what follows, and that it is recorded
  await expect(sure).toContainText(
    "Grace Hopper will sign in with the password alone until they set up a second step again.",
  );
  await expect(sure).toContainText(
    "Their account’s history shows that you did this.",
  );
  // when: they confirm
  await sure.getByRole("button", { name: "Turn off the app" }).click();
  // then: the app and the codes are gone, the password left
  await expect(sure).toHaveCount(0);
  await expect(dialog).not.toContainText("Authenticator app");
  await expect(dialog).not.toContainText("Recovery codes");
  await expect(dialog).toContainText("Password");
  expect(server.listed[1].second_factor).toBe(false);
  // when: they close the dialog
  await dialog.getByRole("button", { name: "Close" }).click();
  // then: Grace is no longer marked
  await expect(row(page, "Grace Hopper")).not.toContainText("Two-step sign-in");
});

test("an operator removes someone's lost key, the other way of the second step kept", async ({
  page,
}) => {
  // given: Grace with a passkey, a security key and the app, and the operator looking at her ways in
  const server = await anOperatorWith(page, [
    HEDY,
    {
      ...GRACE,
      second_factor: true,
      ways: {
        keys: [
          {
            key: "key_mac",
            name: "MacBook",
            role: "primary",
            since: "2026-10-02T10:00:00Z",
          },
          {
            key: "key_blue",
            name: "YubiKey blue",
            role: "second-factor",
            since: "2026-10-05T10:00:00Z",
          },
        ],
        totp_since: "2026-11-01T09:00:00Z",
        recovery_codes_left: 7,
      },
    },
  ]);
  await (await offered(page, "Grace Hopper", "Ways of signing in")).click();
  const dialog = page.getByRole("dialog", {
    name: "How Grace Hopper signs in",
  });
  await expect(dialog).toContainText("Passkey “MacBook”");
  await expect(dialog).toContainText("signs in alone");
  await expect(dialog).toContainText("Security key “YubiKey blue”");
  await expect(dialog).toContainText("7 of 10 left");

  // when: they remove the security key
  await dialog
    .getByRole("button", { name: "Remove Security key “YubiKey blue”" })
    .click();
  const sure = dialog.getByRole("alertdialog", {
    name: "Removing “YubiKey blue”",
  });
  // then: they are told the second step stays
  await expect(sure).toContainText(
    "Grace Hopper still signs in with the password and a second step.",
  );
  // when: they keep it after all
  await sure.getByRole("button", { name: "Keep it" }).click();
  // then: nothing changes
  await expect(sure).toHaveCount(0);
  expect(server.listed[1].ways!.keys).toHaveLength(2);
  // when: they remove it for real
  await dialog
    .getByRole("button", { name: "Remove Security key “YubiKey blue”" })
    .click();
  await sure.getByRole("button", { name: "Remove the key" }).click();
  // then: it is gone, the passkey, the app and the codes kept
  await expect(dialog).not.toContainText("YubiKey blue");
  await expect(dialog).toContainText("Passkey “MacBook”");
  await expect(dialog).toContainText("7 of 10 left");
  expect(server.listed[1].second_factor).toBe(true);
  // when: they remove the passkey
  await dialog
    .getByRole("button", { name: "Remove Passkey “MacBook”" })
    .click();
  const passkey = dialog.getByRole("alertdialog", {
    name: "Removing “MacBook”",
  });
  await passkey.getByRole("button", { name: "Remove the passkey" }).click();
  // then: only the password and the app's second step are left
  await expect(dialog).not.toContainText("MacBook");
  expect(server.listed[1].ways!.keys).toEqual([]);
});

test("each change to an account says the version it was made on", async ({
  page,
}) => {
  // given: an operator looking at an account at version five
  await anOperatorWith(page, [HEDY, { ...GRACE, version: 5 }]);
  const madeOn: (string | null)[] = [];
  page.on("request", (request) => {
    if (/\/lock$/.test(new URL(request.url()).pathname)) {
      madeOn.push(request.headers()["if-match"] ?? null);
    }
  });

  // when: they lock it
  await (await offered(page, "Grace Hopper", "Lock…")).click();
  const dialog = page.getByRole("dialog", { name: "Lock Grace Hopper" });
  await dialog.getByLabel("Reason").fill("Sent spam to every member.");
  await dialog.getByRole("button", { name: "Lock", exact: true }).click();
  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Locked since");
  // and: unlock it again
  await (await offered(page, "Grace Hopper", "Unlock")).click();
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
  await (await offered(page, "Grace Hopper", "Lock…")).click();
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
  await (await offered(page, "Grace Hopper", "Lock…")).click();
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
  await (await offered(page, "Grace Hopper", "Lock…")).click();
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
  await (await offered(page, "Grace Hopper", "Make operator")).click();
  const dialog = page.getByRole("dialog", {
    name: "Make Grace Hopper an operator",
  });
  await dialog.getByRole("button", { name: "Make operator" }).click();

  // then: it is an operator, no longer offered locking
  await expect(dialog).toHaveCount(0);
  await expect(row(page, "Grace Hopper")).toContainText("Operator");
  await expect(
    await offered(page, "Grace Hopper", "Take away the operator role"),
  ).toBeVisible();
  await expect(await offered(page, "Grace Hopper", "Lock…")).toHaveCount(0);
  expect(server.listed[1].operator).toBe(true);
});

test("an operator takes the role away from another", async ({ page }) => {
  // given: an operator looking at the accounts, with another operator
  const server = await anOperatorWith(page, [
    HEDY,
    { ...GRACE, operator: true },
  ]);

  // when: they take the role away from the other
  await (
    await offered(page, "Grace Hopper", "Take away the operator role")
  ).click();
  const dialog = page.getByRole("dialog", {
    name: "Take the operator role from Grace Hopper",
  });
  await dialog.getByRole("button", { name: "Take away the role" }).click();

  // then: the other is no longer an operator
  await expect(dialog).toHaveCount(0);
  await expect(
    await offered(page, "Grace Hopper", "Make operator"),
  ).toBeVisible();
  expect(server.listed[1].operator).toBe(false);
});

test("the last operator is told the role stays", async ({ page }) => {
  // given: the last operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they begin taking the role from themselves
  await (
    await offered(page, "Ada Lovelace", "Take away the operator role")
  ).click();
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
  await (
    await offered(page, "Ada Lovelace", "Take away the operator role")
  ).click();
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
  await expect(await offered(page, "Hedy Lamarr", "Make operator")).toHaveCount(
    0,
  );
  await expect(
    await offered(page, "Grace Hopper", "Make operator"),
  ).toHaveCount(0);
});

test("cancelling leaves the role as it was", async ({ page }) => {
  // given: an operator looking at the accounts
  const server = await anOperatorWith(page);

  // when: they begin making another an operator and cancel
  await (await offered(page, "Grace Hopper", "Make operator")).click();
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
