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
export const THE_TOTP_SECRET = "JBSWY3DPEHPK3PXPKZ4M2QHTJBSWY3DP";
export const THE_APP_CODE = "481920";
export const THE_NEW_RECOVERY_CODES = [
  "n2ew3-cd4ef",
  "n3ew4-cd5ef",
  "n4ew5-cd6ef",
  "n5ew6-cd7ef",
  "n6ew7-cd8ef",
  "n7ew8-cd9ef",
  "n8ew9-cd2ef",
  "n9ew2-cd3ef",
  "n2ex3-cd4ef",
  "n3ex4-cd5ef",
];
export const THE_RECOVERY_CODES = [
  "k7qp3-mx9tw",
  "r2dn8-vc4hf",
  "b9wz4-tq7ke",
  "m3hx6-pz2sn",
  "f8cj5-yr3dq",
  "t4vg9-nk6wb",
  "z6ep2-hs8mx",
  "q5rz7-dj3vc",
  "w8nb4-gx9tp",
  "h3ks6-cf2rz",
];

export type Listed = {
  id: string;
  name: string;
  email: string;
  verified: boolean;
  operator: boolean;
  signed_up_at: string;
  locks: { kind: string; reason: string; since: string }[];
  second_factor?: boolean;
  ways?: WaysIn;
  version?: number;
};

export type WaysIn = {
  keys: {
    key: string;
    name: string;
    role: "primary" | "second-factor";
    since: string;
  }[];
  totp_since: string | null;
  recovery_codes_left: number | null;
};

function waysOf(account: Listed): WaysIn {
  account.ways ??= {
    keys: [],
    totp_since: account.second_factor ? "2026-11-01T09:00:00Z" : null,
    recovery_codes_left: account.second_factor ? 10 : null,
  };
  return account.ways;
}

function secondStepLeft(account: Listed) {
  account.version = (account.version ?? 0) + 1;
  const ways = waysOf(account);
  account.second_factor =
    ways.totp_since !== null ||
    ways.keys.some((key) => key.role === "second-factor");
  if (!account.second_factor) {
    ways.recovery_codes_left = null;
  }
}

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

export type ToYou = {
  id: string;
  project: string;
  project_name?: string;
  role: string;
  invited_by?: string;
  until: string;
};

export type Subscribed = {
  id: string;
  topic: string;
  about: Record<string, string>;
  socket: WebSocketRoute;
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
  subscribed: Subscribed[];
  subscribing: number;
  liveSockets: WebSocketRoute[];
  mineListed: number;
  toYou: ToYou[];
  answered: string[];
  declinedByLink: string[];
  memberships: { project: string; role: string }[] | null;
  notices: Noticed[];
  dismissed: string[];
  totpOffered: boolean;
  secondFactor: {
    since: string;
    recovery_codes_left: number;
    answered_by_totp: boolean;
    answered_by_key: boolean;
  } | null;
  owing: boolean;
  recoveryCodes: string[];
  keysOffered: boolean;
  passkeysAskedFor: number;
  keys: HeldKey[];
  keyAskedFor: number;
  renewalLags: number;
  renewing: boolean;
  preferredWay: "totp" | "key" | "last-used";
  addingRole: "primary" | "second-factor";
  replacing: string | null;
  answeredLastBy: "totp" | "key" | null;
  deletion: { until: string; by: "holder" | "operator" } | null;
  deletionPending: boolean;
  keptAccount: number;
  toHandOver: string[];
};

export type HeldKey = {
  key: string;
  name: string;
  role: "primary" | "second-factor";
  since: string;
  credential: string;
};

