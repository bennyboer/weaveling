import type { Page, Route, WebSocketRoute } from "@playwright/test";

export const ADA = {
  account: "account_1",
  name: "Ada Lovelace",
  email: "ada@example.com",
  stage: "signed-in",
};
export const HER_PASSWORD = "correct horse battery";
export const THE_CODE = "123456";
export const THE_OPERATOR_CODE = "346af3a2b90cbb3ab7cd3cb07ccc2a1f";
export const THE_INVITATION = "5d1c0e7a9b3f4c6e8a2d7f1b0c9e3a5d";

export type Listed = {
  id: string;
  name: string;
  email: string;
  verified: boolean;
  operator: boolean;
  signed_up_at: string;
  locks: { kind: string; reason: string; since: string }[];
};

export const GRACE: Listed = {
  id: "account_2",
  name: "Grace Hopper",
  email: "grace@example.com",
  verified: true,
  operator: false,
  signed_up_at: "2026-10-22T08:00:00Z",
  locks: [],
};

export const HEDY: Listed = {
  id: "account_3",
  name: "Hedy Lamarr",
  email: "hedy@example.com",
  verified: false,
  operator: false,
  signed_up_at: "2026-10-23T08:00:00Z",
  locks: [],
};

export type Invited = {
  id: string;
  email: string;
  issued_at: string;
  until: string;
  status: string;
  declined_for?: string;
};

