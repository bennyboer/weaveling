import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import { aNewProject, API } from "./support/shell";
import { flow } from "./support/writing";

const spoken = (page: Page) =>
  page.evaluate(() => document.documentElement.getAttribute("lang"));

const picker = (page: Page, named: string) =>
  page.getByRole("combobox", { name: named });

test.describe("a browser asking for German", () => {
  test.use({ locale: "de-DE" });

  test("at first the app speaks the browser's language", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { level: 1 })).toHaveText(
      "Deine Projekte",
    );
    expect(await spoken(page)).toBe("de");
    await expect(picker(page, "Sprache")).toHaveValue("de");
  });

  test("inside a project the masthead speaks German too", async ({ page }) => {
    await aNewProject(page, "Ansichten");

    const views = page.getByRole("navigation", { name: "Ansichten" });
    await expect(views.getByRole("link")).toHaveText([
      "Schreiben",
      "Pinnwand",
      "Gliederung",
      "Ideen",
    ]);
    await expect(
      page
        .getByRole("group", { name: "Farbschema" })
        .getByRole("button", { name: "Wie das System" }),
    ).toBeVisible();
    await expect(page.getByRole("contentinfo", { name: "Status" })).toBeVisible();
    await expect(page.getByRole("link", { name: "Alle Projekte" })).toBeVisible();
  });

  test("deleting a project asks in German, the name in German quotation marks", async ({
    page,
  }) => {
    const title = await aNewProject(page, "Löschen");
    await page.goto("/");

    await page
      .getByRole("listitem")
      .filter({ hasText: title })
      .getByRole("button", { name: "Weitere Aktionen" })
      .click();
    await page.getByRole("menuitem", { name: "Löschen" }).click();

    const asked = page.getByRole("dialog", { name: "Dieses Projekt löschen?" });
    await expect(asked).toContainText(
      `„${title}“ ist dann für immer weg. Löschen lässt sich nicht rückgängig machen.`,
    );
    await asked.getByRole("button", { name: "Abbrechen" }).click();
    await expect(asked).toHaveCount(0);
  });
});

test.describe("a browser asking for German, about a project's details", () => {
  test.use({ locale: "de-DE" });

  test("a new project's details are asked in German, the book's languages named in German", async ({
    page,
  }) => {
    await page.goto("/");
    await page.getByPlaceholder("Ein Arbeitstitel…").fill("Der Webstuhl");
    await page.getByRole("button", { name: "Anlegen", exact: true }).click();

    const asked = page.getByRole("dialog", { name: "Neues Projekt" });
    await expect(asked.getByLabel("Titel")).toHaveValue("Der Webstuhl");
    const language = asked.getByLabel("Sprache des Buchs");
    await expect(language).toHaveValue("de");
    await expect(language.getByRole("option", { name: "Deutsch", exact: true })).toHaveCount(1);
    await expect(
      language.getByRole("option", { name: "Englisch (English)" }),
    ).toHaveCount(1);
    await expect(
      asked.getByLabel("Anführungszeichen").getByRole("option").first(),
    ).toHaveText("Wie in der Sprache üblich, »…«");
    await expect(asked.getByLabel("Beschreibung")).toHaveAttribute(
      "placeholder",
      "Worum es im Buch geht, in ein paar Zeilen. Optional.",
    );
    await asked.getByRole("button", { name: "Abbrechen" }).click();
    await expect(asked).toHaveCount(0);
  });

  test("rewriting the quotation marks is asked and followed in German, one scene told as one", async ({
    page,
  }) => {
    await aNewProject(page, "Zeichen");
    await aBookWithScenes(page, { "Part One": 1 });
    let asked = 0;
    await page.route("**/api/scenes/quotation-restylings/**", async (route) => {
      asked += 1;
      if (asked > 1) {
        await route.continue();
        return;
      }
      await route.fulfill({
        json: { change: 1_000_000, total: 1, done: 0, finished: false },
      });
    });
    await page.getByRole("button", { name: "Projektdetails" }).click();
    const details = page.getByRole("dialog", { name: "Projektdetails" });
    await details.getByLabel("Anführungszeichen").selectOption({ label: "»…«" });
    await details.getByRole("button", { name: "Speichern" }).click();

    await expect(details).toContainText(
      "Wechseln die Anführungszeichen von “…” zu »…«, werden sie in jeder Szene des Buchs neu gesetzt.",
    );
    await details.getByRole("button", { name: "Zeichen wechseln" }).click();

    await expect(details).toContainText(
      "Die Anführungszeichen werden neu gesetzt: 0 von 1 Szene.",
    );
    await expect(
      details.getByRole("progressbar", {
        name: "Die Anführungszeichen werden neu gesetzt",
      }),
    ).toBeVisible();
    await expect(details).toContainText("Jede Szene setzt jetzt »…«.");
    await details.getByRole("button", { name: "Schließen" }).click();
    await expect(details).toHaveCount(0);
  });

  test("a server out of reach is told in German", async ({ page }) => {
    await page.route("**/api/projects", (route) => route.abort());

    await page.goto("/");

    await expect(page.getByRole("alert")).toHaveText(
      "Der Server ist gerade nicht erreichbar. Bitte versuch es gleich noch einmal.",
    );
  });

  test("a project gone meanwhile is told as a project, in German", async ({
    page,
  }) => {
    const title = `Weg ${crypto.randomUUID().slice(0, 8)}`;
    await page.request.post(`${API}/projects`, {
      data: { name: title, language: "de" },
    });
    await page.goto("/");
    const row = page.getByRole("listitem").filter({ hasText: title });
    await expect(row).toBeVisible();
    await page.route("**/api/projects/*", (route) =>
      route.request().method() === "PATCH"
        ? route.fulfill({ status: 404 })
        : route.continue(),
    );

    await row.getByRole("button", { name: "Weitere Aktionen" }).click();
    await page.getByRole("menuitem", { name: "Umbenennen" }).click();
    const renaming = page.getByRole("listitem").getByRole("textbox");
    await renaming.fill(`${title} neu`);
    await renaming.press("Enter");

    await expect(page.getByRole("alert")).toHaveText(
      "Dieses Projekt gibt es nicht mehr.",
    );
  });
});

