import { test, expect } from "@playwright/test";

import { aNewProject, openTheOutline } from "./support/shell";
import { openTheWritingView } from "./support/writing";

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

test("the editor's code is fetched only once the writing view is opened", async ({
  page,
}) => {
  // given: the browser's fetches of the editor watched
  const fetched: string[] = [];
  page.on("request", (request) => {
    const path = new URL(request.url()).pathname;
    if (path.startsWith("/js/editor")) {
      fetched.push(path);
    }
  });

  // when: a project is made and opened on its board
  await aNewProject(page, "Editing");

  // then: the editor has not been fetched
  expect(fetched).toEqual([]);
  // when: the writing view is opened
  await openTheWritingView(page);
  // then: the editor is fetched, once
  expect(fetched).toHaveLength(1);
});
