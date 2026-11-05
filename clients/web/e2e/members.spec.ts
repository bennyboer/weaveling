import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";
import { API } from "./support/shell";
import { withAccounts } from "./support/signed-in";

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
  // given: a project with an owner, an author and a reader not known by name
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

  // when: its owner opens its members
  await page.goto(`/projects/${project}/write`);
  await page.getByRole("link", { name: "Members" }).click();

  // then: they are listed by name and role, the owner marked as them
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
  // and: only that project's members were asked for
  expect(asked.length).toBeGreaterThan(0);
  expect(asked.every((one) => one === project)).toBe(true);
});

test("without accounts a project has no members to show", async ({ page }) => {
  // given: a project on a server without accounts
  test.skip(withAccounts, "the server these tests run against keeps accounts");
  const project = await aProject(page);

  // when: it is opened
  await page.goto(`/projects/${project}/write`);

  // then: no members are offered
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
  // given: an owner on the members page with an author and a reader
  const asked = await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  const people = page.locator(".members li");

  // when: the author is made an owner
  await page.getByLabel("Role of Grace Hopper").selectOption("Owner");
  // then: they are an owner, and the first may now step down
  await expect.poll(() => asked.patched).toEqual([{ account: GRACE.account, role: "owner" }]);
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("owner");
  await expect(
    page.getByLabel("Role of Ada Lovelace"),
    "with another owner, one may step down",
  ).toHaveValue("owner");

  // when: the reader is removed
  await people.filter({ hasText: "Hedy Lamarr" }).getByRole("button", { name: "Remove" }).click();
  // then: they leave the list
  await expect(people).toHaveCount(2);
  expect(asked.deleted).toEqual([HEDY.account]);
  // and: the owner is offered no removing of themselves
  await expect(people.filter({ hasText: "Ada Lovelace" }).getByRole("button", { name: "Remove" })).toHaveCount(0);
});

test("two role changes close together leave the later one standing, whichever answer comes last", async ({
  page,
}) => {
  // given: an owner on the members page with an author
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
  ]);
  // and: the answer to the next change held back
  let release = () => {};
  const held = new Promise<void>((resolve) => {
    release = resolve;
  });
  let holding = true;
  let lateAnswerTakenIn = () => {};
  const answeredLate = new Promise<void>((resolve) => {
    lateAnswerTakenIn = resolve;
  });
  await page.route(
    (url) => url.pathname.startsWith("/api/members"),
    async (route) => {
      if (route.request().method() !== "PATCH" || !holding) {
        return route.fallback();
      }
      holding = false;
      await held;
      await route.fulfill({
        json: [
          { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
          { ...GRACE, role: "reader" },
        ],
      });
      await (await route.request().response())?.finished();
      lateAnswerTakenIn();
    },
  );
  const role = page.getByLabel("Role of Grace Hopper");

  // when: the author is made a reader, then at once an owner
  await role.selectOption("Reader");
  await expect.poll(() => holding).toBe(false);
  await role.selectOption("Owner");
  await expect(role).toHaveValue("owner");
  // and: the answer to the first comes last
  release();
  await answeredLate;
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));

  // then: the later change stands
  await expect(role, "the answer to the earlier change is older than the one already shown").toHaveValue("owner");
});

