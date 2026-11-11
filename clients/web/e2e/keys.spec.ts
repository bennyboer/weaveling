import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_APP_CODE,
  THE_CODE,
  THE_RECOVERY_CODES,
  aServerWithAccounts,
  signIn,
  type Server,
  confirmedByHerPassword,
} from "./support/accounts";

test.use({ baseURL: "http://localhost:3100" });

async function aSecurityKey(page: Page) {
  const devtools = await page.context().newCDPSession(page);
  await devtools.send("WebAuthn.enable");
  const { authenticatorId } = await devtools.send(
    "WebAuthn.addVirtualAuthenticator",
    {
      options: {
        protocol: "ctap2",
        transport: "usb",
        hasResidentKey: true,
        hasUserVerification: true,
        isUserVerified: true,
        automaticPresenceSimulation: true,
      },
    },
  );

  return async () => {
    await devtools.send("WebAuthn.removeVirtualAuthenticator", {
      authenticatorId,
    });
  };
}

async function onTheSecurityPage(
  page: Page,
  { keysOffered = true, totpOffered = true } = {},
): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.keysOffered = keysOffered;
  server.totpOffered = totpOffered;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await page.goto("/account/security");
  await expect(page.getByRole("heading", { name: "Security" })).toBeVisible();

  return server;
}

const ways = (page: Page) => page.getByRole("main").getByRole("list").first();

function aKeyHeld(server: Server, name: string) {
  server.keys.push({
    key: `key_${name.replaceAll(" ", "")}`,
    name,
    role: "second-factor",
    since: "2026-11-07T10:00:00Z",
    credential: "AAAA",
  });
  server.secondFactor = {
    since: "2026-11-07T10:00:00Z",
    recovery_codes_left: 10,
    answered_by_totp: false,
    answered_by_key: true,
  };
}

test("a security key made by the browser is added under its name, the first second step with recovery codes", async ({
  page,
}) => {
  // given: an author on their security page, with a security key at hand
  const server = await onTheSecurityPage(page);
  await aSecurityKey(page);

  // when: they add a key, and name it once the browser has made it
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they are told it is added and the second step is on
  await expect(
    page.getByText(
      "The key is added, and signing in now asks for a second step.",
    ),
  ).toBeVisible();
  // and: the recovery codes are shown, to be kept before going on
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(
    THE_RECOVERY_CODES,
  );
  await page.getByLabel("I have kept them somewhere safe").check();
  await page.getByRole("button", { name: "Done" }).click();
  // and: the key is listed by its name, beside the recovery codes
  await expect(ways(page)).toContainText("Security key “YubiKey blue”");
  await expect(ways(page)).toContainText("10 of 10 left");
  expect(server.keys.map((key) => key.name)).toEqual(["YubiKey blue"]);
});

test("a key beside a second step is added without new codes", async ({
  page,
}) => {
  // given: an author with a key already, and another at hand
  const server = await aServerWithAccounts(page);
  server.keysOffered = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  aKeyHeld(server, "YubiKey blue");
  await page.goto("/account/security");
  await aSecurityKey(page);

  // when: they add the second key
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey green");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: it is listed beside the first, and no codes are shown
  await expect(
    page.getByText("The key is added. You were signed out everywhere else."),
  ).toBeVisible();
  await expect(ways(page)).toContainText("Security key “YubiKey green”");
  await expect(ways(page)).toContainText("Security key “YubiKey blue”");
  await expect(
    page.getByRole("heading", { name: "Keep these recovery codes" }),
  ).toHaveCount(0);
});

test("a name another key has is refused, and the key is added under another", async ({
  page,
}) => {
  // given: an author with a key named, adding another
  const server = await aServerWithAccounts(page);
  server.keysOffered = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  aKeyHeld(server, "YubiKey blue");
  await page.goto("/account/security");
  await aSecurityKey(page);
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });

  // when: they give it the first one's name
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they are told, and the dialog stays
  await expect(
    naming.getByText("Another of your keys has this name already."),
  ).toBeVisible();
  // when: they give it another name
  await naming.getByLabel("Name").fill("YubiKey green");
  await naming.getByRole("button", { name: "Add the key" }).click();
  // then: it is added
  await expect(ways(page)).toContainText("Security key “YubiKey green”");
  expect(server.keys).toHaveLength(2);
});

