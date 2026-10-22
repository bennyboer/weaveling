import { test, expect, type Page, type Route } from "@playwright/test";

const ADA = {
  account: "account_1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  stage: "signed-in",
};
const HER_PASSWORD = "correct horse battery";

type Server = { signedIn: boolean; signedOut: number; projectsAsked: number };

const refusing = (route: Route, status: number, type: string) =>
  route.fulfill({
    status,
    contentType: "application/problem+json",
    body: JSON.stringify({ type, status, detail: type }),
  });

async function aServerWithAccounts(
  page: Page,
  { verified = true } = {},
): Promise<Server> {
  const server: Server = { signedIn: false, signedOut: 0, projectsAsked: 0 };

  await page.route(
    (url) => url.pathname === "/api/sessions/current",
    async (route) => {
      if (route.request().method() === "DELETE") {
        server.signedIn = false;
        server.signedOut += 1;
        return route.fulfill({ status: 204 });
      }
      return server.signedIn
        ? route.fulfill({ json: ADA })
        : refusing(route, 401, "not-signed-in");
    },
  );
  await page.route(
    (url) => url.pathname === "/api/sessions",
    async (route) => {
      const { email, password } = route.request().postDataJSON();
      if (email !== ADA.email || password !== HER_PASSWORD) {
        return refusing(route, 401, "sign-in-refused");
      }
      if (!verified) {
        return refusing(route, 403, "address-not-verified");
      }
      server.signedIn = true;
      return route.fulfill({ json: ADA });
    },
  );
  await page.route(
    (url) => url.pathname === "/api/projects",
    async (route) => {
      server.projectsAsked += 1;
      return route.fallback();
    },
  );

  return server;
}

async function signIn(page: Page, email: string, password: string) {
  await page.getByLabel("Email address").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in" }).click();
}

const masthead = (page: Page) => page.getByRole("banner");

test("without accounts, the work opens as it always did", async ({ page }) => {
  await page.goto("/");

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
  await expect(
    masthead(page).getByRole("button", { name: "Sign out" }),
  ).toHaveCount(0);
});

test("on a server, signing in comes before any of the work is asked for", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toHaveCount(0);
  expect(server.projectsAsked).toBe(0);
});

test("signing in opens the work and names who signed in", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  await expect(masthead(page)).toContainText("Ada Lovelace");
});

test("signing in keeps the address that was asked for", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/?from=a-link");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  expect(new URL(page.url()).search).toBe("?from=a-link");
});

test("a wrong password says so and stays on signing in", async ({ page }) => {
  await aServerWithAccounts(page);
  await page.goto("/");

  await signIn(page, ADA.email, "not her password at all");

  await expect(page.getByRole("alert")).toHaveText(
    "This email address and password do not sign in.",
  );
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
});

test("an address not yet confirmed is told to enter its code", async ({
  page,
}) => {
  await aServerWithAccounts(page, { verified: false });
  await page.goto("/");

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(page.getByRole("alert")).toHaveText(
    "This email address is not confirmed yet. Enter the code we sent to it.",
  );
});

test("signing out ends the session and asks to sign in again", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(masthead(page)).toContainText("Ada Lovelace");

  await masthead(page).getByRole("button", { name: "Sign out" }).click();

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  expect(server.signedOut).toBe(1);
});

test("signing in can be done in the reader's own language", async ({
  browser,
}) => {
  const context = await browser.newContext({ locale: "de-DE" });
  const page = await context.newPage();
  await aServerWithAccounts(page);

  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Anmelden" })).toBeVisible();
  await expect(page.getByLabel("Passwort")).toBeVisible();
  await context.close();
});