test.describe("a browser asking for a language the app does not speak", () => {
  test.use({ locale: "it-IT" });

  test("the app speaks English", async ({ page }) => {
    await page.goto("/");

    await expect(page.getByRole("heading", { level: 1 })).toHaveText(
      "Your projects",
    );
    expect(await spoken(page)).toBe("en");
  });
});

test("a language chosen in the masthead is spoken at once, and kept across a reload", async ({
  page,
}) => {
  await page.goto("/");

  await picker(page, "Language").selectOption("de");

  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Deine Projekte",
  );
  expect(await spoken(page)).toBe("de");
  await page.reload();
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Deine Projekte",
  );
  await expect(picker(page, "Sprache")).toHaveValue("de");
  expect(await spoken(page)).toBe("de");
});

test("the languages a machine translated say so, in the language being spoken", async ({
  page,
}) => {
  await page.goto("/");

  await expect(picker(page, "Language").getByRole("option")).toHaveText([
    "English",
    "Deutsch",
    "Español (machine-translated)",
    "Français (machine-translated)",
    "Português do Brasil (machine-translated)",
  ]);
  await picker(page, "Language").selectOption("de");
  await expect(picker(page, "Sprache").getByRole("option")).toHaveText([
    "English",
    "Deutsch",
    "Español (maschinell übersetzt)",
    "Français (maschinell übersetzt)",
    "Português do Brasil (maschinell übersetzt)",
  ]);
});

test("the book keeps its own language, whatever the app speaks", async ({
  page,
}) => {
  await aNewProject(page, "OwnLanguage");
  await picker(page, "Language").selectOption("de");

  await page
    .getByRole("navigation", { name: "Ansichten" })
    .getByRole("link", { name: "Schreiben" })
    .click();

  await expect(flow(page)).toHaveAttribute("lang", "en");
  expect(await spoken(page)).toBe("de");
});

test("the masthead shows the language spoken by its short name, taking less room than the theme", async ({
  page,
}) => {
  await aNewProject(page, "Short");
  await page.setViewportSize({ width: 390, height: 700 });

  await picker(page, "Language").selectOption("es");

  const shown = page.locator(".masthead").getByText("ES", { exact: true });
  await expect(shown).toBeVisible();
  const taken = (await picker(page, "Idioma").boundingBox())!;
  const theme = (await page.getByRole("group", { name: "Tema" }).boundingBox())!;
  expect(taken.width).toBeLessThan(theme.width);
  expect(
    Math.abs(taken.y + taken.height / 2 - (theme.y + theme.height / 2)),
    "on the same row as the theme",
  ).toBeLessThanOrEqual(2);
});
