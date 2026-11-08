import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_RECOVERY_CODES,
  aServerWithAccounts,
  signIn,
  type Server,
} from "./support/accounts";

test.use({ baseURL: "http://localhost:8080" });

async function aSecurityKey(page: Page) {
  const devtools = await page.context().newCDPSession(page);
  await devtools.send("WebAuthn.enable");
  const { authenticatorId } = await devtools.send("WebAuthn.addVirtualAuthenticator", {
    options: {
      protocol: "ctap2",
      transport: "usb",
      hasResidentKey: true,
      hasUserVerification: true,
      isUserVerified: true,
      automaticPresenceSimulation: true,
    },
  });

  return async () => {
    await devtools.send("WebAuthn.removeVirtualAuthenticator", { authenticatorId });
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
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they are told it is added and the second step is on
  await expect(page.getByText("The key is added, and signing in now asks for a second step.")).toBeVisible();
  // and: the recovery codes are shown, to be kept before going on
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(THE_RECOVERY_CODES);
  await page.getByLabel("I have kept them somewhere safe").check();
  await page.getByRole("button", { name: "Done" }).click();
  // and: the key is listed by its name, beside the recovery codes
  await expect(ways(page)).toContainText("Security key “YubiKey blue”");
  await expect(ways(page)).toContainText("10 of 10 left");
  expect(server.keys.map((key) => key.name)).toEqual(["YubiKey blue"]);
});

test("a key beside a second step is added without new codes", async ({ page }) => {
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
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey green");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: it is listed beside the first, and no codes are shown
  await expect(page.getByText("The key is added. You were signed out everywhere else.")).toBeVisible();
  await expect(ways(page)).toContainText("Security key “YubiKey green”");
  await expect(ways(page)).toContainText("Security key “YubiKey blue”");
  await expect(page.getByRole("heading", { name: "Keep these recovery codes" })).toHaveCount(0);
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
  const naming = page.getByRole("dialog", { name: "Your key is made" });

  // when: they give it the first one's name
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they are told, and the dialog stays
  await expect(naming.getByText("Another of your keys has this name already.")).toBeVisible();
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
  const actions = () => page.getByRole("button", { name: /^Actions for / }).first();

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
  await removing.getByLabel("Your password, to be sure it’s you").fill("not her password");
  await removing.getByRole("button", { name: "Remove" }).click();
  // then: they are told, and the key stays
  await expect(removing.getByText("That is not your password.")).toBeVisible();
  expect(server.keys).toHaveLength(1);
  // when: they give their password
  await removing.getByLabel("Your password, to be sure it’s you").fill(HER_PASSWORD);
  await removing.getByRole("button", { name: "Remove" }).click();
  // then: it is gone, and so is the second step
  await expect(page.getByText("The key is removed. You were signed out everywhere else.")).toBeVisible();
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

  // then: they are told no key was made, and none is listed
  await expect(page.getByText("No key was made.")).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
  expect(server.keys).toHaveLength(0);
});

test("an instance that cannot add keys offers none", async ({ page }) => {
  // given: an author on an instance not told where it is served
  await onTheSecurityPage(page, { keysOffered: false });

  // then: no key can be added, an authenticator app still can
  await expect(page.getByRole("button", { name: "Add a security key" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Set up an authenticator app" })).toBeVisible();
});

const accountButton = (page: Page) =>
  page.getByRole("banner").getByRole("button", { name: "Account: Ada Lovelace" });

async function withAKeyAdded(page: Page) {
  const server = await onTheSecurityPage(page);
  const lost = await aSecurityKey(page);
  await page.getByRole("button", { name: "Add a security key" }).click();
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
  await expect(page.getByRole("button", { name: "Use the code from your app instead" })).toHaveCount(0);
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
  await expect(page.getByText("No key was touched. Try once more, or use another way.")).toBeVisible();
  await expect(page.getByRole("button", { name: "Use a recovery code instead" })).toBeVisible();
  await expect(accountButton(page)).toHaveCount(0);
});

test("with a key and an app, the key is asked first and the app's code is a click away", async ({
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
  // when: they turn to the app's code
  await page.getByRole("button", { name: "Use the code from your app instead" }).click();
  // then: the code is asked for, the key a click away again
  await expect(page.getByLabel("The code your app shows for Weaveling")).toBeVisible();
  await page.getByRole("button", { name: "Use a security key instead" }).click();
  await expect(page.getByText("Touch your security key.")).toBeVisible();
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
  const naming = page.getByRole("dialog", { name: "Your key is made" });
  await naming.getByLabel("Name").fill("YubiKey blue");
  await naming.getByRole("button", { name: "Add the key" }).click();

  // then: they stay signed in and are shown the recovery codes
  await expect(page.getByRole("heading", { name: "Keep these recovery codes" })).toBeVisible();
  await expect(page.getByRole("main").getByRole("listitem")).toHaveText(THE_RECOVERY_CODES);
  await expect(page.getByRole("heading", { name: "Sign in" })).toHaveCount(0);
});
