import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_CODE,
  THE_OPERATOR_CODE,
  aServerWithAccounts,
  signIn,
  type Server,
} from "./support/accounts";

async function aFreshInstanceByInvitation(page: Page): Promise<Server> {
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";
  server.operatorWanted = true;
  await page.goto("/");

  return server;
}

async function setUp(page: Page, code: string) {
  await page.goto("/setup");
  await page.getByLabel("Code from the server's log").fill(code);
  await page.getByLabel("Your name").fill(ADA.name);
  await page.getByLabel("Email address").fill(ADA.email);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign up", exact: true }).click();
}

test("the first account signs up with the logged code and, confirmed and signed in, runs the instance", async ({
  page,
}) => {
  // given: a fresh instance where accounts come by invitation
  const server = await aFreshInstanceByInvitation(page);

  // when: the first account signs up with the logged code
  await setUp(page, ` ${THE_OPERATOR_CODE} `);
  // and: confirms its address and signs in
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await page.getByLabel("Code", { exact: true }).fill(THE_CODE);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();

  // then: it is shown how to manage the instance
  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  await expect(page).toHaveURL(/\/instance\/accounts$/);
  // and: it runs the instance
  expect(server.operator).toBe(true);
  expect(server.signedUp).toEqual({ email: ADA.email, name: ADA.name });
});

test("a code that is not the logged one is refused in place", async ({
  page,
}) => {
  // given: a fresh instance where accounts come by invitation
  const server = await aFreshInstanceByInvitation(page);

  // when: someone signs up with another code
  await setUp(page, "0123456789abcdef0123456789abcdef");

  // then: the code is refused in place
  await expect(page.getByRole("alert")).toContainText(
    "This is not the code the server logged.",
  );
  // and: nobody is signed up
  expect(server.signedUp).toBeNull();
});

test("once an instance has an operator, nobody is offered to set it up", async ({
  page,
}) => {
  // given: an instance that has an operator
  const server = await aServerWithAccounts(page);
  server.signingUp = "invitation";

  // when: the page opens
  await page.goto("/");

  // then: signing in is offered
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Forgot your password?" }),
  ).toBeVisible();
  // and: setting up is not
  await expect(
    page.getByRole("button", { name: "Set up this Weaveling instance" }),
  ).toHaveCount(0);
});

test("while nobody runs the instance, its sign-in page does not tell so, and it is set up at its own address", async ({
  page,
}) => {
  // given: a fresh instance where anyone may sign up
  const server = await aServerWithAccounts(page);
  server.operatorWanted = true;
  await page.goto("/");

  // then: signing up is offered
  await expect(
    page.getByRole("button", { name: "No account yet? Sign up" }),
  ).toBeVisible();
  // and: setting up is not, which would tell anyone the instance is unclaimed
  await expect(
    page.getByRole("button", { name: "Set up this Weaveling instance" }),
  ).toHaveCount(0);
  // when: the first account sets it up with the logged code
  await setUp(page, THE_OPERATOR_CODE);
  // and: confirms its address and signs in
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await page.getByLabel("Code", { exact: true }).fill(THE_CODE);
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();

  // then: it runs the instance
  await expect(
    page.getByRole("heading", { name: "Manage instance" }),
  ).toBeVisible();
  expect(server.operator).toBe(true);
});

test("the word under the heading stands apart from the first field", async ({
  page,
}) => {
  // given: setting up a fresh instance
  await aFreshInstanceByInvitation(page);
  await page.goto("/setup");

  // when: the word under the heading and the first fields are measured
  const hint = await page
    .getByText("This Weaveling instance has no operator yet.")
    .boundingBox();
  const field = await page
    .locator(".field > span")
    .filter({ hasText: "Code from the server's log" })
    .boundingBox();
  const between = await page
    .locator(".field > span")
    .filter({ hasText: "Your name" })
    .boundingBox();
  const code = await page
    .getByLabel("Code from the server's log")
    .boundingBox();
  expect(hint && field && between && code).toBeTruthy();

  // then: the word stands as far from the first field as one field from the next
  expect(
    field!.y - (hint!.y + hint!.height),
    "as much room as between one field and the next",
  ).toBeGreaterThanOrEqual(between!.y - (code!.y + code!.height));
});

test("setting up has an address of its own", async ({ page }) => {
  // given: a fresh instance
  await aFreshInstanceByInvitation(page);

  // when: its setting up address is opened
  await page.goto("/setup");

  // then: setting up is offered
  await expect(
    page.getByRole("heading", { name: "Set up this Weaveling instance" }),
  ).toBeVisible();
  await expect(page.getByLabel("Code from the server's log")).toBeVisible();
});

test("signed in while nobody runs the instance, the menu leads to becoming its operator", async ({
  page,
}) => {
  // given: an author signed in while nobody runs the instance
  const server = await aServerWithAccounts(page);
  server.operatorWanted = true;
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: she chooses to set it up from her menu
  await page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" })
    .click();
  await page
    .getByRole("menuitem", { name: "Set up this Weaveling instance" })
    .click();

  // then: she is offered to become its operator
  await expect(page).toHaveURL(/\/setup$/);
  await expect(
    page.getByRole("heading", { name: "Become the operator" }),
  ).toBeVisible();
});

test("signed in where someone runs the instance, setting up says so", async ({
  page,
}) => {
  // given: an author signed in where someone runs the instance
  await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  // then: her menu offers no setting up
  await page
    .getByRole("banner")
    .getByRole("button", { name: "Account: Ada Lovelace" })
    .click();
  await expect(
    page.getByRole("menuitem", { name: "Set up this Weaveling instance" }),
  ).toHaveCount(0);

  // when: she opens the setting up address
  await page.goto("/setup");

  // then: she is told it is set up already
  await expect(page.getByRole("main")).toContainText(
    "This Weaveling instance is already set up",
  );
  await expect(page.getByLabel("Code")).toHaveCount(0);
});