test("a role change refused because another owner changed the roles meanwhile is made again on what they left", async ({
  page,
}) => {
  // given: an owner on the members page with an author and a reader, the members' version told
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  let version = 3;
  let listed: Listed[] = [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ];
  const sent: { role: string; on: string | undefined }[] = [];
  await page.route(
    (url) => url.pathname.startsWith("/api/members"),
    async (route) => {
      const request = route.request();
      const path = new URL(request.url()).pathname;
      if (request.method() === "GET" && path === "/api/members") {
        return route.fulfill({ json: listed, headers: { ETag: `"${version}"` } });
      }
      if (request.method() !== "PATCH") {
        return route.fallback();
      }
      const { role } = request.postDataJSON();
      const on = request.headers()["if-match"];
      sent.push({ role, on });
      if (sent.length === 1) {
        listed = listed.map((member) => (member.account === HEDY.account ? { ...member, role: "author" } : member));
        version += 1;
      }
      if (on !== `"${version}"`) {
        return route.fulfill({
          status: 412,
          contentType: "application/problem+json",
          body: JSON.stringify({ type: "outdated", status: 412, detail: "changed meanwhile" }),
        });
      }
      const account = path.split("/").pop();
      listed = listed.map((member) => (member.account === account ? { ...member, role } : member));
      version += 1;
      return route.fulfill({ json: listed, headers: { ETag: `"${version}"` } });
    },
  );
  await page.reload();
  await expect(page.locator(".members li")).toHaveCount(3);

  // when: the author is made a reader
  await page.getByLabel("Role of Grace Hopper").selectOption("Reader");

  // then: it is sent on the version read, refused, and sent again on the newer one
  await expect
    .poll(() => sent)
    .toEqual([
      { role: "reader", on: '"3"' },
      { role: "reader", on: '"4"' },
    ]);
  // and: both changes stand
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("reader");
  await expect(page.getByLabel("Role of Hedy Lamarr")).toHaveValue("author");
  // and: nothing is told as refused
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("an author sees the roles plainly and may only leave, which leads back to the projects", async ({
  page,
}) => {
  // given: an author on the members page
  const asked = await onTheMembersPage(page, [
    { ...GRACE, role: "owner" },
    { account: ADA.account, role: "author", name: ADA.name },
  ]);

  // then: the roles show plainly, with no removing
  await expect(page.getByRole("combobox", { name: /Role of/ })).toHaveCount(0);
  await expect(page.locator(".members li").nth(0)).toContainText("Owner");
  await expect(page.locator(".members li").nth(1)).toContainText("Author");
  await expect(page.getByRole("button", { name: "Remove" })).toHaveCount(0);
  // when: they leave
  await page.getByRole("button", { name: "Leave" }).click();

  // then: their projects show
  await expect(page).toHaveURL(/\/$/);
  expect(asked.deleted).toEqual([ADA.account]);
});

test("the only owner is shown as such, may stay, and leaves by handing the project over", async ({
  page,
}) => {
  // given: the only owner on the members page, with others
  const asked = await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  const own = page.locator(".members li").filter({ hasText: "(you)" });
  // then: they are shown as the only owner, their role fixed
  await expect(own).toContainText("Owner, the only one");
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveCount(0);

  // when: they leave and choose to stay
  await page.getByRole("button", { name: "Leave" }).click();
  const dialog = page.getByRole("dialog", { name: "You are its only owner" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Stay" }).click();
  // then: nothing changes
  await expect(dialog).toHaveCount(0);
  expect(asked.patched).toEqual([]);
  expect(asked.deleted).toEqual([]);

  // when: they leave, handing the project to another
  await page.getByRole("button", { name: "Leave" }).click();
  await dialog.getByLabel("New owner").selectOption({ label: "Hedy Lamarr (Reader)" });
  await dialog.getByRole("button", { name: "Hand over and leave" }).click();

  // then: the other owns it and they are gone
  await expect(page).toHaveURL(/\/$/);
  expect(asked.patched).toEqual([{ account: HEDY.account, role: "owner" }]);
  expect(asked.deleted).toEqual([ADA.account]);
});

test("the only member, leaving, is offered to move the project to the trash instead", async ({ page }) => {
  // given: the only member of a project
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);
  const project = new URL(page.url()).pathname.split("/")[2];

  // when: they leave
  await page.getByRole("button", { name: "Leave" }).click();
  // then: they are offered to move it to the trash
  const dialog = page.getByRole("dialog", { name: "Nobody else is in it" });
  await expect(dialog).toContainText("where it can be restored for 30 days");
  // when: they take that
  await dialog.getByRole("button", { name: "Move the project to the trash" }).click();

  // then: the project is gone
  await expect(page).toHaveURL(/\/$/);
  expect((await page.request.get(`${API}/projects/${project}`)).status()).toBe(404);
});

test("a change refused is told, and the members are asked for again", async ({ page }) => {
  // given: an owner on the members page
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
    { ...HEDY, role: "reader" },
  ]);
  // and: changes to members refused
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

  // when: a role is changed
  await page.getByLabel("Role of Grace Hopper").selectOption("Reader");
  // then: the refusal is told and the role is as it was
  await expect(page.getByRole("alert")).toHaveText(told);
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("author");

  // when: a member is removed
  await page.locator(".members li").filter({ hasText: "Hedy Lamarr" }).getByRole("button", { name: "Remove" }).click();
  // then: the refusal is told and the member stays
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
  // given: an owner on the members page, with a lapsed invitation
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

  // when: the invited are opened
  await page.getByRole("link", { name: "Invited" }).click();
  // then: the project's invitations are listed with their roles
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/members/invited$`));
  const invitations = page.locator(".invitations li");
  await expect(invitations).toHaveCount(1);
  await expect(invitations.nth(0)).toContainText("lin@example.com");
  await expect(invitations.nth(0)).toContainText("Reader");
  expect(asked.listed).toEqual([project]);

  // when: an address is invited as an owner
  await page.getByLabel("Email address").fill("ken@example.com");
  await page.getByLabel("As").selectOption("Owner");
  await page.getByRole("button", { name: "Invite", exact: true }).click();

  // then: it is listed first, as an owner
  await expect(invitations).toHaveCount(2);
  await expect(invitations.nth(0)).toContainText("ken@example.com");
  await expect(invitations.nth(0)).toContainText("Owner");
  // when: the lapsed invitation is invited again
  await page.getByRole("button", { name: "Invite again" }).click();
  // then: it is sent again as it was
  await expect(invitations).toHaveCount(3);
  expect(asked.sent).toEqual([
    { email: "ken@example.com", scope: { kind: "project", project, role: "owner" } },
    { email: "lin@example.com", scope: { kind: "project", project: "", role: "reader" } },
  ]);
});

test("whoever is no owner finds no Invited tab", async ({ page }) => {
  // given: an author on the members page
  await onTheMembersPage(page, [
    { ...GRACE, role: "owner" },
    { account: ADA.account, role: "author", name: ADA.name },
  ]);

  // then: no invited tab is offered
  await expect(page.getByRole("link", { name: "People" })).toBeVisible();
  await expect(page.getByRole("link", { name: "Invited" })).toHaveCount(0);
});

test("a hand-over half done is told, and the members are shown as they now are", async ({
  page,
}) => {
  // given: the only owner on the members page, with an author
  await onTheMembersPage(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { ...GRACE, role: "author" },
  ]);
  // and: their leaving refused after the hand-over
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

  // when: they hand the project over and leave
  await page.getByRole("button", { name: "Leave" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Hand over and leave" }).click();

  // then: the refusal is told
  await expect(page.getByRole("alert")).toHaveText("This person is not a member of this project.");
  // and: the members show as they now are
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
  await expect(page.getByLabel("Role of Grace Hopper")).toHaveValue("owner");
  await expect(page.getByLabel("Role of Ada Lovelace")).toHaveValue("owner");
});

test("the masthead shows a project's members as a stack of initials, three and how many more", async ({
  page,
}) => {
  // given: a project of four members, one not known by name
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

  // when: it is opened
  await page.goto(`/projects/${project}/write`);

  // then: the masthead shows three initials and how many more
  const members = page.getByRole("link", {
    name: "Members: Ada Lovelace, Grace Hopper, Not known by name yet, Hedy Lamarr",
  });
  await expect(members).toBeVisible();
  await expect(members.locator(".avatar:visible")).toHaveText(["A", "G", "?", "+1"]);
  // when: they are clicked
  await members.click();
  // then: the members page opens
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
});

test("one's own initial wears the same tint in the stack as on the account's button", async ({
  page,
}) => {
  // given: a project of mine alone
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);

  // when: it is opened
  await page.goto(`/projects/${project}/write`);

  // then: my initial wears the same tint as my account's button
  const mine = page.locator(".masthead .stack .avatar").first();
  await expect(mine).toHaveText("A");
  const tinted = (locator: import("@playwright/test").Locator) =>
    locator.evaluate((element) => getComputedStyle(element).backgroundColor);
  expect(await tinted(mine)).toBe(
    await tinted(page.getByRole("button", { name: "Account: Ada Lovelace" })),
  );
});

test("on a phone the members are in the More menu, not beside the project's name", async ({
  page,
}) => {
  // given: a phone
  await page.setViewportSize({ width: 360, height: 700 });
  // and: a project with two members
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  const project = await aProject(page);
  await membersServed(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper" },
  ]);

  // when: it is opened
  await page.goto(`/projects/${project}/write`);
  await expect(page.getByRole("button", { name: "More" })).toBeVisible();

  // then: no members or sharing show beside its name
  await expect(page.locator(".masthead .stack")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Share", exact: true })).toHaveCount(0);
  // when: the more menu is opened and members chosen
  await page.getByRole("button", { name: "More" }).click();
  const menu = page.getByRole("dialog", { name: "More" });
  await expect(menu.getByRole("heading", { name: "This project" })).toBeVisible();
  await menu.getByRole("link", { name: "Members" }).click();
  // then: the members page opens and the menu closes
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/members$`));
  await expect(menu).toHaveCount(0);
});

