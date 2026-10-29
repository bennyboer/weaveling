import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";
import { API } from "./support/shell";

async function aProject(page: import("@playwright/test").Page): Promise<string> {
  const made = await page.request.post(`${API}/projects`, {
    data: { name: `The Loom ${Date.now()}`, language: "en" },
  });
  expect(made.ok()).toBe(true);

  return (await made.json()).id;
}

test("a project's members are listed by name and role, with whoever is me marked", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  const asked: string[] = [];
  await page.route(
    (url) => url.pathname === "/api/members",
    (route) => {
      asked.push(new URL(route.request().url()).searchParams.get("project") ?? "");
      return route.fulfill({
        json: [
          { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
          {
            account: "account_2",
            role: "author",
            name: "Grace Hopper",
            email: "grace@example.com",
          },
          { account: "account_3", role: "reader" },
        ],
      });
    },
  );

  await page.goto(`/projects/${project}/write`);
  await page.getByRole("link", { name: "Members" }).click();

  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
  const people = page.locator(".members li");
  await expect(people).toHaveCount(3);
  await expect(people.nth(0)).toContainText("Ada Lovelace(you)");
  await expect(people.nth(0)).toContainText(ADA.email);
  await expect(people.nth(0)).toContainText("Owner, the only one");
  await expect(people.nth(1)).toContainText("Grace Hopper");
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("author");
  await expect(people.nth(1)).not.toContainText("(you)");
  await expect(people.nth(2)).toContainText("Not known by name yet");
  await expect(page.getByLabel("Role of Not known by name yet")).toHaveValue("reader");
  expect(asked.length).toBeGreaterThan(0);
  expect(asked.every((one) => one === project)).toBe(true);
});

test("without accounts a project has no members to show", async ({ page }) => {
  const project = await aProject(page);

  await page.goto(`/projects/${project}/write`);

  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("link", { name: "Members" })).toHaveCount(0);
});

type Listed = { account: string; role: string; name?: string; email?: string };

async function membersServed(page: Page, listed: Listed[]) {
  const asked = { patched: [] as { account: string; role: string }[], deleted: [] as string[] };
  await page.route(
    (url) => url.pathname.startsWith("/api/members"),
    async (route) => {
      const request = route.request();
      const account = new URL(request.url()).pathname.split("/").pop() ?? "";
      const theLastOwner = () =>
        listed.filter((member) => member.role === "owner").length === 1 &&
        listed.some((member) => member.account === account && member.role === "owner");
      const lastOwnerRefused = () =>
        route.fulfill({
          status: 409,
          contentType: "application/problem+json",
          body: JSON.stringify({ type: "last-owner", status: 409, detail: "last-owner" }),
        });
      if (request.method() === "PATCH") {
        const { role } = request.postDataJSON();
        asked.patched.push({ account, role });
        if (role !== "owner" && theLastOwner()) {
          return lastOwnerRefused();
        }
        listed = listed.map((member) => (member.account === account ? { ...member, role } : member));
        return route.fulfill({ json: listed });
      }
      if (request.method() === "DELETE") {
        asked.deleted.push(account);
        if (theLastOwner()) {
          return lastOwnerRefused();
        }
        listed = listed.filter((member) => member.account !== account);
        return route.fulfill({ status: 204 });
      }
      return route.fulfill({ json: listed });
    },
  );

  return asked;
}

async function onTheMembersPage(page: Page, listed: Listed[]) {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  const asked = await membersServed(page, listed);
  await page.goto(`/projects/${project}/members`);
  await expect(page.locator(".members li")).toHaveCount(listed.length);

  return asked;
}

const GRACE = { account: "account_2", name: "Grace Hopper", email: "grace@example.com" };
const HEDY = { account: "account_3", name: "Hedy Lamarr", email: "hedy@example.com" };

test("an owner changes a member's role and removes another, and the list follows", async ({
  page,
}) => {
  const asked = await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  const people = page.locator(".members li");

  await page.getByLabel("Role of Grace Hopper").selectOption("Owner");
  await expect.poll(() => asked.patched).toEqual([{ account: GRACE.account, role: "owner" }]);
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("owner");
  await expect(
    page.getByLabel("Role of Ada Lovelace"),
    "with another owner, one may step down",
  ).toHaveValue("owner");

  await people.filter({ hasText: "Hedy Lamarr" }).getByRole("button", { name: "Remove" }).click();
  await expect(people).toHaveCount(2);
  expect(asked.deleted).toEqual([HEDY.account]);
  await expect(people.filter({ hasText: "Ada Lovelace" }).getByRole("button", { name: "Remove" })).toHaveCount(0);
});

