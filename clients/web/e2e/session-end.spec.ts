import { test, expect } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, sessionEndedOn, signIn } from "./support/accounts";

const A_BEAT = 500;

test("a session ending while the page is open signs it out, and the socket is not opened again", async ({
  page,
}) => {
  // given: an author signed in and following live
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("button", { name: "Notifications" })).toBeVisible();
  await expect.poll(() => server.liveSockets.length).toBe(1);
  // and: every new live connection counted
  let openedAgain = 0;
  page.on("websocket", (socket) => {
    if (socket.url().endsWith("/api/live")) {
      openedAgain += 1;
    }
  });

  // when: her session ends
  sessionEndedOn(server);

  // then: she is asked to sign in
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  // and: nothing follows live for her any more
  await page.waitForTimeout(A_BEAT);
  expect(openedAgain, "an ended session is not knocked on again").toBe(0);
});

test("signing in again after the session ended follows on one socket again", async ({ page }) => {
  // given: an author whose session ended while she was signed in
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect.poll(() => server.liveSockets.length).toBe(1);
  sessionEndedOn(server);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  // when: she signs in again
  await signIn(page, ADA.email, HER_PASSWORD);

  // then: she is signed in
  await expect(page.getByRole("button", { name: "Notifications" })).toBeVisible();
  // and: she follows her invitations once
  await expect
    .poll(() => server.subscribed.filter((subscribed) => subscribed.topic === "invitations").length)
    .toBe(1);
});