export type Server = {
  password: string;
  resetsAsked: number;
  verified: boolean;
  signedIn: boolean;
  signedOut: number;
  signedUp: { email: string; name: string } | null;
  codesSent: number;
  codesTried: number;
  projectsAsked: number;
  operator: boolean;
  listed: Listed[];
  invited: Invited[];
  invitingWaits: number;
  pageSize: number;
  signingUp: "open" | "invitation";
  invitationRefused: { status: number; type: string } | null;
  signedInAs: typeof ADA | null;
  operatorWanted: boolean;
  memberships: WebSocketRoute[];
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
    password: HER_PASSWORD,
    resetsAsked: 0,
    verified,
    signedIn: false,
    signedOut: 0,
    signedUp: null,
    codesSent: 0,
    codesTried: 0,
    projectsAsked: 0,
    operator: false,
    listed: [],
    invited: [],
    invitingWaits: 0,
    pageSize: 100,
    signingUp: "open",
    invitationRefused: null,
    signedInAs: null,
    operatorWanted: false,
    memberships: [],
  };

  await page.route(at("/api/service/mode"), (route) =>
    route.fulfill({
      json: {
        accounts: true,
        sign_up: server.signingUp,
        operator_wanted: server.operatorWanted && !server.operator,
      },
    }),
  );
  await page.route(at("/api/sessions/current"), async (route) => {
    if (route.request().method() === "DELETE") {
      server.signedIn = false;
      server.signedOut += 1;
      return route.fulfill({ status: 204 });
    }
    return server.signedIn
      ? route.fulfill({
          json: { ...(server.signedInAs ?? ADA), operator: server.operator },
        })
      : refusing(route, 401, "not-signed-in");
  });
  await page.route(at("/api/sessions"), async (route) => {
    const { email, password } = route.request().postDataJSON();
    if (email !== ADA.email || password !== server.password) {
      return refusing(route, 401, "sign-in-refused");
    }
    if (!server.verified) {
      return refusing(route, 403, "address-not-verified");
    }
    server.signedIn = true;
    return route.fulfill({ json: { ...ADA, operator: server.operator } });
  });
  await page.route(at("/api/accounts/by-invitation"), async (route) => {
    const { token, name, password } = route.request().postDataJSON();
    if (server.invitationRefused) {
      return refusing(
        route,
        server.invitationRefused.status,
        server.invitationRefused.type,
      );
    }
    if (token !== THE_INVITATION) {
      return refusing(route, 422, "invitation-invalid");
    }
    if (password.length < 12) {
      return refusing(route, 422, "password-too-short", {
        min: 12,
        actual: password.length,
      });
    }
    server.signedIn = true;
    server.signedInAs = {
      account: "account_9",
      name,
      email: "grace@example.com",
      stage: "signed-in",
    };
    return route.fulfill({
      status: 201,
      json: { ...server.signedInAs, operator: false },
    });
  });
  await page.route(at("/api/accounts"), async (route) => {
    const { email, name, password, operator_code } = route
      .request()
      .postDataJSON();
    if (server.signingUp === "invitation") {
      if (!server.operatorWanted || server.operator || !operator_code) {
        return refusing(route, 403, "signing-up-by-invitation-only");
      }
      if (operator_code.trim() !== THE_OPERATOR_CODE) {
        return refusing(route, 422, "operator-code-wrong");
      }
    }
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
  await page.route(at("/api/accounts/password-reset"), async (route) => {
    server.resetsAsked += 1;
    return route.fulfill({ status: 202 });
  });
  await page.route(at("/api/accounts/password"), async (route) => {
    const { code, password } = route.request().postDataJSON();
    if (password.length < 12) {
      return refusing(route, 422, "password-too-short", {
        min: 12,
        actual: password.length,
      });
    }
    if (code !== THE_CODE) {
      return refusing(route, 422, "verification-code-wrong");
    }
    server.password = password;
    server.verified = true;
    return route.fulfill({ status: 204 });
  });
  await page.route(at("/api/operators"), async (route) => {
    if (!server.signedIn) {
      return refusing(route, 401, "not-signed-in");
    }
    const { code } = route.request().postDataJSON();
    if (code.trim() !== THE_OPERATOR_CODE) {
      return refusing(route, 422, "operator-code-wrong");
    }
    server.operator = true;
    return route.fulfill({ status: 204 });
  });
  await page.route(at("/api/invitations"), async (route) => {
    if (!server.operator) {
      return refusing(route, 403, "not-an-operator");
    }
    if (route.request().method() === "POST") {
      if (server.invitingWaits > 0) {
        return refusing(route, 429, "too-many-tries", {
          minutes: server.invitingWaits,
        });
      }
      const { email } = route.request().postDataJSON();
      const issued = new Date();
      const invitation: Invited = {
        id: `invitation_${100 + server.invited.length}`,
        email: email.trim().toLowerCase(),
        issued_at: issued.toISOString(),
        until: new Date(issued.getTime() + 7 * 86_400_000).toISOString(),
        status: "open",
      };
      server.invited.unshift(invitation);
      return route.fulfill({ status: 201, json: invitation });
    }
    const after = new URL(route.request().url()).searchParams.get("after");
    const from = after
      ? server.invited.findIndex((invitation) => invitation.id === after) + 1
      : 0;
    const invitations = server.invited.slice(from, from + server.pageSize);
    const next =
      from + server.pageSize < server.invited.length
        ? invitations[invitations.length - 1].id
        : null;
    return route.fulfill({ json: { invitations, next } });
  });
  await page.route(
    (url) => /^\/api\/invitations\/[^/]+$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[3];
      const at = server.invited.findIndex((invitation) => invitation.id === id);
      if (at < 0) {
        return refusing(route, 404, "invitation-not-found");
      }
      server.invited.splice(at, 1);
      return route.fulfill({ status: 204 });
    },
  );
  await page.route(at("/api/instance/accounts"), async (route) => {
    if (!server.operator) {
      return refusing(route, 403, "not-an-operator");
    }
    const everyone: Listed[] = [
      ...server.listed,
      {
        id: ADA.account,
        name: ADA.name,
        email: ADA.email,
        verified: true,
        operator: server.operator,
        signed_up_at: "2026-10-21T08:00:00Z",
        locks: [],
      },
    ];
    const after = new URL(route.request().url()).searchParams.get("after");
    const from = after
      ? everyone.findIndex((account) => account.id === after) + 1
      : 0;
    const accounts = everyone.slice(from, from + server.pageSize);
    const next =
      from + server.pageSize < everyone.length
        ? accounts[accounts.length - 1].id
        : null;
    return route.fulfill({ json: { accounts, next } });
  });
  await page.route(
    (url) => /^\/api\/instance\/accounts\/[^/]+\/lock$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[4];
      const account = server.listed.find((listed) => listed.id === id);
      if (!account) {
        return refusing(route, 404, "account-not-found");
      }
      if (route.request().method() === "DELETE") {
        account.locks = [];
        return route.fulfill({ status: 204 });
      }
      const { reason } = route.request().postDataJSON();
      if (reason.trim() === "") {
        return refusing(route, 422, "lock-reason-blank");
      }
      account.locks = [
        {
          kind: "operator",
          reason: reason.trim(),
          since: new Date().toISOString(),
        },
      ];
      return route.fulfill({ status: 204 });
    },
  );
  await page.route(
    (url) => /^\/api\/instance\/accounts\/[^/]+\/operator$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[4];
      const yourself = id === ADA.account;
      const account = server.listed.find((listed) => listed.id === id);
      if (!yourself && !account) {
        return refusing(route, 404, "account-not-found");
      }
      const operators =
        server.listed.filter((listed) => listed.operator).length +
        (server.operator ? 1 : 0);
      if (route.request().method() === "DELETE") {
        if (operators === 1) {
          return refusing(route, 409, "last-operator");
        }
        if (yourself) {
          server.operator = false;
        } else {
          account!.operator = false;
        }
        return route.fulfill({ status: 204 });
      }
      if (!account!.verified) {
        return refusing(route, 409, "operator-address-not-verified");
      }
      if (account!.locks.length > 0) {
        return refusing(route, 409, "locked-cannot-become-operator");
      }
      account!.operator = true;
      return route.fulfill({ status: 204 });
    },
  );
  await page.routeWebSocket(at("/api/members/live"), (socket) => {
    server.memberships.push(socket);
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

export function toldOfMembers(
  server: Server,
  of: string,
  event: Record<string, string>,
) {
  for (const socket of server.memberships) {
    socket.send(JSON.stringify({ of, version: 2, event }));
  }
}
