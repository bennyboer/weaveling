import { test, expect, type Page } from "@playwright/test";

import {
  ADA,
  HER_PASSWORD,
  THE_CODE,
  aServerWithAccounts,
  signIn,
} from "./support/accounts";

async function signUp(page: Page, password = HER_PASSWORD) {
  await page.getByRole("button", { name: "No account yet? Sign up" }).click();
  await page.getByLabel("Your name").fill(ADA.name);
  await page.getByLabel("Email address").fill(ADA.email);
  await page.getByLabel("Password").fill(password);
  await page.getByRole("button", { name: "Sign up", exact: true }).click();
}

async function enterCode(page: Page, code: string) {
  await page.getByLabel("Code").fill(code);
}

const boxes = (page: Page) => page.locator(".code-box");

test("signing up asks for the code sent to the address", async ({ page }) => {
  // given: a server with accounts
  const server = await aServerWithAccounts(page);
  await page.goto("/");

  // when: an author signs up
  await signUp(page);

  // then: she is asked for the code sent to her address
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
  await expect(page.getByText(`We sent a code to ${ADA.email}.`)).toBeVisible();
  // and: she is signed up
  expect(server.signedUp).toEqual({ email: ADA.email, name: ADA.name });
});

test("the right code confirms the address and leads back to signing in", async ({
  page,
}) => {
  // given: an author signed up
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she enters the right code
  await enterCode(page, THE_CODE);

  // then: her address is confirmed
  await expect(page.getByRole("status")).toHaveText(
    "Your address is confirmed. You can sign in now.",
  );
  // and: signing in is offered with her address filled in
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  // when: she signs in
  await page.getByLabel("Password").fill(HER_PASSWORD);
  await page.getByRole("button", { name: "Sign in", exact: true }).click();
  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("a wrong code says so and stays", async ({ page }) => {
  // given: an author signed up
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she enters a wrong code
  await enterCode(page, "000000");

  // then: she is told it is wrong
  await expect(page.getByRole("alert")).toHaveText(
    "That is not the code we sent.",
  );
  // and: she is still asked to confirm her address
  await expect(
    page.getByRole("heading", { name: "Confirm your address" }),
  ).toBeVisible();
});

test("a new code can be asked for", async ({ page }) => {
  // given: an author signed up
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she asks for a new code
  await page.getByRole("button", { name: "Send a new code" }).click();

  // then: she is told it is on its way
  await expect(page.getByRole("status")).toHaveText(
    "A new code is on its way.",
  );
  // and: another code is sent
  expect(server.codesSent).toBe(2);
});

test("a password too short is refused with its numbers", async ({ page }) => {
  // given: a server with accounts
  await aServerWithAccounts(page);
  await page.goto("/");

  // when: an author signs up with a short password
  await signUp(page, "short");

  // then: she is told how short it is against how long it must be
  await expect(page.getByRole("alert")).toHaveText(
    "A password needs at least 12 characters; this one has 5.",
  );
  // and: she stays on signing up
  await expect(page.getByRole("heading", { name: "Sign up" })).toBeVisible();
});

test("an address not yet confirmed can go on to enter its code", async ({
  page,
}) => {
  // given: an author signed in whose address is not confirmed
  await aServerWithAccounts(page, { verified: false });
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);

  // when: she goes on to enter her code
  await page.getByRole("button", { name: "Enter the code" }).click();
  await enterCode(page, THE_CODE);
  // and: signs in again
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: the projects are shown
  await expect(
    page.getByRole("heading", { name: "Your projects" }),
  ).toBeVisible();
});

test("going between signing in and up keeps the address typed", async ({
  page,
}) => {
  // given: an address typed on signing in
  await aServerWithAccounts(page);
  await page.goto("/");
  await page.getByLabel("Email address").fill(ADA.email);

  // when: signing up is opened
  await page.getByRole("button", { name: "No account yet? Sign up" }).click();
  // then: the address is kept
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
  // when: signing in is opened again
  await page
    .getByRole("button", { name: "Already have an account? Sign in" })
    .click();

  // then: the address is kept
  await expect(page.getByLabel("Email address")).toHaveValue(ADA.email);
});

test("the code confirms itself once all six digits are in", async ({
  page,
}) => {
  // given: an author signed up
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she types all six digits
  await page.getByLabel("Code").pressSequentially(THE_CODE);

  // then: her address is confirmed
  await expect(page.getByRole("status")).toHaveText(
    "Your address is confirmed. You can sign in now.",
  );
  // and: the code was tried once
  expect(server.codesTried).toBe(1);
});

test("a code pasted with a space in it still counts", async ({ page }) => {
  // given: an author signed up
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she enters the code with a space in it
  await enterCode(page, "123 456");

  // then: her address is confirmed
  await expect(page.getByRole("status")).toHaveText(
    "Your address is confirmed. You can sign in now.",
  );
});

test("a wrong code is sent once, and the boxes empty for the next", async ({
  page,
}) => {
  // given: an author signed up
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she enters a wrong code
  await enterCode(page, "000000");

  // then: she is told it is wrong
  await expect(page.getByRole("alert")).toHaveText(
    "That is not the code we sent.",
  );
  // and: the boxes are emptied
  await expect(page.getByLabel("Code")).toHaveValue("");
  // and: the code was tried once
  await page.waitForTimeout(300);
  expect(
    server.codesTried,
    "each try counts against five, so a code must not be sent again unasked",
  ).toBe(1);
});

test("each box shows its digit, and the next to fill stands out", async ({
  page,
}) => {
  // given: an author signed up
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // when: she types two digits
  await page.getByLabel("Code").pressSequentially("12");

  // then: each box shows its digit
  await expect(boxes(page)).toHaveCount(6);
  await expect(boxes(page).nth(0)).toHaveText("1");
  await expect(boxes(page).nth(1)).toHaveText("2");
  await expect(boxes(page).nth(2)).toHaveText("");
  // and: the next to fill stands out
  await expect(boxes(page).nth(2)).toHaveClass(/next/);
  await expect(boxes(page).nth(1)).not.toHaveClass(/next/);
});

test("the code field offers what the phone read from the mail", async ({
  page,
}) => {
  // given: an author signed up
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);

  // then: the code field offers a one-time code read from the mail
  await expect(page.getByLabel("Code")).toHaveAttribute(
    "autocomplete",
    "one-time-code",
  );
  // and: asks for digits
  await expect(page.getByLabel("Code")).toHaveAttribute("inputmode", "numeric");
});

test("a new code asked for starts with empty boxes", async ({ page }) => {
  // given: an author signed up, with some digits typed
  await aServerWithAccounts(page);
  await page.goto("/");
  await signUp(page);
  await page.getByLabel("Code").pressSequentially("123");

  // when: she asks for a new code
  await page.getByRole("button", { name: "Send a new code" }).click();

  // then: she is told it is on its way
  await expect(page.getByRole("status")).toHaveText(
    "A new code is on its way.",
  );
  // and: the boxes are empty
  await expect(page.getByLabel("Code")).toHaveValue("");
});
