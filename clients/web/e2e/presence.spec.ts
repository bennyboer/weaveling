import { test, expect, request, type Browser, type BrowserContext, type Page } from "@playwright/test";

import { aBookInTheWritingView, flow, projectOf, scene } from "./support/writing";
import { API } from "./support/shell";
import { lastCodeDropped, withAccounts } from "./support/signed-in";

test.skip(!withAccounts, "who is writing where is only told between accounts");

const PASSWORD = "a password only grace knows";

/// Signs up a second account, confirms its address and signs it in, in a browser of its own.
async function anotherAccount(browser: Browser, named: string): Promise<{ email: string; context: BrowserContext }> {
  const email = `${named.toLowerCase()}-${crypto.randomUUID().slice(0, 8)}@example.com`;
  const server = await request.newContext({ baseURL: `${API}/` });
  expect((await server.post("accounts", { data: { email, name: named, password: PASSWORD } })).status()).toBe(202);
  const code = lastCodeDropped(process.env.WEAVELING_E2E_MAIL_DROP!, email);
  expect((await server.post("accounts/verification", { data: { email, code } })).status()).toBe(204);
  expect((await server.post("sessions", { data: { email, password: PASSWORD } })).ok()).toBe(true);
  const state = await server.storageState();
  await server.dispose();
  const cookies = state.cookies.map((cookie) => ({ ...cookie, secure: true }));

  return {
    email,
    context: await browser.newContext({
      storageState: {
        ...state,
        cookies: [...cookies, ...cookies.map((cookie) => ({ ...cookie, domain: "localhost" }))],
      },
    }),
  };
}

/// Has the owner invite the account into the project as an author, and the account take the invitation up.
async function madeAnAuthor(owner: Page, author: BrowserContext, email: string, project: string) {
  const invited = await owner.request.post(`${API}/invitations`, {
    data: { email, scope: { kind: "project", project, role: "author" } },
  });
  expect(invited.ok(), await invited.text()).toBe(true);
  const mine = await (await author.request.get(`${API}/invitations/mine`)).json();
  const invitation = mine.find((each: { project: string }) => each.project === project);
  const accepted = await author.request.post(
    `${API}/accounts/current/invitations/${invitation.id}/accept`,
  );
  expect(accepted.ok(), await accepted.text()).toBe(true);
}

const writingHere = (page: Page, id: string) =>
  flow(page)
    .locator(".scene-row")
    .filter({ has: page.locator(`[data-scene="${id}"]`) })
    .getByRole("list", { name: "Writing here" });

test("another author shows in the margin of the scene where their cursor is, and is gone once they leave", async ({
  page,
  browser,
}) => {
  // given: a book of two scenes in the writing view
  const { "Part One": scenes } = await aBookInTheWritingView(page, "Presence", { "Part One": 2 });
  const project = projectOf(page)!;
  // and: grace, an author in it, writing in it too
  const grace = await anotherAccount(browser, "Grace");
  await madeAnAuthor(page, grace.context, grace.email, project);
  const graces = await grace.context.newPage();
  await graces.goto(page.url());
  await expect(flow(graces)).toBeVisible();

  // when: grace puts her cursor into the second scene
  await scene(graces, scenes[1]).click();

  // then: the second scene shows her in its margin, by name
  await expect(writingHere(page, scenes[1]).getByRole("listitem")).toHaveAccessibleName(
    "Grace is writing here",
  );
  // and: the first scene shows nobody
  await expect(writingHere(page, scenes[0])).toHaveCount(0);
  // and: she does not see herself
  await expect(writingHere(graces, scenes[1])).toHaveCount(0);

  // when: she moves her cursor into the first scene
  await scene(graces, scenes[0]).click();

  // then: she shows there instead
  await expect(writingHere(page, scenes[0]).getByRole("listitem")).toHaveAccessibleName(
    "Grace is writing here",
  );
  await expect(writingHere(page, scenes[1])).toHaveCount(0);

  // when: she closes her tab
  await grace.context.close();

  // then: she is gone at once, not only once the others stop waiting for her
  await expect(writingHere(page, scenes[0])).toHaveCount(0, { timeout: 5_000 });
});
