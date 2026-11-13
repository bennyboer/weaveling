import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  aServerWithAccounts,
  signIn,
  type Invited,
  type Server,
} from "./support/accounts";

const OPEN: Invited = {
  id: "invitation_3",
  email: "grace@example.com",
  issued_at: "2026-10-23T08:00:00Z",
  until: "2026-10-30T08:00:00Z",
  status: "open",
};

const DECLINED: Invited = {
  id: "invitation_2",
  email: "ada@example.com",
  issued_at: "2026-10-22T08:00:00Z",
  until: "2026-10-29T08:00:00Z",
  status: "declined",
  declined_for: "address-has-account",
};

const TURNED_DOWN: Invited = {
  id: "invitation_4",
  email: "hopper@example.com",
  issued_at: "2026-10-21T08:00:00Z",
  until: "2026-10-28T08:00:00Z",
  status: "declined",
  declined_for: "turned-down",
};

const EXPIRED: Invited = {
  id: "invitation_1",
  email: "hedy@example.com",
  issued_at: "2026-10-10T08:00:00Z",
  until: "2026-10-17T08:00:00Z",
  status: "expired",
};

async function anOperatorInviting(
  page: Page,
  invited: Invited[],
  pageSize = 100,
): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.operator = true;
  server.invited = invited;
  server.pageSize = pageSize;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page
      .getByRole("banner")
      .getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
  await page.goto("/instance/invitations");

  return server;
}

const row = (page: Page, email: string) =>
  page.getByRole("listitem").filter({ hasText: email });

test("the operator sees whom they invited, when, and how each stands", async ({
  page,
}) => {
  // given: an operator looking at invitations open, declined, turned down and run out
  await anOperatorInviting(page, [OPEN, DECLINED, TURNED_DOWN, EXPIRED]);

  // then: each is listed with when it was sent and how it stands
  await expect(
    page
      .getByRole("navigation", { name: "Sections" })
      .getByRole("link", { name: "Invitations" }),
  ).toHaveAttribute("aria-current", "page");
  await expect(row(page, "grace@example.com")).toContainText(
    "Invited on 23 Oct 2026",
  );
  await expect(row(page, "grace@example.com")).toContainText(
    "Open until 30 Oct 2026",
  );
  await expect(row(page, "hedy@example.com")).toContainText(
    "Expired on 17 Oct 2026",
  );
  await expect(row(page, "ada@example.com")).toContainText(
    "Not needed: this email address already has an account",
  );
  await expect(row(page, "hopper@example.com")).toContainText("Declined");
  await expect(page.getByText("Nobody is invited just now.")).toHaveCount(0);
});

test("with nobody invited the operator is told so", async ({ page }) => {
  // given: an operator who invited nobody
  await anOperatorInviting(page, []);

  // then: they are told so
  await expect(page.getByText("Nobody is invited just now.")).toBeVisible();
});

test("a long list of invitations comes a page at a time", async ({ page }) => {
  // given: an operator with more invitations than fit a page
  await anOperatorInviting(page, [OPEN, DECLINED, EXPIRED], 2);
  const more = page.getByRole("button", { name: "Show more invitations" });

  // then: the first page is listed
  await expect(row(page, "grace@example.com")).toBeVisible();
  await expect(row(page, "hedy@example.com")).toHaveCount(0);
  // when: more is asked for
  await more.click();

  // then: the rest follows and no more is offered
  await expect(row(page, "hedy@example.com")).toBeVisible();
  await expect(more).toHaveCount(0);
});

test("accounts and invitations are each a page of their own, kept on reload", async ({
  page,
}) => {
  // given: an operator with an invitation
  const server = await anOperatorInviting(page, [OPEN]);
  server.listed = [];
  const sections = page.getByRole("navigation", { name: "Sections" });

  // when: the instance is opened
  await page.goto("/instance");
  // then: the accounts show on a page of their own
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  await expect(row(page, "Ada Lovelace")).toBeVisible();
  await expect(row(page, "grace@example.com")).toHaveCount(0);

  // when: the invitations are chosen
  await sections.getByRole("link", { name: "Invitations" }).click();
  // then: they show on a page of their own
  await expect(page).toHaveURL(/\/instance\/invitations$/);
  await expect(row(page, "grace@example.com")).toBeVisible();
  await expect(row(page, "Ada Lovelace")).toHaveCount(0);

  // when: the page is reloaded
  await page.reload();
  // then: the invitations still show
  await expect(row(page, "grace@example.com")).toBeVisible();

  // when: the browser goes back
  await page.goBack();
  // then: the accounts show again
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  await expect(row(page, "Ada Lovelace")).toBeVisible();
});

