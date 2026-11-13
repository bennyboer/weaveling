import { test, expect, type Page } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, signIn } from "./support/accounts";

const MB = 1024 * 1024;
const GB = 1024 * MB;
const EVERY_ACCOUNT_S_LIMIT = GB;

type Held = {
  account: string;
  name: string | null;
  email: string | null;
  used: number;
  own: number | null;
};

type Project = {
  project: string;
  name: string | null;
  used: number;
  owners: { account: string; name: string | null; email: string | null }[];
};

type Storage = {
  accounts: Held[];
  projects: Project[];
  pageSize: number;
  searched: string[];
  given: { account: string; most: number }[];
  takenBack: string[];
};

const GRACE: Held = {
  account: "account_grace",
  name: "Grace Hopper",
  email: "grace@example.com",
  used: 2 * MB,
  own: null,
};
const HEDY: Held = {
  account: "account_hedy",
  name: "Hedy Lamarr",
  email: "hedy@example.com",
  used: MB,
  own: 4 * GB,
};

/// Answers the storage routes: the instance's usage, the accounts the most used first, a page at a time and by search, and giving and taking back limits.
async function aStorage(page: Page, accounts: Held[], projects: Project[] = []): Promise<Storage> {
  const storage: Storage = {
    accounts: accounts.map((held) => ({ ...held })),
    projects,
    pageSize: 50,
    searched: [],
    given: [],
    takenBack: [],
  };

  await page.route(
    (url) => url.pathname === "/api/assets/storage",
    (route) =>
      route.fulfill({
        json: {
          used: storage.accounts.reduce((sum, held) => sum + held.used, 0),
          most: 10 * GB,
        },
      }),
  );
  await page.route(
    (url) => url.pathname === "/api/assets/storage/accounts",
    (route) => {
      const url = new URL(route.request().url());
      const term = (url.searchParams.get("search") ?? "").toLowerCase();
      storage.searched.push(term);
      const found = storage.accounts
        .filter((held) =>
          `${held.name ?? ""} ${held.email ?? ""}`.toLowerCase().includes(term),
        )
        .sort((one, other) => other.used - one.used);
      const after = url.searchParams.get("after");
      const from = after ? found.findIndex((held) => held.account === after) + 1 : 0;
      const listed = found.slice(from, from + storage.pageSize);
      const more = from + storage.pageSize < found.length;

      return route.fulfill({
        json: {
          accounts: listed.map((held) => ({
            account: held.account,
            name: held.name,
            email: held.email,
            used: held.used,
            most: held.own ?? EVERY_ACCOUNT_S_LIMIT,
            own_limit: held.own !== null,
          })),
          next: more ? listed[listed.length - 1].account : null,
        },
      });
    },
  );
  await page.route(
    (url) => url.pathname === "/api/assets/storage/projects",
    (route) => {
      const url = new URL(route.request().url());
      const term = (url.searchParams.get("search") ?? "").toLowerCase();
      storage.searched.push(term);
      const found = storage.projects
        .filter((project) => (project.name ?? "").toLowerCase().includes(term))
        .sort((one, other) => other.used - one.used);
      const after = url.searchParams.get("after");
      const from = after ? found.findIndex((project) => project.project === after) + 1 : 0;
      const listed = found.slice(from, from + storage.pageSize);
      const more = from + storage.pageSize < found.length;

      return route.fulfill({
        json: {
          projects: listed.map((project) => ({ ...project, most: 10 * MB })),
          next: more ? listed[listed.length - 1].project : null,
        },
      });
    },
  );
  await page.route(
    (url) => /^\/api\/assets\/accounts\/[^/]+\/limit$/.test(url.pathname),
    async (route) => {
      const account = new URL(route.request().url()).pathname.split("/")[4];
      const held = storage.accounts.find((each) => each.account === account);
      if (route.request().method() === "PUT") {
        const most = (await route.request().postDataJSON()).most as number;
        storage.given.push({ account, most });
        if (held) {
          held.own = most;
        }
      } else {
        storage.takenBack.push(account);
        if (held) {
          held.own = null;
        }
      }

      return route.fulfill({ status: 204 });
    },
  );

  return storage;
}

async function anOperatorLookingAtTheStorage(
  page: Page,
  accounts = [HEDY, GRACE],
  projects: Project[] = [],
) {
  const storage = await aStorage(page, accounts, projects);
  const server = await aServerWithAccounts(page);
  server.operator = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
  await page.goto("/instance");
  await page.getByRole("navigation", { name: "Sections" }).getByRole("link", { name: "Storage" }).click();

  return storage;
}