async function aProjectToShare(page: Page, listed: Listed[]) {
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
  // given: an owner of a project with an author and an invitation
  const { server } = await aProjectToShare(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper", email: "grace@example.com" },
  ]);

  // when: share is chosen
  await page.getByRole("button", { name: "Share", exact: true }).click();

  // then: it shows who has access and who is invited
  const dialog = shareDialog(page);
  await expect(dialog.getByRole("heading", { name: "People with access" })).toBeVisible();
  await expect(dialog.locator(".members li")).toHaveCount(2);
  await expect(dialog.locator(".members li").nth(1)).toContainText("Grace Hopper");
  await expect(dialog.getByRole("heading", { name: "Invited" })).toBeVisible();
  await expect(dialog.locator(".invitations")).toContainText("ken@example.com");

  // when: an address is invited as a reader
  await dialog.getByLabel("Email address").fill("mary@example.com");
  await dialog.getByLabel("As").selectOption("reader");
  await dialog.getByRole("button", { name: "Invite", exact: true }).click();

  // then: it is listed invited
  await expect(dialog.locator(".invitations")).toContainText("mary@example.com");
  expect(server.invited[0].email).toBe("mary@example.com");
  // when: escape is pressed
  await page.keyboard.press("Escape");
  // then: the dialog closes
  await expect(shareDialog(page)).toHaveCount(0);
});