test("an operator invites an address and sees it on the list at once", async ({
  page,
}) => {
  // given: an operator who invited nobody
  const server = await anOperatorInviting(page, []);
  const address = page.getByLabel("Email address");

  // when: they invite an address
  await address.fill("Grace@Example.com");
  await page.getByRole("button", { name: "Invite" }).click();

  // then: it is listed open at once, in lower case
  await expect(row(page, "grace@example.com")).toContainText("Open until");
  await expect(address).toHaveValue("");
  await expect(page.getByText("Nobody is invited just now.")).toHaveCount(0);
  expect(server.invited.map((invitation) => invitation.email)).toEqual([
    "grace@example.com",
  ]);
});

test("an address invited too often is told how long to wait", async ({
  page,
}) => {
  // given: an operator who must wait before inviting again
  const server = await anOperatorInviting(page, []);
  server.invitingWaits = 3;

  // when: they invite an address
  await page.getByLabel("Email address").fill("grace@example.com");
  await page.getByRole("button", { name: "Invite" }).click();

  // then: they are told how long to wait
  await expect(page.getByRole("alert")).toContainText(
    "Please wait 3 minutes before trying again.",
  );
  // and: the address is kept, nothing listed
  await expect(page.getByLabel("Email address")).toHaveValue(
    "grace@example.com",
  );
  await expect(row(page, "grace@example.com")).toHaveCount(0);
});

test("an operator withdraws an open invitation, one run out or declined offers nothing to withdraw", async ({
  page,
}) => {
  // given: an operator looking at invitations open, declined and run out
  const server = await anOperatorInviting(page, [OPEN, DECLINED, EXPIRED]);

  // then: the declined one offers nothing to withdraw
  await expect(row(page, "ada@example.com")).toContainText(
    "already has an account",
  );
  await expect(
    row(page, "ada@example.com").getByRole("button", { name: "Withdraw" }),
  ).toHaveCount(0);
  // when: the open one is withdrawn
  await row(page, "grace@example.com")
    .getByRole("button", { name: "Withdraw" })
    .click();

  // then: it leaves the list
  await expect(row(page, "grace@example.com")).toHaveCount(0);
  expect(server.invited.map((invitation) => invitation.id)).toEqual([
    DECLINED.id,
    EXPIRED.id,
  ]);
  // and: the one run out offers inviting again, not withdrawing
  await expect(row(page, "hedy@example.com")).toContainText("Expired on");
  await expect(
    row(page, "hedy@example.com").getByRole("button", { name: "Withdraw" }),
  ).toHaveCount(0);
  await expect(
    row(page, "hedy@example.com").getByRole("button", { name: "Invite again" }),
  ).toBeVisible();
});

for (const width of [1280, 320]) {
  test(`the address to invite is one line, its button beside or below it at ${width}px`, async ({
    page,
  }) => {
    // given: a window of the given width
    await page.setViewportSize({ width, height: 800 });
    // and: an operator looking at the invitations
    await anOperatorInviting(page, []);

    // then: the address is one line, its button beside it on a wide window and below on a phone
    const field = await page.getByLabel("Email address").boundingBox();
    const button = await page
      .getByRole("button", { name: "Invite" })
      .boundingBox();
    const sections = await page
      .getByRole("navigation", { name: "Sections" })
      .boundingBox();
    expect(field && button && sections).toBeTruthy();

    expect(field!.height, "an address is a line, not a box").toBeLessThan(
      button!.height * 1.5,
    );
    expect(Math.round(field!.x), "the field begins where the page does").toBe(
      Math.round(sections!.x),
    );
    if (width >= 800) {
      expect(
        Math.round(field!.y + field!.height),
        "the button sits beside the field",
      ).toBe(Math.round(button!.y + button!.height));
    } else {
      expect(button!.y, "on a phone the button goes below").toBeGreaterThan(
        field!.y + field!.height,
      );
    }
  });
}

test("an invitation run out is invited again, the new one listed open above it", async ({
  page,
}) => {
  // given: an operator looking at an open invitation and one run out
  const server = await anOperatorInviting(page, [OPEN, EXPIRED]);
  // then: the open one offers no inviting again
  await expect(row(page, "grace@example.com")).toContainText("Open until");
  await expect(
    row(page, "grace@example.com").getByRole("button", {
      name: "Invite again",
    }),
  ).toHaveCount(0);

  // when: the one run out is invited again
  await row(page, "hedy@example.com")
    .getByRole("button", { name: "Invite again" })
    .click();

  // then: a new open one is listed above it
  await expect(row(page, "hedy@example.com")).toHaveCount(2);
  await expect(row(page, "hedy@example.com").first()).toContainText(
    "Open until",
  );
  await expect(row(page, "hedy@example.com").last()).toContainText(
    "Expired on",
  );
  expect(server.invited[0]).toMatchObject({
    email: "hedy@example.com",
    status: "open",
  });
});