export type Noticed = {
  id: string;
  noted_at: string;
  about: { kind: "role-changed"; project: string; role: string; by?: string };
  names?: Record<string, string>;
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

const deletionPendingTold = (server: Server) => ({
  ...ADA,
  stage: "deletion-pending",
  operator: false,
  deletion: server.deletion,
});

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
    subscribed: [],
    subscribing: 0,
    liveSockets: [],
    mineListed: 0,
    toYou: [],
    answered: [],
    declinedByLink: [],
    memberships: null,
    notices: [],
    dismissed: [],
    totpOffered: false,
    secondFactor: null,
    owing: false,
    recoveryCodes: [...THE_RECOVERY_CODES],
    keysOffered: false,
    passkeysAskedFor: 0,
    keys: [],
    keyAskedFor: 60000,
    renewalLags: 0,
    renewing: false,
    preferredWay: "last-used",
    addingRole: "second-factor",
    replacing: null,
    answeredLastBy: null,
    deletion: null,
    deletionPending: false,
    keptAccount: 0,
    toHandOver: [],
  };

  await page.route(at("/api/service/mode"), (route) =>
    route.fulfill({
      json: {
        accounts: {
          sign_up: server.signingUp,
          operator_wanted: server.operatorWanted && !server.operator,
          can_set_up_totp: server.totpOffered,
          can_add_keys: server.keysOffered,
        },
      },
    }),
  );
  await page.route(at("/api/sessions/current"), async (route) => {
    if (route.request().method() === "DELETE") {
      server.owing = false;
      server.deletionPending = false;
      server.signedIn = false;
      server.signedOut += 1;
      return route.fulfill({ status: 204 });
    }
    if (server.renewing) {
      return refusing(route, 401, "not-signed-in");
    }
    if (server.owing) {
      return route.fulfill({
        json: {
          ...ADA,
          stage: "second-factor-owed",
          operator: false,
          second_factor: secondFactorTold(server),
        },
      });
    }
    if (server.deletionPending) {
      return route.fulfill({ json: deletionPendingTold(server) });
    }
    return server.signedIn
      ? route.fulfill({
          json: {
            ...(server.signedInAs ?? ADA),
            operator: server.operator,
            ...(server.secondFactor
              ? { second_factor: secondFactorTold(server) }
              : {}),
          },
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
    if (server.secondFactor) {
      server.owing = true;
      return route.fulfill({
        json: {
          ...ADA,
          stage: "second-factor-owed",
          operator: false,
          second_factor: secondFactorTold(server),
        },
      });
    }
    if (server.deletion) {
      server.deletionPending = true;
      return route.fulfill({ json: deletionPendingTold(server) });
    }
    server.signedIn = true;
    return route.fulfill({ json: { ...ADA, operator: server.operator } });
  });
  await page.route(at("/api/accounts/current/deletion"), async (route) => {
    if (route.request().method() === "DELETE") {
      if (!server.deletionPending) {
        return refusing(route, 409, "no-deletion-pending");
      }
      if (server.deletion?.by === "operator") {
        return refusing(route, 403, "deletion-asked-by-the-operator");
      }
      server.deletion = null;
      server.deletionPending = false;
      server.signedIn = true;
      server.keptAccount += 1;
      return route.fulfill({ json: { ...ADA, operator: server.operator } });
    }
    const { password } = route.request().postDataJSON();
    if (password !== server.password) {
      return refusing(route, 422, "password-wrong");
    }
    if (server.toHandOver.length > 0) {
      return route.fulfill({
        status: 409,
        contentType: "application/problem+json",
        body: JSON.stringify({
          type: "projects-to-hand-over",
          status: 409,
          detail: "projects-to-hand-over",
          projects: server.toHandOver,
        }),
      });
    }
    server.deletion = {
      until: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString(),
      by: "holder",
    };
    server.signedIn = false;
    return route.fulfill({ json: server.deletion });
  });
  await page.route(at("/api/invitations/decline"), async (route) => {
    const { token } = route.request().postDataJSON();
    if (server.invitationRefused) {
      return refusing(
        route,
        server.invitationRefused.status,
        server.invitationRefused.type,
      );
    }
    if (token !== THE_INVITATION || server.declinedByLink.includes(token)) {
      return refusing(route, 422, "invitation-invalid");
    }
    server.declinedByLink.push(token);
    return route.fulfill({ status: 204 });
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
  await page.route(at("/api/accounts/current/totp"), async (route) => {
    const { password } = route.request().postDataJSON();
    if (password !== server.password) {
      return refusing(route, 422, "password-wrong");
    }
    if (!server.secondFactor?.answered_by_totp) {
      return refusing(route, 409, "no-second-factor");
    }
    if (server.secondFactor.answered_by_key) {
      server.secondFactor.answered_by_totp = false;
    } else {
      server.secondFactor = null;
    }
    socketsToldTheSessionEnded(server);
    return route.fulfill({ status: 204 });
  });
  await page.route(at("/api/accounts/current/totp/setup"), async (route) => {
    if (!server.signedIn) {
      return refusing(route, 401, "not-signed-in");
    }
    if (!server.totpOffered) {
      return refusing(route, 409, "totp-unavailable");
    }
    return route.fulfill({
      json: {
        secret: THE_TOTP_SECRET,
        otpauth_uri: `otpauth://totp/Weaveling:ada%40example.com?secret=${THE_TOTP_SECRET}&issuer=Weaveling`,
      },
    });
  });
  await page.route(
    at("/api/accounts/current/totp/setup/finish"),
    async (route) => {
      const { code } = route.request().postDataJSON();
      if (code !== THE_APP_CODE) {
        return refusing(route, 422, "totp-code-wrong");
      }
      server.secondFactor = {
        since: new Date().toISOString(),
        recovery_codes_left: 10,
        answered_by_totp: true,
        answered_by_key: false,
      };
      socketsToldTheSessionEnded(server);
      return route.fulfill({ json: { codes: THE_RECOVERY_CODES } });
    },
  );
  const theKeyNamed = (url: URL) =>
    /^\/api\/accounts\/current\/keys\/(?!challenge$)[^/]+$/.test(url.pathname);
  const secondStepKeys = () =>
    server.keys.filter((key) => key.role === "second-factor");
  const listed = (key: HeldKey) => ({
    key: key.key,
    name: key.name,
    role: key.role,
    since: key.since,
  });
  await page.route(
    at("/api/accounts/current/second-step/preferred"),
    async (route) => {
      const { preferred_way } = route.request().postDataJSON();
      if (!["totp", "key", "last-used"].includes(preferred_way)) {
        return refusing(route, 400, "preferred-way-unknown");
      }
      server.preferredWay = preferred_way;
      return route.fulfill({ status: 204 });
    },
  );
  await page.route(
    at("/api/accounts/current/keys/challenge"),
    async (route) => {
      if (!server.keysOffered) {
        return refusing(route, 409, "keys-unavailable");
      }
      const asked = route.request().postDataJSON() ?? {};
      server.addingRole = asked.role ?? "second-factor";
      server.replacing = asked.replacing ?? null;
      const passkey = server.addingRole === "primary";
      const challenge = Buffer.from(
        crypto.getRandomValues(new Uint8Array(32)),
      ).toString("base64url");
      return route.fulfill({
        json: {
          options: {
            publicKey: {
              rp: { id: "localhost", name: "Weaveling" },
              user: {
                id: Buffer.from(ADA.account).toString("base64url"),
                name: ADA.email,
                displayName: ADA.name,
              },
              challenge,
              pubKeyCredParams: [{ type: "public-key", alg: -7 }],
              timeout: server.keyAskedFor,
              attestation: "none",
              authenticatorSelection: passkey
                ? {
                    residentKey: "required",
                    requireResidentKey: true,
                    userVerification: "required",
                  }
                : { userVerification: "preferred" },
              excludeCredentials: server.keys
                .filter((key) => key.key !== server.replacing)
                .map((key) => ({ type: "public-key", id: key.credential })),
            },
          },
        },
      });
    },
  );
  await page.route(at("/api/accounts/current/keys"), async (route) => {
    if (route.request().method() === "GET") {
      return route.fulfill({ json: server.keys.map(listed) });
    }
    const { name, answer } = route.request().postDataJSON();
    if (!name.trim()) {
      return refusing(route, 422, "key-name-blank");
    }
    if (server.keys.some((key) => key.name === name)) {
      return refusing(route, 409, "key-name-taken");
    }
    const key = `key_${crypto.randomUUID().replaceAll("-", "").slice(0, 22)}`;
    server.keys.push({
      key,
      name,
      role: server.addingRole,
      since: new Date().toISOString(),
      credential: answer.rawId,
    });
    await sessionRenewed(server);
    if (server.addingRole === "primary") {
      return route
        .fulfill({ status: 201, json: { key } })
        .finally(() => (server.renewing = false));
    }
    if (server.secondFactor) {
      server.secondFactor.answered_by_key = true;
      return route
        .fulfill({ status: 201, json: { key } })
        .finally(() => (server.renewing = false));
    }
    server.recoveryCodes = [...THE_RECOVERY_CODES];
    server.secondFactor = {
      since: new Date().toISOString(),
      recovery_codes_left: 10,
      answered_by_totp: false,
      answered_by_key: true,
    };
    return route
      .fulfill({
        status: 201,
        json: { key, recovery_codes: THE_RECOVERY_CODES },
      })
      .finally(() => (server.renewing = false));
  });
  const keyChanged = (url: URL) =>
    /^\/api\/accounts\/current\/keys\/[^/]+\/(promote|demote)$/.test(
      url.pathname,
    );
  await page.route(keyChanged, async (route) => {
    const [, , , , , id, change] = new URL(
      route.request().url(),
    ).pathname.split("/");
    const held = server.keys.find((key) => key.key === id);
    if (!held) {
      return refusing(route, 404, "no-such-key");
    }
    const { password, answer } = route.request().postDataJSON();
    if (password !== server.password) {
      return refusing(route, 422, "password-wrong");
    }
    socketsToldTheSessionEnded(server);
    if (change === "promote") {
      held.role = "primary";
      held.credential = answer.rawId;
      if (server.secondFactor) {
        server.secondFactor.answered_by_key = secondStepKeys().length > 0;
        if (
          !server.secondFactor.answered_by_key &&
          !server.secondFactor.answered_by_totp
        ) {
          server.secondFactor = null;
        }
      }
      return route.fulfill({ json: { key: held.key } });
    }
    held.role = "second-factor";
    if (server.secondFactor) {
      server.secondFactor.answered_by_key = true;
      return route.fulfill({ json: { key: held.key } });
    }
    server.recoveryCodes = [...THE_RECOVERY_CODES];
    server.secondFactor = {
      since: new Date().toISOString(),
      recovery_codes_left: 10,
      answered_by_totp: false,
      answered_by_key: true,
    };
    return route.fulfill({
      json: { key: held.key, recovery_codes: THE_RECOVERY_CODES },
    });
  });
  await page.route(theKeyNamed, async (route) => {
    const id = new URL(route.request().url()).pathname.split("/").at(-1);
    const held = server.keys.find((key) => key.key === id);
    if (!held) {
      return refusing(route, 404, "no-such-key");
    }
    if (route.request().method() === "PATCH") {
      const { name } = route.request().postDataJSON();
      if (server.keys.some((key) => key !== held && key.name === name)) {
        return refusing(route, 409, "key-name-taken");
      }
      held.name = name;
      return route.fulfill({ status: 204 });
    }
    const { password } = route.request().postDataJSON();
    if (password !== server.password) {
      return refusing(route, 422, "password-wrong");
    }
    server.keys = server.keys.filter((key) => key !== held);
    if (server.secondFactor) {
      server.secondFactor.answered_by_key = secondStepKeys().length > 0;
      if (
        !server.secondFactor.answered_by_key &&
        !server.secondFactor.answered_by_totp
      ) {
        server.secondFactor = null;
      }
    }
    socketsToldTheSessionEnded(server);
    return route.fulfill({ status: 204 });
  });
  const signedInAllTheWay = (route: Route) => {
    server.owing = false;
    server.signedIn = true;
    return route.fulfill({
      json: {
        ...ADA,
        operator: server.operator,
        second_factor: secondFactorTold(server),
      },
    });
  };
  await page.route(at("/api/sessions/passkey/challenge"), async (route) => {
    server.passkeysAskedFor += 1;
    if (!server.keysOffered) {
      return refusing(route, 409, "keys-unavailable");
    }
    return route.fulfill({
      json: {
        signing_in: "signing-in-1",
        options: {
          publicKey: {
            challenge: Buffer.from(
              crypto.getRandomValues(new Uint8Array(32)),
            ).toString("base64url"),
            rpId: "localhost",
            timeout: server.keyAskedFor,
            userVerification: "required",
          },
        },
      },
    });
  });
  await page.route(at("/api/sessions/passkey"), async (route) => {
    const { signing_in, answer } = route.request().postDataJSON();
    const passkey = server.keys.find(
      (key) => key.role === "primary" && key.credential === answer.rawId,
    );
    if (signing_in !== "signing-in-1" || !passkey) {
      return refusing(route, 422, "key-refused");
    }
    server.signedIn = true;
    return route.fulfill({
      json: {
        ...ADA,
        operator: server.operator,
        ...(server.secondFactor
          ? { second_factor: secondFactorTold(server) }
          : {}),
      },
    });
  });
  await page.route(at("/api/sessions/current/key/challenge"), async (route) => {
    if (!server.owing) {
      return refusing(route, 401, "not-signed-in");
    }
    if (secondStepKeys().length === 0) {
      return refusing(route, 404, "no-such-key");
    }
    return route.fulfill({
      json: {
        options: {
          publicKey: {
            challenge: Buffer.from(
              crypto.getRandomValues(new Uint8Array(32)),
            ).toString("base64url"),
            rpId: "localhost",
            timeout: server.keyAskedFor,
            userVerification: "preferred",
            allowCredentials: secondStepKeys().map((key) => ({
              type: "public-key",
              id: key.credential,
            })),
          },
        },
      },
    });
  });
  await page.route(at("/api/sessions/current/key"), async (route) => {
    if (!server.owing) {
      return refusing(route, 401, "not-signed-in");
    }
    const { answer } = route.request().postDataJSON();
    if (!secondStepKeys().some((key) => key.credential === answer.rawId)) {
      return refusing(route, 422, "key-refused");
    }
    server.answeredLastBy = "key";
    return signedInAllTheWay(route);
  });
  await page.route(at("/api/sessions/current/totp"), async (route) => {
    if (!server.owing) {
      return refusing(route, 401, "not-signed-in");
    }
    const { code } = route.request().postDataJSON();
    if (code !== THE_APP_CODE) {
      return refusing(route, 422, "totp-code-wrong");
    }
    server.answeredLastBy = "totp";
    return signedInAllTheWay(route);
  });
  await page.route(at("/api/sessions/current/recovery-code"), async (route) => {
    if (!server.owing) {
      return refusing(route, 401, "not-signed-in");
    }
    const { code } = route.request().postDataJSON();
    const typed = code.replace(/[\s-]/g, "").toLowerCase();
    const at = server.recoveryCodes.findIndex(
      (kept) => kept.replace("-", "") === typed,
    );
    if (at < 0 || !server.secondFactor) {
      return refusing(route, 422, "recovery-code-wrong");
    }
    server.recoveryCodes.splice(at, 1);
    server.secondFactor.recovery_codes_left = server.recoveryCodes.length;
    return signedInAllTheWay(route);
  });
  await page.route(
    at("/api/accounts/current/recovery-codes"),
    async (route) => {
      const { password } = route.request().postDataJSON();
      if (password !== server.password) {
        return refusing(route, 422, "password-wrong");
      }
      if (!server.secondFactor) {
        return refusing(route, 409, "no-second-factor");
      }
      server.recoveryCodes = [...THE_NEW_RECOVERY_CODES];
      if (server.secondFactor) {
        server.secondFactor.recovery_codes_left = 10;
      }
      return route.fulfill({ json: { codes: THE_NEW_RECOVERY_CODES } });
    },
  );
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
      if (route.request().method() !== "DELETE") {
        return route.fallback();
      }
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
    (url) => /^\/api\/instance\/accounts\/[^/]+\/totp$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[4];
      const account = server.listed.find((listed) => listed.id === id);
      if (!account) {
        return refusing(route, 404, "account-not-found");
      }
      waysOf(account).totp_since = null;
      secondStepLeft(account);
      return route.fulfill({ status: 204 });
    },
  );
  await page.route(
    (url) => /^\/api\/instance\/accounts\/[^/]+\/ways-in$/.test(url.pathname),
    async (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[4];
      const account = server.listed.find((listed) => listed.id === id);
      if (!account) {
        return refusing(route, 404, "account-not-found");
      }
      return route.fulfill({
        json: { version: account.version ?? 0, ...waysOf(account) },
      });
    },
  );
  await page.route(
    (url) =>
      /^\/api\/instance\/accounts\/[^/]+\/keys\/[^/]+$/.test(url.pathname),
    async (route) => {
      const [, , , , id, , key] = new URL(route.request().url()).pathname.split(
        "/",
      );
      const account = server.listed.find((listed) => listed.id === id);
      if (!account) {
        return refusing(route, 404, "account-not-found");
      }
      const ways = waysOf(account);
      const at = ways.keys.findIndex((held) => held.key === key);
      if (at < 0) {
        return refusing(route, 404, "no-such-key");
      }
      ways.keys.splice(at, 1);
      secondStepLeft(account);
      return route.fulfill({ status: 204 });
    },
  );
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
  await page.routeWebSocket(at("/api/live"), (socket) => {
    server.liveSockets.push(socket);
    const others = (subscribed: Subscribed) => subscribed.socket !== socket;
    socket.onMessage((data) => {
      const asked = JSON.parse(String(data));
      const elsewhere = (subscribed: Subscribed) =>
        others(subscribed) || subscribed.id !== asked.id;
      server.subscribed = server.subscribed.filter(elsewhere);
      if (asked.do === "subscribe") {
        server.subscribing += 1;
        server.subscribed.push({
          id: asked.id,
          topic: asked.topic,
          about: asked.about,
          socket,
        });
        socket.send(JSON.stringify({ told: "following", id: asked.id }));
      }
    });
    socket.onClose(() => {
      server.liveSockets = server.liveSockets.filter((open) => open !== socket);
      server.subscribed = server.subscribed.filter(others);
    });
  });
  await page.route(at("/api/members/mine"), (route) =>
    server.memberships === null
      ? route.fallback()
      : route.fulfill({ json: server.memberships }),
  );
  await page.route(at("/api/notifications"), (route) =>
    server.signedIn
      ? route.fulfill({ json: server.notices })
      : refusing(route, 401, "not-signed-in"),
  );
  await page.route(
    (url) => /^\/api\/notifications\/[^/]+\/dismissed$/.test(url.pathname),
    (route) => {
      const id = new URL(route.request().url()).pathname.split("/")[3];
      if (!server.notices.some((notice) => notice.id === id)) {
        return refusing(route, 404, "not-found");
      }
      server.notices = server.notices.filter((notice) => notice.id !== id);
      server.dismissed.push(id);
      return route.fulfill({ status: 204 });
    },
  );
  await page.route(at("/api/invitations/mine"), (route) => {
    server.mineListed += 1;
    return server.signedIn
      ? route.fulfill({ json: server.toYou })
      : refusing(route, 401, "not-signed-in");
  });
  await page.route(
    (url) =>
      /^\/api\/accounts\/current\/invitations\/[^/]+\/(accept|decline)$/.test(
        url.pathname,
      ),
    async (route) => {
      const [, , , , , id, answer] = new URL(
        route.request().url(),
      ).pathname.split("/");
      if (!server.toYou.some((invitation) => invitation.id === id)) {
        return refusing(route, 422, "invitation-invalid");
      }
      server.toYou = server.toYou.filter((invitation) => invitation.id !== id);
      server.answered.push(`${answer}:${id}`);
      return route.fulfill({ status: 204 });
    },
  );
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

export function subscribedTo(server: Server, topic: string): Subscribed[] {
  return server.subscribed.filter((subscribed) => subscribed.topic === topic);
}

function toldOn(server: Server, topic: string, frame: Record<string, unknown>) {
  for (const subscribed of subscribedTo(server, topic)) {
    subscribed.socket.send(
      JSON.stringify({ told: "frame", id: subscribed.id, frame }),
    );
  }
}

export function toldOfMembers(
  server: Server,
  of: string,
  event: Record<string, string>,
) {
  toldOn(server, "members", { of, version: 2, event });
}

export function toldOfNotices(server: Server, of: string) {
  toldOn(server, "notifications", { of, version: 2, event: null });
}

export function toldOfMine(server: Server) {
  toldOn(server, "invitations", {
    of: "invitation_9",
    version: 2,
    event: null,
  });
}

export function endedOn(server: Server, topic: string, why: string) {
  for (const subscribed of subscribedTo(server, topic)) {
    subscribed.socket.send(
      JSON.stringify({ told: "ended", id: subscribed.id, why }),
    );
  }
}

export function sessionEndedOn(server: Server) {
  server.signedIn = false;
  socketsToldTheSessionEnded(server);
}

function secondFactorTold(server: Server) {
  const factor = server.secondFactor;
  if (!factor) {
    return null;
  }
  const answers = (way: string | null) =>
    (way === "totp" && factor.answered_by_totp) ||
    (way === "key" && factor.answered_by_key);
  const wished =
    server.preferredWay === "last-used"
      ? server.answeredLastBy
      : server.preferredWay;
  const askedWay = answers(wished)
    ? wished
    : factor.answered_by_key
      ? "key"
      : "totp";

  return { ...factor, preferred_way: server.preferredWay, asked_way: askedWay };
}

async function sessionRenewed(server: Server) {
  socketsToldTheSessionEnded(server);
  if (server.renewalLags > 0) {
    server.renewing = true;
    await new Promise((lagged) => setTimeout(lagged, server.renewalLags));
  }
}

export function socketsToldTheSessionEnded(server: Server) {
  for (const socket of server.liveSockets) {
    socket.close({ code: 4401, reason: "session-ended" });
  }
}
