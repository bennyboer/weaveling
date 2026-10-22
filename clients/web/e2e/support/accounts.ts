import type { Page, Route } from "@playwright/test";

export const ADA = {
  account: "account_1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  stage: "signed-in",
};
export const HER_PASSWORD = "correct horse battery";
export const THE_CODE = "123456";

export type Server = {
  verified: boolean;
  signedIn: boolean;
  signedOut: number;
  signedUp: { email: string; name: string } | null;
  codesSent: number;
  codesTried: number;
  projectsAsked: number;
};

const refusing = (
  route: Route,
  status: number,
  type: string,
  particulars: Record<string, number> = {},
) =>
  route.fulfill({
    status,
    contentType: "application/problem+json",
    body: JSON.stringify({ type, status, detail: type, ...particulars }),
  });

const at = (path: string) => (url: URL) => url.pathname === path;

export async function aServerWithAccounts(
  page: Page,
  { verified = true } = {},
): Promise<Server> {
  const server: Server = {
    verified,
    signedIn: false,
    signedOut: 0,
    signedUp: null,
    codesSent: 0,
    codesTried: 0,
    projectsAsked: 0,
  };

  await page.route(at("/api/service/mode"), (route) =>
    route.fulfill({ json: { accounts: true } }),
  );
  await page.route(at("/api/sessions/current"), async (route) => {
    if (route.request().method() === "DELETE") {
      server.signedIn = false;
      server.signedOut += 1;
      return route.fulfill({ status: 204 });
    }
    return server.signedIn
      ? route.fulfill({ json: ADA })
      : refusing(route, 401, "not-signed-in");
  });
  await page.route(at("/api/sessions"), async (route) => {
    const { email, password } = route.request().postDataJSON();
    if (email !== ADA.email || password !== HER_PASSWORD) {
      return refusing(route, 401, "sign-in-refused");
    }
    if (!server.verified) {
      return refusing(route, 403, "address-not-verified");
    }
    server.signedIn = true;
    return route.fulfill({ json: ADA });
  });
  await page.route(at("/api/accounts"), async (route) => {
    const { email, name, password } = route.request().postDataJSON();
    if (password.length < 12) {
      return refusing(route, 422, "password-too-short", {
        min: 12,
        actual: password.length,
      });
    }
    server.signedUp = { email, name };
    server.verified = false;
    server.codesSent += 1;
    return route.fulfill({ status: 202 });
  });
  await page.route(at("/api/accounts/verification"), async (route) => {
    const { code } = route.request().postDataJSON();
    server.codesTried += 1;
    if (code !== THE_CODE) {
      return refusing(route, 422, "verification-code-wrong");
    }
    server.verified = true;
    return route.fulfill({ status: 204 });
  });
  await page.route(at("/api/accounts/verification/code"), async (route) => {
    server.codesSent += 1;
    return route.fulfill({ status: 202 });
  });
  await page.route(at("/api/projects"), async (route) => {
    server.projectsAsked += 1;
    return server.signedIn
      ? route.fallback()
      : refusing(route, 401, "not-signed-in");
  });

  return server;
}

export async function signIn(page: Page, email: string, password: string) {
  await page.getByLabel("Email address").fill(email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
}