const listed = (page: Page) => page.locator(".storage-accounts .storage-use > li");
const projectsListed = (page: Page) => page.locator(".storage-projects .storage-use > li");

const THE_LOOM: Project = {
  project: "project_loom",
  name: "The Loom",
  used: 3 * MB,
  owners: [
    { account: "account_grace", name: "Grace Hopper", email: "grace@example.com" },
    { account: "account_lost", name: null, email: null },
  ],
};
const THE_WEFT: Project = {
  project: "project_weft",
  name: "The Weft",
  used: MB,
  owners: [{ account: "account_hedy", name: "Hedy Lamarr", email: "hedy@example.com" }],
};
const row = (page: Page, name: string) => listed(page).filter({ hasText: name });

test("the operator sees what the instance uses, and each account the most used first", async ({
  page,
}) => {
  // given: an operator, Grace using 2 MB, Hedy 1 MB with her own limit
  await anOperatorLookingAtTheStorage(page);

  // then: the instance's use is told against its limit
  await expect(page.getByRole("main")).toContainText("3 MB of 10 GB used");
  // and: the accounts, the one using the most first, with their name, address and limit
  await expect(listed(page)).toHaveCount(2);
  await expect(listed(page).nth(0)).toContainText("Grace Hopper");
  await expect(listed(page).nth(0)).toContainText("grace@example.com");
  await expect(listed(page).nth(0)).toContainText("2 MB of 1 GB used");
  await expect(listed(page).nth(1)).toContainText("1 MB of 4 GB used");
  // and: only Hedy's limit is marked as her own
  await expect(row(page, "Hedy Lamarr")).toContainText("Own limit");
  await expect(row(page, "Grace Hopper")).not.toContainText("Own limit");
});

test("an account is found by its name or address, and none matching says so", async ({
  page,
}) => {
  // given: an operator looking at the storage
  const storage = await anOperatorLookingAtTheStorage(page);
  await expect(listed(page)).toHaveCount(2);

  // when: she searches for Hedy's address
  await page.getByLabel("Find an account").fill("HEDY@");

  // then: Hedy alone is listed, and the term was sent to the server
  await expect(listed(page)).toHaveCount(1);
  await expect(listed(page).nth(0)).toContainText("Hedy Lamarr");
  expect(storage.searched).toContain("hedy@");
  // when: she searches for someone who uses nothing
  await page.getByLabel("Find an account").fill("nobody");
  // then: no account matches
  await expect(page.getByText("No account matches.")).toBeVisible();
});

test("an account without a known name is shown by its id", async ({ page }) => {
  // given: an account whose name the assets never heard
  await anOperatorLookingAtTheStorage(page, [
    { account: "account_lost", name: null, email: null, used: MB, own: null },
  ]);

  // then: it is listed by its id
  await expect(listed(page).nth(0)).toContainText("account_lost");
});

test("the operator gives an account a limit of its own, in megabytes", async ({ page }) => {
  // given: an operator looking at the storage
  const storage = await anOperatorLookingAtTheStorage(page);

  // when: she changes Grace's limit, shown as the default in gigabytes
  await row(page, "Grace Hopper").getByRole("button", { name: "Change limit" }).click();
  const dialog = page.getByRole("dialog", { name: "Storage limit for Grace Hopper" });
  await expect(dialog.getByLabel("Limit")).toHaveValue("1");
  await expect(dialog.getByLabel("Unit")).toHaveValue("gigabytes");
  // and: sets 500 megabytes
  await dialog.getByLabel("Limit").fill("500");
  await dialog.getByLabel("Unit").selectOption({ label: "MB" });
  await dialog.getByRole("button", { name: "Save" }).click();

  // then: the limit was given in bytes
  await expect(dialog).toBeHidden();
  expect(storage.given).toEqual([{ account: "account_grace", most: 500 * MB }]);
  // and: Grace is listed with her own limit
  await expect(row(page, "Grace Hopper")).toContainText("2 MB of 500 MB used");
  await expect(row(page, "Grace Hopper")).toContainText("Own limit");
});

