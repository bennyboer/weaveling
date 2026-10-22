import { mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { expect, request } from "@playwright/test";

import { API } from "./shell";

const EMAIL = "e2e@example.com";
const PASSWORD = "a password only the tests know";

export const withAccounts = process.env.WEAVELING_E2E_ACCOUNTS === "on";

function lastCodeDropped(drop: string, to: string): string {
  const sent = readdirSync(drop)
    .sort()
    .map((name) => JSON.parse(readFileSync(join(drop, name), "utf8")))
    .filter((mail) => mail.to === to);
  const code = sent.at(-1)?.body.match(/\b\d{6}\b/)?.[0];
  if (!code) {
    throw new Error(`no code was dropped for ${to} in ${drop}`);
  }

  return code;
}

export default async function signIn() {
  const drop = process.env.WEAVELING_E2E_MAIL_DROP!;
  const kept = process.env.WEAVELING_E2E_SIGNED_IN!;
  const server = await request.newContext({ baseURL: `${API}/` });

  const signedUp = await server.post("accounts", {
    data: { email: EMAIL, name: "End To End", password: PASSWORD },
  });
  expect(signedUp.status()).toBe(202);

  const verified = await server.post("accounts/verification", {
    data: { email: EMAIL, code: lastCodeDropped(drop, EMAIL) },
  });
  expect(verified.status()).toBe(204);

  const signedIn = await server.post("sessions", {
    data: { email: EMAIL, password: PASSWORD },
  });
  expect(signedIn.ok()).toBe(true);

  const state = await server.storageState();
  await server.dispose();

  mkdirSync(dirname(kept), { recursive: true });
  writeFileSync(
    kept,
    JSON.stringify({
      ...state,
      cookies: state.cookies.map((cookie) => ({ ...cookie, secure: false })),
    }),
  );
}
