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
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveValue("owner");
  await expect(people.nth(1)).toContainText("Grace Hopper");
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("author");
  await expect(people.nth(1)).not.toContainText("(you)");
  await expect(people.nth(2)).toContainText("Not known by name yet");
  await expect(page.getByLabel("Role of Not known by name yet")).toHaveValue("reader");
  expect(asked).toEqual([project]);
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

test("the last owner is told why they may neither step down nor leave, and stays as they were", async ({
  page,
}) => {
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
  ]);
  const told = "A project keeps at least one owner. Make someone else an owner first.";

  await page.getByLabel("Role of Ada Lovelace").selectOption("Author");
  await expect(page.getByRole("alert")).toHaveText(told);
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveValue("owner");

  await page.getByRole("button", { name: "Leave" }).click();
  await expect(page.getByRole("alert")).toHaveText(told);
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
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
