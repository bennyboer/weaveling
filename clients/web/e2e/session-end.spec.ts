import { test, expect } from "@playwright/test";

import { ADA, HER_PASSWORD, aServerWithAccounts, sessionEndedOn, signIn } from "./support/accounts";

const A_BEAT = 500;

test("a session ending while the page is open signs it out, and the socket is not opened again", async ({
  page,
}) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect(page.getByRole("button", { name: "Notifications" })).toBeVisible();
  await expect.poll(() => server.liveSockets.length).toBe(1);
  let openedAgain = 0;
  page.on("websocket", (socket) => {
    if (socket.url().endsWith("/api/live")) {
      openedAgain += 1;
    }
  });

  sessionEndedOn(server);

  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();
  await page.waitForTimeout(A_BEAT);
  expect(openedAgain, "an ended session is not knocked on again").toBe(0);
});

test("signing in again after the session ended follows on one socket again", async ({ page }) => {
  const server = await aServerWithAccounts(page);
  await page.goto("/");
  await signIn(page, ADA.email, HER_PASSWORD);
  await expect.poll(() => server.liveSockets.length).toBe(1);
  sessionEndedOn(server);
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  await signIn(page, ADA.email, HER_PASSWORD);

  await expect(page.getByRole("button", { name: "Notifications" })).toBeVisible();
  await expect
    .poll(() => server.subscribed.filter((subscribed) => subscribed.topic === "invitations").length)
    .toBe(1);
});