test("an author sees the roles plainly and may only leave, which leads back to the projects", async ({
  page,
}) => {
  const asked = await onTheMembersPage(page, [
    { ...GRACE, role: "owner" },
    { account: ADA.account, role: "author", name: ADA.name },
  ]);

  await expect(page.getByRole("combobox", { name: /Role of/ })).toHaveCount(0);
  await expect(page.locator(".members li").nth(0)).toContainText("Owner");
  await expect(page.locator(".members li").nth(1)).toContainText("Author");
  await expect(page.getByRole("button", { name: "Remove" })).toHaveCount(0);
  await page.getByRole("button", { name: "Leave" }).click();

  await expect(page).toHaveURL(/\/$/);
  expect(asked.deleted).toEqual([ADA.account]);
});

test("the only owner is shown as such, may stay, and leaves by handing the project over", async ({
  page,
}) => {
  const asked = await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  const own = page.locator(".members li").filter({ hasText: "(you)" });
  await expect(own).toContainText("Owner, the only one");
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveCount(0);

  await page.getByRole("button", { name: "Leave" }).click();
  const dialog = page.getByRole("dialog", { name: "You are its only owner" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Stay" }).click();
  await expect(dialog).toHaveCount(0);
  expect(asked.patched).toEqual([]);
  expect(asked.deleted).toEqual([]);

  await page.getByRole("button", { name: "Leave" }).click();
  await dialog.getByLabel("New owner").selectOption({ label: "Hedy Lamarr (Reader)" });
  await dialog.getByRole("button", { name: "Hand over and leave" }).click();

  await expect(page).toHaveURL(/\/$/);
  expect(asked.patched).toEqual([{ account: HEDY.account, role: "owner" }]);
  expect(asked.deleted).toEqual([ADA.account]);
});

test("the only member, leaving, is offered to delete the project instead", async ({ page }) => {
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);
  const project = new URL(page.url()).pathname.split("/")[2];

  await page.getByRole("button", { name: "Leave" }).click();
  const dialog = page.getByRole("dialog", { name: "Nobody else is in it" });
  await expect(dialog).toContainText("Deleting cannot be undone.");
  await dialog.getByRole("button", { name: "Delete the project" }).click();

  await expect(page).toHaveURL(/\/$/);
  expect((await page.request.get(`${API}/projects/${project}`)).status()).toBe(404);
});

test("a change refused is told, and the members are asked for again", async ({ page }) => {
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  await page.route(
    (url) => url.pathname.startsWith("/api/members/"),
    (route) =>
      route.fulfill({
        status: 403,
        contentType: "application/problem+json",
        body: JSON.stringify({ type: "not-an-owner", status: 403, detail: "not-an-owner" }),
      }),
  );
  const told = "Only an owner of this project may do this.";

  await page.getByLabel("Role of Grace Hopper").selectOption("Reader");
  await expect(page.getByRole("alert")).toHaveText(told);
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("author");

  await page.locator(".members li").filter({ hasText: "Hedy Lamarr" }).getByRole("button", { name: "Remove" }).click();
  await expect(page.getByRole("alert")).toHaveText(told);
  await expect(page.locator(".members li")).toHaveCount(3);
});

type ProjectInvitation = {
  id: string;
  email: string;
  issued_at: string;
  until: string;
  status: string;
  scope: { kind: "project"; project: string; role: string };
};

async function projectInvitationsServed(page: Page, held: ProjectInvitation[]) {
  const asked = { listed: [] as string[], sent: [] as unknown[] };
  await page.route(
    (url) => url.pathname === "/api/invitations",
    async (route) => {
      const request = route.request();
      if (request.method() === "POST") {
        const sent = request.postDataJSON();
        asked.sent.push(sent);
        const issued = new Date();
        const invitation: ProjectInvitation = {
          id: `invitation_${100 + held.length}`,
          email: sent.email,
          issued_at: issued.toISOString(),
          until: new Date(issued.getTime() + 7 * 86_400_000).toISOString(),
          status: "open",
          scope: sent.scope,
        };
        held.unshift(invitation);
        return route.fulfill({ status: 201, json: invitation });
      }
      asked.listed.push(new URL(request.url()).searchParams.get("project") ?? "");
      return route.fulfill({ json: { invitations: held, next: null } });
    },
  );

  return asked;
}

test("an owner invites into the project as a role and invites a lapsed invitation again as it was", async ({
  page,
}) => {
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);
  const project = new URL(page.url()).pathname.split("/")[2];
  const asked = await projectInvitationsServed(page, [
    {
      id: "invitation_1",
      email: "lin@example.com",
      issued_at: "2026-10-13T08:00:00Z",
      until: "2026-10-20T08:00:00Z",
      status: "expired",
      scope: { kind: "project", project: "", role: "reader" },
    },
  ]);

  await page.getByRole("link", { name: "Invited" }).click();
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/members/invited$`));
  const invitations = page.locator(".invitations li");
  await expect(invitations).toHaveCount(1);
  await expect(invitations.nth(0)).toContainText("lin@example.com");
  await expect(invitations.nth(0)).toContainText("Reader");
  expect(asked.listed).toEqual([project]);

  await page.getByLabel("Email address").fill("ken@example.com");
  await page.getByLabel("As").selectOption("Owner");
  await page.getByRole("button", { name: "Invite", exact: true }).click();

  await expect(invitations).toHaveCount(2);
  await expect(invitations.nth(0)).toContainText("ken@example.com");
  await expect(invitations.nth(0)).toContainText("Owner");
  await page.getByRole("button", { name: "Invite again" }).click();
  await expect(invitations).toHaveCount(3);
  expect(asked.sent).toEqual([
    { email: "ken@example.com", scope: { kind: "project", project, role: "owner" } },
    { email: "lin@example.com", scope: { kind: "project", project: "", role: "reader" } },
  ]);
});

test("whoever is no owner finds no Invited tab", async ({ page }) => {
  await onTheMembersPage(page, [
    { ...GRACE, role: "owner" },
    { account: ADA.account, role: "author", name: ADA.name },
  ]);

  await expect(page.getByRole("link", { name: "People" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Invited" })).toHaveCount(0);
});

test("a hand-over half done is told, and the members are shown as they now are", async ({
  page,
}) => {
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
  ]);
  await page.route(
    (url) => url.pathname.startsWith("/api/members/"),
    (route) =>
      route.request().method() === "DELETE"
        ? route.fulfill({
            status: 403,
            contentType: "application/problem+json",
            body: JSON.stringify({ type: "not-a-member", status: 403, detail: "not-a-member" }),
          })
        : route.fallback(),
  );

  await page.getByRole("button", { name: "Leave" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Hand over and leave" }).click();

  await expect(page.getByRole("alert")).toHaveText("This person is not a member of this project.");
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("owner");
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveValue("owner");
});

test("the masthead shows a project's members as a stack of initials, three and how many more", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper" },
    { account: "account_3", role: "reader" },
    { account: "account_4", role: "reader", name: "Hedy Lamarr" },
  ]);

  await page.goto(`/projects/${project}/write`);

  const members = page.getByRole("link", {
    name: "Members: Ada Lovelace, Grace Hopper, Not known by name yet, Hedy Lamarr",
  });
  await expect(members).toBeVisible();
  await expect(members.locator(".avatar:visible")).toHaveText(["A", "G", "?", "+1"]);
  await members.click();
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
});

test("one's own initial wears the same tint in the stack as on the account's button", async ({
  page,
}) => {
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);

  await page.goto(`/projects/${project}/write`);

  const mine = page.locator(".masthead .stack .avatar").first();
  await expect(mine).toHaveText("A");
  const tinted = (locator: import("@playwright/test").Locator) =>
    locator.evaluate((element) => getComputedStyle(element).backgroundColor);
  expect(await tinted(mine)).toBe(
    await tinted(page.getByRole("button", { name: "Account: Ada Lovelace" })),
  );
});

test("on a phone the stack shows one's first member and how many more, beside the masthead's buttons", async ({
  page,
}) => {
  await page.setViewportSize({ width: 360, height: 700 });
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper" },
    { account: "account_3", role: "reader" },
    { account: "account_4", role: "reader", name: "Hedy Lamarr" },
  ]);

  await page.goto(`/projects/${project}/write`);

  const stack = page.locator(".masthead .stack");
  await expect(stack.locator(".avatar:visible")).toHaveText(["A", "+3"]);
  const shown = await stack.boundingBox();
  const next = await page.locator(".masthead .bell-button").boundingBox();
  expect(shown && next).toBeTruthy();
  expect(shown!.x + shown!.width).toBeLessThanOrEqual(next!.x);
});

async function sharing(page: Page, listed: Listed[]) {
  const server = await aServerWithAccounts(page);
  server.operator = true;
  server.invited = [
    {
      id: "invitation_1",
      email: "ken@example.com",
      issued_at: "2026-10-28T08:00:00Z",
      until: "2099-11-04T08:00:00Z",
      status: "open",
    },
  ];
  const invitationsAsked: string[] = [];
  page.on("request", (request) => {
    if (new URL(request.url()).pathname === "/api/invitations") {
      invitationsAsked.push(request.method());
    }
  });
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, listed);
  await page.goto(`/projects/${project}/write`);

  return { server, invitationsAsked, project };
}

const shareDialog = (page: Page) => page.getByRole("dialog", { name: /^Share “/ });

test("Share opens a dialog to invite, with who has access and who is invited", async ({
  page,
}) => {
  const { server } = await sharing(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper", email: "grace@example.com" },
  ]);

  await page.getByRole("button", { name: "Share", exact: true }).click();

  const dialog = shareDialog(page);
  await expect(dialog.getByRole("heading", { name: "People with access" })).toBeVisible();
  await expect(dialog.locator(".members li")).toHaveCount(2);
  await expect(dialog.locator(".members li").nth(1)).toContainText("Grace Hopper");
  await expect(dialog.getByRole("heading", { name: "Invited" })).toBeVisible();
  await expect(dialog.locator(".invitations")).toContainText("ken@example.com");

  await dialog.getByLabel("Email address").fill("mary@example.com");
  await dialog.getByLabel("As").selectOption("reader");
  await dialog.getByRole("button", { name: "Invite", exact: true }).click();

  await expect(dialog.locator(".invitations")).toContainText("mary@example.com");
  expect(server.invited[0].email).toBe("mary@example.com");
  await page.keyboard.press("Escape");
  await expect(shareDialog(page)).toHaveCount(0);
});

test("someone who does not own the project sees who has access, and asks for no invitations", async ({
  page,
}) => {
  const { invitationsAsked } = await sharing(page, [
    { account: "account_2", role: "owner", name: "Grace Hopper" },
    { account: ADA.account, role: "author", name: ADA.name, email: ADA.email },
  ]);

  await page.getByRole("button", { name: "Share", exact: true }).click();

  const dialog = shareDialog(page);
  await expect(dialog.locator(".members li")).toHaveCount(2);
  await expect(dialog.getByLabel("Email address")).toHaveCount(0);
  await expect(dialog.getByRole("heading", { name: "Invited" })).toHaveCount(0);
  expect(invitationsAsked).toEqual([]);
});

test("from the Share dialog the Members page is one step away", async ({ page }) => {
  const { project } = await sharing(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);
  await page.getByRole("button", { name: "Share", exact: true }).click();

  await shareDialog(page).getByRole("link", { name: "All members and invitations" }).click();

  await expect(page).toHaveURL(new RegExp(`/projects/${project}/members$`));
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
});

test("without accounts there is nothing to share", async ({ page }) => {
  const project = await aProject(page);

  await page.goto(`/projects/${project}/write`);

  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("button", { name: "Share", exact: true })).toHaveCount(0);
});

test("on a phone, Share is its icon alone and the dialog fits the screen", async ({ page }) => {
  await page.setViewportSize({ width: 360, height: 700 });
  await sharing(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper", email: "grace@example.com" },
  ]);

  const share = page.getByRole("button", { name: "Share", exact: true });
  await expect(share).toBeVisible();
  const button = await share.boundingBox();
  const bell = await page.locator(".masthead .bell-button").boundingBox();
  expect(button && bell).toBeTruthy();
  expect(button!.width).toBeLessThan(40);
  expect(button!.x + button!.width).toBeLessThanOrEqual(bell!.x);

  await share.click();
  const shown = await shareDialog(page).boundingBox();
  expect(shown).not.toBeNull();
  expect(shown!.x).toBeGreaterThanOrEqual(0);
  expect(shown!.x + shown!.width).toBeLessThanOrEqual(360);
});