test("an own limit is taken back, so the default applies again", async ({ page }) => {
  // given: an operator looking at the storage
  const storage = await anOperatorLookingAtTheStorage(page);

  // when: she takes back Hedy's own limit
  await row(page, "Hedy Lamarr").getByRole("button", { name: "Change limit" }).click();
  const dialog = page.getByRole("dialog", { name: "Storage limit for Hedy Lamarr" });
  await expect(dialog.getByLabel("Limit")).toHaveValue("4");
  await dialog.getByRole("button", { name: "Use the default limit" }).click();

  // then: the limit was taken back, and Hedy is held to the default
  await expect(dialog).toBeHidden();
  expect(storage.takenBack).toEqual(["account_hedy"]);
  await expect(row(page, "Hedy Lamarr")).toContainText("1 MB of 1 GB used");
  await expect(row(page, "Hedy Lamarr")).not.toContainText("Own limit");
});

test("a limit that is no number above 0 is not sent", async ({ page }) => {
  // given: an operator changing Grace's limit
  const storage = await anOperatorLookingAtTheStorage(page);
  await row(page, "Grace Hopper").getByRole("button", { name: "Change limit" }).click();
  const dialog = page.getByRole("dialog", { name: "Storage limit for Grace Hopper" });

  // when: she enters 0 and saves
  await dialog.getByLabel("Limit").fill("0");
  await dialog.getByRole("button", { name: "Save" }).click();

  // then: she is told what to enter, and nothing was given
  await expect(dialog.getByRole("alert")).toHaveText("Enter a number greater than 0.");
  expect(storage.given).toEqual([]);
  // when: she enters one and a half, with a comma
  await dialog.getByLabel("Limit").fill("1,5");
  await dialog.getByRole("button", { name: "Save" }).click();
  // then: it is read as 1.5 gigabytes
  await expect(dialog).toBeHidden();
  expect(storage.given).toEqual([{ account: "account_grace", most: 1.5 * GB }]);
});

test("more accounts follow with Show more", async ({ page }) => {
  // given: three accounts, two to a page
  const storage = await aStorage(page, []);
  storage.pageSize = 2;
  storage.accounts.push(
    { ...GRACE },
    { ...HEDY },
    { account: "account_ada", name: "Ada Lovelace", email: ADA.email, used: 300, own: null },
  );
  const server = await aServerWithAccounts(page);
  server.operator = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(
    page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" }),
  ).toBeVisible();
  await page.goto("/instance/storage");
  await expect(listed(page)).toHaveCount(2);

  // when: she asks for more
  await page.getByRole("button", { name: "Show more" }).click();

  // then: the third follows, and there is no more
  await expect(listed(page)).toHaveCount(3);
  await expect(listed(page).nth(2)).toContainText("300 B of 1 GB used");
  await expect(page.getByRole("button", { name: "Show more" })).toHaveCount(0);
});

test("the operator sees each project the most used first, with its owners by name", async ({
  page,
}) => {
  // given: an operator, the loom using 3 MB and the weft 1 MB
  await anOperatorLookingAtTheStorage(page, [HEDY, GRACE], [THE_WEFT, THE_LOOM]);

  // then: the projects are listed, the one using the most first, against their limit
  await expect(projectsListed(page)).toHaveCount(2);
  await expect(projectsListed(page).nth(0)).toContainText("The Loom");
  await expect(projectsListed(page).nth(0)).toContainText("3 MB of 10 MB used");
  // and: its owners by name and address, or by id where the name is unknown
  await expect(projectsListed(page).nth(0)).toContainText(
    "Owners: Grace Hopper (grace@example.com), account_lost",
  );
  await expect(projectsListed(page).nth(1)).toContainText("The Weft");
});

test("a project is found by its name, and none matching says so", async ({ page }) => {
  // given: an operator looking at the loom and the weft
  await anOperatorLookingAtTheStorage(page, [], [THE_WEFT, THE_LOOM]);
  await expect(projectsListed(page)).toHaveCount(2);

  // when: she searches for the weft
  await page.getByLabel("Find a project").fill("weft");

  // then: the weft alone is listed
  await expect(projectsListed(page)).toHaveCount(1);
  await expect(projectsListed(page).nth(0)).toContainText("The Weft");
  // when: she searches for a project nobody has
  await page.getByLabel("Find a project").fill("nothing");
  // then: no project matches, and the accounts are untouched
  await expect(page.getByText("No project matches.")).toBeVisible();
  await expect(page.getByText("No account uses any storage yet.")).toBeVisible();
});
