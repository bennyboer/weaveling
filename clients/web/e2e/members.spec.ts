import { test, expect } from "@playwright/test";

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
  await expect(people.nth(0)).toContainText("Owner");
  await expect(people.nth(1)).toContainText("Grace Hopper");
  await expect(people.nth(1)).toContainText("Author");
  await expect(people.nth(1)).not.toContainText("(you)");
  await expect(people.nth(2)).toContainText("Not known by name yet");
  await expect(people.nth(2)).toContainText("Reader");
  expect(asked).toEqual([project]);
});

test("without accounts a project has no members to show", async ({ page }) => {
  const project = await aProject(page);

  await page.goto(`/projects/${project}/write`);

  await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
  await expect(page.getByRole("link", { name: "Members" })).toHaveCount(0);
});