test("a key is renamed, and removed for the password", async ({ page }) => {
  // given: an author with a key, on their security page
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  aKeyHeld(server, "YubiKey blue");
  await page.goto("/account/security");
  const actions = () =>
    page.getByRole("button", { name: /^Actions for / }).first();

  // when: they rename it
  await actions().click();
  await page.getByRole("menuitem", { name: "Rename" }).click();
  const renaming = page.getByRole("dialog", { name: "Rename “YubiKey blue”" });
  await renaming.getByLabel("Name").fill("Keyring");
  await renaming.getByRole("button", { name: "Save" }).click();
  // then: it is listed under the new name
  await expect(ways(page)).toContainText("Security key “Keyring”");
  // when: they remove it with a wrong password
  await actions().click();
  await page.getByRole("menuitem", { name: "Remove" }).click();
  const removing = page.getByRole("dialog", { name: "Remove “Keyring”?" });
  await removing
    .getByLabel("Your password, to be sure it’s you")
    .fill("not her password");
  await removing.getByRole("button", { name: "Remove" }).click();
  // then: they are told, and the key stays
  await expect(removing.getByText("That is not your password.")).toBeVisible();
  expect(server.keys).toHaveLength(1);
  // when: they give their password
  await removing
    .getByLabel("Your password, to be sure it’s you")
    .fill(HER_PASSWORD);
  await removing.getByRole("button", { name: "Remove" }).click();
  // then: it is gone, and so is the second step
  await expect(
    page.getByText("The key is removed. You were signed out everywhere else."),
  ).toBeVisible();
  await expect(ways(page)).not.toContainText("Keyring");
  expect(server.keys).toHaveLength(0);
});

test("a key the browser does not make changes nothing", async ({ page }) => {
  // given: an author with no key at hand, whose browser gives up asking after a second
  const server = await onTheSecurityPage(page);
  server.keyAskedFor = 1000;
  const devtools = await page.context().newCDPSession(page);
  await devtools.send("WebAuthn.enable");

  // when: they try to add it
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);

  // then: they are told no key was made, and none is listed
  await expect(page.getByText("No key was made.")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(server.keys).toHaveLength(0);
});

