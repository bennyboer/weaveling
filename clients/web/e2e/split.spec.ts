import { test, expect } from "@playwright/test";

import { aNewProject, openTheOutline } from "./support/shell";

test("a view's code is fetched only once the view is opened", async ({
  page,
}) => {
  // given: the browser's fetches of WebAssembly watched
  const fetched: string[] = [];
  page.on("request", (request) => {
    if (request.url().endsWith(".wasm")) {
      fetched.push(new URL(request.url()).pathname);
    }
  });

  // when: a project is made and opened, not yet its outline
  await aNewProject(page, "Splitting");

  // then: the outline's code has not been fetched
  expect(fetched.some((path) => path.includes("outline"))).toBe(false);
  // when: the outline is opened
  await openTheOutline(page);
  // then: its code is fetched, and the outline shown
  await expect
    .poll(() => fetched.filter((path) => path.includes("outline")))
    .toHaveLength(1);
  await expect(
    page.getByText("Nothing in the book yet. Add a section to begin."),
  ).toBeVisible();
});