test("someone who does not own the project sees who has access, and asks for no invitations", async ({
  page,
}) => {
  // given: an author of a project
  const { invitationsAsked } = await aProjectToShare(page, [
    { account: "account_2", role: "owner", name: "Grace Hopper" },
    { account: ADA.account, role: "author", name: ADA.name, email: ADA.email },
  ]);

  // when: share is chosen
  await page.getByRole("button", { name: "Share", exact: true }).click();

  // then: it shows who has access
  const dialog = shareDialog(page);
  await expect(dialog.locator(".members li")).toHaveCount(2);
  // and: offers no inviting and asks for no invitations
  await expect(dialog.getByLabel("Email address")).toHaveCount(0);
  await expect(dialog.getByRole("heading", { name: "Invited" })).toHaveCount(0);
  expect(invitationsAsked).toEqual([]);
});

test("from the Share dialog the Members page is one step away", async ({ page }) => {
  // given: the share dialog open
  const { project } = await aProjectToShare(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
  ]);
  await page.getByRole("button", { name: "Share", exact: true }).click();

  // when: all members and invitations is followed
  await shareDialog(page).getByRole("link", { name: "All members and invitations" }).click();

  // then: the members page opens
  await expect(page).toHaveURL(new RegExp(`/projects/${project}/members$`));
  await expect(page.getByRole("heading", { name: "Members" })).toBeVisible();
});

test("without accounts there is nothing to share", async ({ page }) => {
  // given: a project on a server without accounts
  test.skip(withAccounts, "the server these tests run against keeps accounts");
  const project = await aProject(page);

  // when: it is opened
  await page.goto(`/projects/${project}/write`);

  // then: nothing is offered to share
  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("button", { name: "Share", exact: true })).toHaveCount(0);
});

test("on a phone, Share is in the More menu and the dialog fits the screen", async ({ page }) => {
  // given: a phone
  await page.setViewportSize({ width: 360, height: 700 });
  // and: an owner of a project with an author
  await aProjectToShare(page, [
    { account: ADA.account, role: "owner", name: ADA.name, email: ADA.email },
    { account: "account_2", role: "author", name: "Grace Hopper", email: "grace@example.com" },
  ]);

  // when: share is chosen from the more menu
  await page.getByRole("button", { name: "More" }).click();
  await page.getByRole("dialog", { name: "More" }).getByRole("button", { name: "Share" }).click();

  // then: the menu closes and the dialog fits the screen
  await expect(page.getByRole("dialog", { name: "More" })).toHaveCount(0);
  await expect(shareDialog(page).getByRole("heading", { name: "People with access" })).toBeVisible();
  const shown = await shareDialog(page).boundingBox();
  expect(shown).not.toBeNull();
  expect(shown!.x).toBeGreaterThanOrEqual(0);
  expect(shown!.x + shown!.width).toBeLessThanOrEqual(360);
});