test("an instance that cannot add keys offers none", async ({ page }) => {
  // given: an author on an instance not told where it is served
  await onTheSecurityPage(page, { keysOffered: false });

  // then: no key can be added, an authenticator app still can
  await expect(
    page.getByRole("button", { name: "Add a security key" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Set up an authenticator app" }),
  ).toBeVisible();
});

const accountButton = (page: Page) =>
  page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" });

async function withAKeyAdded(page: Page) {
  const server = await onTheSecurityPage(page);
  const lost = await aSecurityKey(page);
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();
  await page.getByLabel("I have kept them somewhere safe").check();
  await page.getByRole("button", { name: "Done" }).click();
  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await signIn(page, ADA.email, HER_PASSWORD);

  return { server, lost };
}

test("a key added answers the second step when signing in again, once the author asks for it", async ({
  page,
}) => {
  // given: an author who added a key, signing in again with their password
  await withAKeyAdded(page);

  // then: the key is asked for, the browser's dialog waiting for a click
  await expect(page.getByText("Touch your security key.")).toBeVisible();
  // when: they use the key
  await page.getByRole("button", { name: "Use the key" }).click();
  // then: they are signed in
  await expect(accountButton(page)).toBeVisible();
});

test("a key not touched leaves the second step to try again, or another way", async ({
  page,
}) => {
  // given: an author owing the second step, their key no longer at hand
  const { server, lost } = await withAKeyAdded(page);
  await lost();
  server.keyAskedFor = 1000;

  // when: they try to use the key
  await page.getByRole("button", { name: "Use the key" }).click();

  // then: they are told, and still asked for the second step
  await expect(
    page.getByText("No key was touched. Try once more, or use another way."),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Use another way" }),
  ).toBeVisible();
  await expect(accountButton(page)).toHaveCount(0);
});

test("with a key and an app, the key is asked first and another way is a click away", async ({
  page,
}) => {
  // given: an author whose second step is answered by a key and an app, signing in
  const server = await aServerWithAccounts(page);
  server.totpOffered = true;
  aKeyHeld(server, "YubiKey blue");
  server.secondFactor!.answered_by_totp = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: the key is asked for first
  await expect(page.getByText("Touch your security key.")).toBeVisible();
  // when: they look for another way
  await page.getByRole("button", { name: "Use another way" }).click();
  // then: every way is offered, the key told as asked first, recovery codes last
  const ways = page.getByRole("main").getByRole("listitem");
  await expect(ways).toHaveCount(3);
  await expect(ways.nth(0)).toContainText("Security key");
  await expect(ways.nth(0)).toContainText("Asked first right now");
  await expect(ways.nth(1)).toContainText("Authenticator app");
  await expect(ways.nth(2)).toContainText("Recovery code");
  // when: they choose the app
  await page.getByRole("button", { name: /^Authenticator app/ }).click();
  // then: its code is asked for, and to use it first from now on is offered
  await expect(
    page.getByLabel("The code your app shows for Weaveling"),
  ).toBeVisible();
  await expect(page.getByLabel("Use this way first from now on")).toBeVisible();
});

test("a way used once is asked first next time, and one kept as first stays first", async ({
  page,
}) => {
  // given: an author whose second step a key and the app answer, signing in, the key asked first
  const server = await aServerWithAccounts(page);
  server.totpOffered = true;
  aKeyHeld(server, "YubiKey blue");
  server.secondFactor!.answered_by_totp = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: they answer by the app, asking to use it first from now on
  await page.getByRole("button", { name: "Use another way" }).click();
  await page.getByRole("button", { name: /^Authenticator app/ }).click();
  await page.getByLabel("Use this way first from now on").check();
  await page
    .getByLabel("The code your app shows for Weaveling")
    .fill(THE_APP_CODE);

  // then: they are signed in, and the app is kept as first
  await expect(accountButton(page)).toBeVisible();
  await expect.poll(() => server.preferredWay).toBe("totp");
  // when: they sign in again
  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await signIn(page, ADA.email, HER_PASSWORD);
  // then: the app's code is asked first
  await expect(
    page.getByLabel("The code your app shows for Weaveling"),
  ).toBeVisible();
});

test("the security page offers which way is asked first, once more than one answers", async ({
  page,
}) => {
  // given: an author whose second step a key and the app answer, on their security page
  const server = await aServerWithAccounts(page);
  server.totpOffered = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  aKeyHeld(server, "YubiKey blue");
  server.secondFactor!.answered_by_totp = true;
  await page.goto("/account/security");
  const first = page.getByRole("group", { name: "Ask first for" });

  // then: the one used last is asked first
  await expect(first.getByLabel(/^The one I used last/)).toBeChecked();
  // when: they choose the app
  await first.getByLabel(/^The authenticator app/).check();
  // then: it is kept
  await expect.poll(() => server.preferredWay).toBe("totp");
  await page.reload();
  await expect(
    page
      .getByRole("group", { name: "Ask first for" })
      .getByLabel(/^The authenticator app/),
  ).toBeChecked();
});

test("the recovery codes are shown even when the renewed session's cookie comes after its socket was told the old one ended", async ({
  page,
}) => {
  // given: an author following live on their security page, the server's renewal lagging behind its sockets
  const server = await onTheSecurityPage(page);
  await expect.poll(() => server.liveSockets.length).toBeGreaterThan(0);
  server.renewalLags = 500;
  await aSecurityKey(page);

  // when: they add their first key
  await page.getByRole("button", { name: "Add a security key" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they stay signed in and are shown the recovery codes
  await expect(
    page.getByRole("heading", { name: "Keep these recovery codes" }),
  ).toBeVisible();
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(
    THE_RECOVERY_CODES,
  );
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
});

test("turning the app off beside a key keeps the recovery codes", async ({
  page,
}) => {
  // given: an author whose second step a key and the app answer, on their security page
  const server = await aServerWithAccounts(page);
  server.totpOffered = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  aKeyHeld(server, "YubiKey blue");
  server.secondFactor!.answered_by_totp = true;
  await page.goto("/account/security");

  // when: they turn the app off with their password
  await page
    .getByRole("button", { name: "Actions for the authenticator app" })
    .click();
  await page.getByRole("menuitem", { name: "Turn off" }).click();
  const asking = page.getByRole("main").locator("form");
  await asking
    .getByLabel("Your password, to be sure it’s you")
    .fill(HER_PASSWORD);
  await asking.getByRole("button", { name: "Turn off" }).click();

  // then: the app is gone, while the key and the recovery codes stay
  await expect(ways(page)).not.toContainText("Authenticator app");
  await expect(ways(page)).toContainText("Security key “YubiKey blue”");
  await expect(ways(page)).toContainText("10 of 10 left");
});

test("a passkey is added to sign in alone, listed apart from the second step, with no codes", async ({
  page,
}) => {
  // given: an author on their security page, with a key at hand
  const server = await onTheSecurityPage(page);
  await aSecurityKey(page);

  // when: they add a passkey and name it
  await page.getByRole("button", { name: "Add a passkey" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("MacBook");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: it is listed under signing in, as a passkey
  await expect(page.getByText("The key is added.")).toBeVisible();
  const signingIn = page.getByRole("main").getByRole("list").first();
  await expect(signingIn).toContainText("Passkey “MacBook”");
  // and: no recovery codes are shown, the second step still off
  await expect(
    page.getByRole("heading", { name: "Keep these recovery codes" }),
  ).toHaveCount(0);
  expect(server.keys.map((key) => key.role)).toEqual(["primary"]);
  expect(server.secondFactor).toBeNull();
});

test("a passkey used as the second step comes with recovery codes, and is made a passkey again in a guided way", async ({
  page,
}) => {
  // given: an author with a passkey, on their security page
  const server = await onTheSecurityPage(page);
  await aSecurityKey(page);
  await page.getByRole("button", { name: "Add a passkey" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("MacBook");
  await naming.getByRole("button", { name: "Add the key" }).click();
  await expect(page.getByText("The key is added.")).toBeVisible();

  // when: they use it as the second step, with their password
  await page.getByRole("button", { name: "Actions for MacBook" }).click();
  await page.getByRole("menuitem", { name: "Use as second step" }).click();
  const demoting = page.getByRole("dialog", {
    name: "Use “MacBook” as a second step",
  });
  await demoting
    .getByLabel("Your password, to be sure it’s you")
    .fill(HER_PASSWORD);
  await demoting.getByRole("button", { name: "Use as second step" }).click();

  // then: the recovery codes are shown, it being the first second step
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(
    THE_RECOVERY_CODES,
  );
  await page.getByLabel("I have kept them somewhere safe").check();
  await page.getByRole("button", { name: "Done" }).click();
  await expect(ways(page)).toContainText("Security key “MacBook”");
  // when: they make it a passkey again, with their password and a touch
  await page.getByRole("button", { name: "Actions for MacBook" }).click();
  await page.getByRole("menuitem", { name: "Use as passkey" }).click();
  const promoting = page.getByRole("dialog", {
    name: "Make “MacBook” a passkey",
  });
  await promoting
    .getByLabel("Your password, to be sure it’s you")
    .fill(HER_PASSWORD);
  await promoting.getByRole("button", { name: "Make it a passkey" }).click();

  // then: it is a passkey again, and the second step is off
  await expect(
    page.getByText("The key is a passkey now and signs you in alone."),
  ).toBeVisible();
  await expect(page.getByRole("main").getByRole("list").first()).toContainText(
    "Passkey “MacBook”",
  );
  expect(server.keys.map((key) => key.role)).toEqual(["primary"]);
  expect(server.secondFactor).toBeNull();
});

async function withAPasskeyAdded(page: Page) {
  const server = await onTheSecurityPage(page);
  const lost = await aSecurityKey(page);
  await page.getByRole("button", { name: "Add a passkey" }).click();

  await confirmedByHerPassword(page);
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("MacBook");
  await naming.getByRole("button", { name: "Add the key" }).click();
  await expect(page.getByText("The key is added.")).toBeVisible();
  await accountButton(page).click();
  await page.getByRole("menuitem", { name: "Sign out" }).click();
  await page.goto("/");

  return { server, lost };
}

test("a passkey signs in alone, with no password and no second step", async ({
  page,
}) => {
  // given: an author who added a passkey and signed out
  await withAPasskeyAdded(page);

  // when: they sign in with the passkey
  await page.getByRole("button", { name: "Sign in with a passkey" }).click();

  // then: they are signed in, nothing more asked
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("a passkey not used leaves signing in by password", async ({ page }) => {
  // given: an author who added a passkey and signed out, the passkey no longer at hand
  const { server, lost } = await withAPasskeyAdded(page);
  await lost();
  server.keyAskedFor = 1000;

  // when: they try to sign in with a passkey
  await page.getByRole("button", { name: "Sign in with a passkey" }).click();

  // then: they are told no passkey was used
  await expect(
    page.getByText(
      "No passkey was used. Try once more, or sign in with your password.",
    ),
  ).toBeVisible();
  // when: they sign in with their password instead
  await signIn(page, ADA.email, HER_PASSWORD);
  // then: they are signed in
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("signing in with a passkey is not offered where this Weaveling instance knows no keys", async ({
  page,
}) => {
  // given: a server offering no keys
  const server = await aServerWithAccounts(page);

  // when: the sign-in page is shown
  await page.goto("/");

  // then: only the password signs in
  await expect(page.getByLabel("Password")).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Sign in with a passkey" }),
  ).toHaveCount(0);
  // and: no passkey is asked for in the address field
  expect(server.passkeysAskedFor).toBe(0);
});

test("the browser's passkeys are offered where it fills in the address, and the button still asks once pressed", async ({
  page,
}) => {
  // given: an author who added a passkey and signed out
  const { server } = await withAPasskeyAdded(page);

  // then: the address field offers the browser's passkeys, asked for once
  await expect(page.getByLabel("Email address")).toHaveAttribute(
    "autocomplete",
    "username webauthn",
  );
  await expect.poll(() => server.passkeysAskedFor).toBe(1);
  // when: they press the button nonetheless
  await page.getByRole("button", { name: "Sign in with a passkey" }).click();
  // then: they are signed in by the passkey
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
  expect(server.passkeysAskedFor).toBe(2);
});

test("one who just signed up is offered a passkey or a second step, which can wait", async ({
  page,
}) => {
  // given: a server offering keys, and an author who just confirmed their address
  const server = await aServerWithAccounts(page);
  server.keysOffered = true;
  server.verified = false;
  await page.goto("/");
  await page.getByRole("button", { name: "No account yet? Sign up" }).click();
  await page.getByLabel("Your name").fill(ADA.name);
  await page.getByLabel("Email address").fill(ADA.email);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up", exact: true }).click();
  await page.getByLabel("Code").fill(THE_CODE);
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);

  // when: they sign in for the first time
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();

  // then: the security page welcomes them, offering a passkey
  await expect(page.getByRole("heading", { name: "Security" })).toBeVisible();
  await expect(
    page.getByText("Your account is ready.", { exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Add a passkey" }),
  ).toBeVisible();
  // when: they leave it for later
  await page
    .getByRole("link", { name: "Not now, take me to my projects" })
    .click();
  // then: their projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});
