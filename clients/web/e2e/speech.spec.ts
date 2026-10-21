import { test, expect, type Page } from "@playwright/test";

import { aBookWithScenes } from "./support/book";
import {
  aNewProject,
  anOpenProject,
  API,
  aLooseScene,
  captureIdea,
  catalogued,
  openTheBoard,
  openTheOutline,
} from "./support/shell";
import { flow, openTheWritingView, projectOf, scene } from "./support/writing";

const spoken = (page: Page) =>
  page.evaluate(() => document.documentElement.getAttribute("lang"));

const picker = (page: Page, named: string) =>
  page.getByRole("combobox", { name: named });


async function aGermanProjectListed(page: Page, title: string) {
  const made = await page.request.post(`${API}/projects`, {
    data: { name: title, language: "de" },
  });
  const { id } = await made.json();
  await expect
    .poll(async () => {
      const listed = await (await page.request.get(`${API}/projects`)).json();
      return listed.some((project: { id: string }) => project.id === id);
    }, { message: "the list of projects is a projection, and a page opened at once could miss it" })
    .toBe(true);

  return id as string;
}

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
    await aNewProject(page, "Ansichten", { boardNamed: "Pinnwand" });

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
    const title = await aNewProject(page, "Löschen", { boardNamed: "Pinnwand" });
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
    let tell: (event: object) => void = () => {};
    await page.routeWebSocket(/\/api\/scenes\/quotation-restylings\/live\?project=/, (socket) => {
      tell = (event) => socket.send(JSON.stringify({ version: 1, event }));
    });
    await aNewProject(page, "Zeichen", { boardNamed: "Pinnwand" });
    await aBookWithScenes(page, { "Part One": 1 });
    await page.route("**/api/scenes/quotation-restylings/*", (route) =>
      route.fulfill({
        json: { change: 1_000_000, total: 1, done: 0, finished: false },
      }),
    );
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
    tell({ change: 1_000_000, total: 1, done: 1, finished: true });
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

  test("a title the server refuses is told in German, with its numbers", async ({
    page,
  }) => {
    await aNewProject(page, "Zu lang", { boardNamed: "Pinnwand" });
    await page.getByRole("button", { name: "Projektdetails" }).click();
    const details = page.getByRole("dialog", { name: "Projektdetails" });

    await details.getByLabel("Titel").fill("a".repeat(201));
    await details.getByRole("button", { name: "Speichern" }).click();

    await expect(details.getByRole("alert")).toHaveText(
      "Ein Titel darf höchstens 200 Zeichen lang sein, dieser hat 201.",
    );
  });

  test("renaming a project deleted meanwhile is told in German", async ({
    page,
  }) => {
    const title = `Gelöscht ${crypto.randomUUID().slice(0, 8)}`;
    const id = await aGermanProjectListed(page, title);
    await page.routeWebSocket(/\/api\/projects\/live$/, () => {});
    await page.goto("/");
    const row = page.getByRole("listitem").filter({ hasText: title });
    await expect(row).toBeVisible();
    await page.request.delete(`${API}/projects/${id}`);

    await row.getByRole("button", { name: "Weitere Aktionen" }).click();
    await page.getByRole("menuitem", { name: "Umbenennen" }).click();
    const renaming = page.getByRole("listitem").getByRole("textbox");
    await renaming.fill(`${title} neu`);
    await renaming.press("Enter");

    await expect(page.getByRole("alert")).toHaveText(
      "Dieses Projekt wurde inzwischen gelöscht.",
    );
  });

  test("an idea's title the server refuses is told in German, with its numbers", async ({
    page,
  }) => {
    await aNewProject(page, "Ideen", { boardNamed: "Pinnwand" });
    await page
      .getByRole("navigation", { name: "Ansichten" })
      .getByRole("link", { name: "Ideen" })
      .click();

    await page
      .getByRole("textbox", { name: "Was ist die Idee?" })
      .fill("a".repeat(2049));
    await page.getByRole("button", { name: "Festhalten" }).click();

    await expect(page.getByRole("alert")).toHaveText(
      "Der Titel einer Idee darf höchstens 2048 Zeichen lang sein, dieser hat 2049.",
    );
  });

  test("a project gone meanwhile is told as a project, in German", async ({
    page,
  }) => {
    const title = `Weg ${crypto.randomUUID().slice(0, 8)}`;
    await aGermanProjectListed(page, title);
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

test("the board speaks the language chosen, at once", async ({ page }) => {
  await anOpenProject(page, "Pinnwand");
  await captureIdea(page, "The loom remembers");
  await openTheBoard(page);

  await picker(page, "Language").selectOption("de");

  const board = page.getByRole("region", { name: "Pinnwand" });
  await expect(board).toBeVisible();
  await expect(page.getByText("Nicht an der Pinnwand · 1").first()).toBeVisible();
  await expect(
    page.getByText("Klick auf eine Idee, um sie dort anzupinnen, wo Platz ist."),
  ).toBeVisible();
  await page
    .getByRole("list", { name: "Ideen, die nicht an der Pinnwand hängen" })
    .getByRole("button", { name: "The loom remembers anpinnen" })
    .click();
  await expect(page.getByText("Jede Idee hängt an der Pinnwand.")).toBeVisible();

  await board.locator(".pinned").filter({ hasText: "The loom remembers" }).click();
  const actions = board.getByRole("toolbar", {
    name: "Aktionen für The loom remembers",
  });
  for (const named of ["umbenennen", "öffnen", "abnehmen"]) {
    await expect(
      actions.getByRole("button", { name: `The loom remembers ${named}` }),
    ).toBeVisible();
  }
  await expect(
    board.getByRole("separator", {
      name: "Größe von The loom remembers oben links ändern",
    }),
  ).toHaveCount(1);
  const zoom = board.getByRole("toolbar", { name: "Zoom" });
  for (const named of ["Verkleinern", "Zoom zurücksetzen", "Vergrößern"]) {
    await expect(zoom.getByRole("button", { name: named })).toBeVisible();
  }
  await expect(page.getByLabel("Name der Idee")).toHaveValue(
    "The loom remembers",
  );
  await expect(page.getByRole("heading", { name: "Kommt vor in" })).toBeVisible();
  await expect(
    page.getByText(
      "Noch nicht im Buch — kein Abschnitt vermerkt sie, keine Szene greift auf sie zurück.",
    ),
  ).toBeVisible();
  await expect(
    page.getByRole("separator", { name: "Breite der Ablage ändern" }),
  ).toHaveCount(1);
  await page.getByRole("button", { name: "Ablage schließen" }).click();
  await expect(page.getByRole("button", { name: "Ablage schließen" })).toHaveCount(0);
});

test("the outline speaks the language chosen, at once", async ({ page }) => {
  await aNewProject(page, "Gliederung");
  await aLooseScene(page);
  await openTheOutline(page);

  await picker(page, "Language").selectOption("de");

  await expect(page.getByRole("list", { name: "Das Manuskript" })).toHaveCount(1);
  await expect(
    page.getByText("Noch nichts im Buch. Leg einen Abschnitt an, um zu beginnen."),
  ).toBeVisible();
  await expect(page.getByText("Nicht im Buch · 1").first()).toBeVisible();
  await expect(
    page
      .getByRole("list", { name: "Szenen, die nicht im Buch stehen" })
      .getByRole("button", { name: "Leer einordnen" }),
  ).toBeVisible();
  await expect(
    page.getByRole("list", { name: "Ideen, die nicht im Buch stehen" }),
  ).toHaveCount(1);
  await page.getByRole("button", { name: "Abschnitt anlegen" }).click();

  await expect(
    page.getByRole("textbox", { name: "Abschnitt Ohne Titel" }),
  ).toHaveAttribute("placeholder", "Ohne Titel");
  const actions = page.getByRole("toolbar", { name: "Aktionen für Ohne Titel" });
  for (const named of [
    "Ohne Titel nach vorn",
    "Ohne Titel nach hinten",
    "Ohne Titel eine Ebene höher",
    "Ohne Titel eine Ebene tiefer",
    "In Ohne Titel schreiben",
    "Ohne Titel entfernen",
  ]) {
    await expect(actions.getByRole("button", { name: named })).toHaveCount(1);
  }
  await expect(
    page.getByRole("button", { name: "Ohne Titel verschieben" }),
  ).toHaveCount(1);
});

test("the ideas speak the language chosen, at once", async ({ page }) => {
  await anOpenProject(page, "Ideen");

  await picker(page, "Language").selectOption("de");

  await expect(page.getByRole("heading", { name: "Ideen", exact: true })).toBeVisible();
  await expect(
    page.getByText("Noch keine Ideen. Schieß eine Idee rein und schau, wohin sie führt."),
  ).toBeVisible();
  const asked = page.getByRole("textbox", { name: "Was ist die Idee?" });
  await expect(asked).toHaveAttribute("placeholder", "Was ist die Idee?");
  await asked.fill("The loom remembers");
  await page.getByRole("button", { name: "Festhalten" }).click();
  await expect(
    page.getByRole("list", { name: "Ideen" }).getByText("The loom remembers"),
  ).toBeVisible();

  await page.route("**/api/ideas/idea_*", async (route) => {
    await new Promise((settled) => setTimeout(settled, 1500));
    await route.continue();
  });
  await page.getByRole("list", { name: "Ideen" }).getByText("The loom remembers").click();
  await expect(page.getByText("Wird geöffnet…")).toBeVisible();
});

test("the writing view speaks the language chosen, at once", async ({
  page,
}) => {
  await aNewProject(page, "Schreiben");
  const scenes = await aBookWithScenes(page, { "Part One": 1 });
  await aLooseScene(page);
  await openTheWritingView(page);
  await scene(page, scenes["Part One"][0]).click();
  await page.keyboard.type("Der Webstuhl schwieg");

  await picker(page, "Language").selectOption("de");

  const book = page.getByRole("article", { name: "Das Buch" });
  await expect(book).toBeVisible();
  expect(
    await book.evaluate((shown) =>
      getComputedStyle(shown).getPropertyValue("--empty-scene"),
    ),
  ).toBe('"Eine leere Szene. Fang an zu schreiben…"');
  await expect(book.getByRole("heading", { name: "Part One" })).toHaveAttribute(
    "data-placeholder",
    "Abschnitt ohne Titel",
  );
  const sides = page.getByRole("navigation", { name: "Seiten des Werks" });
  await expect(sides.getByRole("link")).toHaveText(["Das Buch", "Nicht im Buch1"]);
  const toggles = page.locator(".drawer-toggles button");
  await expect(toggles.nth(0)).toHaveAttribute("aria-label", "Inhalt");
  await expect(toggles.nth(1)).toHaveAttribute("aria-label", "Diese Szene");
  await expect(page.getByText("3 Wörter in „Part One“")).toBeVisible();
  await expect(page.getByText("3 Wörter in „Part One“")).toHaveAttribute(
    "title",
    "Das ganze Buch zählen",
  );
  await expect(page.getByText("Alles gespeichert")).toBeVisible();
  await expect(page.getByRole("button", { name: "Fokusmodus" })).toHaveAttribute(
    "title",
    "Fokusmodus (Strg+Umschalt+F)",
  );

  await page.getByRole("button", { name: "Schreibeinstellungen" }).click();
  const settings = page.getByRole("dialog", { name: "Schreibeinstellungen" });
  for (const named of ["Rechtschreibung prüfen", "Blocksatz", "Schreibmaschinenmodus"]) {
    await expect(settings.getByLabel(named)).toHaveCount(1);
  }
  await expect(
    settings.getByText("Hält die Zeile, in der du schreibst, in der Mitte des Fensters."),
  ).toBeVisible();
  await settings.getByRole("button", { name: "Schließen" }).click();

  await page.getByRole("button", { name: "Fokusmodus" }).click();
  await expect(page.getByRole("navigation", { name: "Wo du bist" })).toBeVisible();
});

test("the writing view's menus and questions speak the language chosen", async ({
  page,
}) => {
  await aNewProject(page, "Rand");
  const made = await aBookWithScenes(page, { "Part One": 2, "Part Two": 1 });
  const [first, second] = made["Part One"];
  await openTheWritingView(page);
  await picker(page, "Language").selectOption("de");
  const book = page.getByRole("article", { name: "Das Buch" });
  const written = (id: string) => book.locator(`[data-scene="${id}"] .ProseMirror`);

  await expect(
    book.getByRole("button", { name: "Szenenaktionen" }).first(),
  ).toHaveAttribute("title", "Szenenaktionen (Umschalt+F10)");
  await expect(
    book.getByRole("button", { name: "Szene verschieben" }).first(),
  ).toHaveAttribute("title", "Zum Verschieben ziehen");
  await written(first).click();
  await page.keyboard.press("Shift+F10");
  await expect(book.getByRole("menuitem")).toHaveText([
    "Umbenennen",
    "Aus dem Buch nehmen",
    "Szene löschen",
  ]);
  await book.getByRole("menuitem", { name: "Szene löschen" }).click();
  const deleting = page.getByRole("dialog", { name: "Diese Szene löschen?" });
  await expect(deleting).toContainText(
    "Diese Szene und alles, was darin steht, ist dann für immer weg. Löschen lässt sich nicht rückgängig machen.",
  );
  await expect(deleting.getByRole("button")).toHaveText(["Abbrechen", "Löschen"]);
  await deleting.getByRole("button", { name: "Abbrechen" }).click();

  await book.getByRole("heading", { name: "Part Two" }).click();
  await page.keyboard.press("Shift+F10");
  const nesting = book.getByRole("menuitem", {
    name: "Unter den vorigen Abschnitt einrücken",
    exact: true,
  });
  await expect(nesting).toHaveAttribute("aria-keyshortcuts", "Tab");
  await expect(nesting.locator("kbd")).toHaveText("Tab");
  await nesting.click();
  await book.getByRole("heading", { name: "Part Two" }).click();
  await page.keyboard.press("Shift+F10");
  const lifting = book.getByRole("menuitem", { name: "Eine Ebene herausrücken", exact: true });
  await expect(lifting).toHaveAttribute("aria-keyshortcuts", "Shift+Tab");
  await expect(lifting.locator("kbd")).toHaveText("Umschalt+Tab");
  await page.keyboard.press("Escape");

  await written(first).click();
  await page.keyboard.type("Eins.");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Shift+ArrowDown");
  await page.keyboard.press("Delete");
  const removing = page.getByRole("dialog", { name: "Das Gewählte entfernen?" });
  await expect(removing).toContainText(
    "2 Szenen sind gewählt. Aus dem Buch genommen, warten die Szenen bei denen, die nicht darin stehen.",
  );
  await expect(removing.getByRole("button")).toHaveText([
    "Abbrechen",
    "Aus dem Buch nehmen",
    "Endgültig löschen",
  ]);
  await removing.getByRole("button", { name: "Abbrechen" }).click();
  await expect(written(second)).toHaveCount(1);
});

test("bringing a scene back and placing one speak the language chosen", async ({
  page,
}) => {
  await aNewProject(page, "Zurück");
  const made = await aBookWithScenes(page, { "Part One": 1 });
  const [only] = made["Part One"];
  await aLooseScene(page);
  await openTheWritingView(page);
  await picker(page, "Language").selectOption("de");
  const book = page.getByRole("article", { name: "Das Buch" });

  await book
    .locator(`.insertion[data-after="${only}"]`)
    .getByRole("button", { name: "Hier einfügen" })
    .click();
  await expect(book.getByRole("menuitem", { name: "Neue Szene" })).toBeVisible();
  await book.getByRole("menuitem", { name: "Szene zurückholen…" }).click();
  const bringing = page.getByRole("dialog", { name: "Szene zurückholen" });
  await expect(bringing).toContainText(
    "Diese Szenen stehen nicht im Buch. Die gewählte kommt dorthin, wo du geklickt hast.",
  );
  await expect(bringing.getByRole("searchbox", { name: "Szene suchen" })).toBeVisible();
  await expect(
    bringing.getByRole("list", { name: "Szenen, die nicht im Buch stehen" }),
  ).toBeVisible();
  await bringing.getByRole("searchbox", { name: "Szene suchen" }).fill("nirgends");
  await expect(bringing).toContainText("Keine Szene passt.");
  await bringing.getByRole("button", { name: "Abbrechen" }).click();

  await page
    .getByRole("navigation", { name: "Seiten des Werks" })
    .getByRole("link", { name: "Nicht im Buch" })
    .click();
  await page.getByRole("button", { name: "Szenenaktionen" }).first().click();
  await page.getByRole("menuitem", { name: "Ins Buch holen…" }).click();
  const placing = page.getByRole("dialog", { name: "Ins Buch holen" });
  await expect(placing).toContainText(
    "Die Szene kommt ans Ende des Abschnitts, den du wählst.",
  );
  await expect(
    placing.getByRole("list", { name: "Abschnitte des Buchs" }),
  ).toBeVisible();
});

test("the contents and the tray speak the language chosen", async ({ page }) => {
  await aNewProject(page, "Ablage");
  const [first] = (await aBookWithScenes(page, { "Part One": 1 }))["Part One"];
  const captured = await page.request.post(`${API}/ideas`, {
    data: { project: projectOf(page), title: "The loom remembers" },
  });
  expect(captured.status()).toBe(201);
  await catalogued(page, ["The loom remembers"]);
  await openTheWritingView(page);
  await picker(page, "Language").selectOption("de");
  const book = page.getByRole("article", { name: "Das Buch" });

  await page.getByRole("button", { name: "Inhalt", exact: true }).click();
  const rail = page.getByRole("complementary", { name: "Inhalt" });
  await expect(rail.getByRole("button", { name: "Alles aufklappen" })).toHaveCount(1);
  await expect(rail.getByRole("button", { name: "Alles einklappen" })).toHaveCount(1);
  await rail.getByRole("button", { name: "Part One aufklappen" }).click();
  await expect(rail.getByRole("button", { name: "Part One einklappen" })).toHaveCount(1);
  await expect(rail.getByRole("button", { name: "Eine leere Szene" })).toBeVisible();
  await rail.getByRole("button", { name: "Inhaltsverzeichnis schließen" }).click();

  await book.locator(`[data-scene="${first}"] .ProseMirror`).click();
  await page.getByRole("button", { name: "Diese Szene" }).click();
  const tray = page.getByRole("complementary", { name: "Diese Szene" });
  await expect(tray.getByRole("textbox", { name: "Szenentitel" })).toBeVisible();
  await expect(tray.getByRole("heading", { name: "Schöpft aus" })).toBeVisible();
  await expect(tray.getByText("Noch keine Ideen verknüpft.")).toBeVisible();
  await tray.getByRole("button", { name: "Idee verknüpfen" }).click();
  const linking = page.getByRole("dialog", { name: "Ideen verknüpfen" });
  await expect(linking.getByRole("searchbox", { name: "Idee suchen" })).toBeVisible();
  await expect(linking.getByRole("button", { name: "Verknüpfen" })).toBeDisabled();
  await linking
    .getByRole("list", { name: "Ideen zum Verknüpfen" })
    .getByLabel("The loom remembers")
    .check();
  await linking.getByRole("button", { name: "Verknüpfen" }).click();
  await expect(
    tray
      .getByRole("list", { name: "Schöpft aus" })
      .getByRole("button", { name: "Verknüpfung mit The loom remembers lösen" }),
  ).toBeVisible();

  await book.getByRole("heading", { name: "Part One" }).click();
  const section = page.getByRole("complementary", { name: "Dieser Abschnitt" });
  await expect(section.getByRole("heading", { name: "Vermerkte Ideen" })).toBeVisible();
  await expect(section.getByText("Hier sind keine Ideen vermerkt.")).toBeVisible();
  await section.getByRole("button", { name: "Idee vermerken" }).click();
  const pinning = page.getByRole("dialog", { name: "Ideen vermerken" });
  await pinning.getByRole("searchbox", { name: "Idee suchen" }).fill("nirgends");
  await expect(pinning.getByText("Keine Idee passt.")).toBeVisible();
});

test.describe("a browser asking for German, about what went wrong", () => {
  test.use({ locale: "de-DE" });

  test("a project's stamp is told in German", async ({ page }) => {
    const title = `Datiert ${crypto.randomUUID().slice(0, 8)}`;
    await aGermanProjectListed(page, title);

    await page.goto("/");

    const month = new Intl.DateTimeFormat("de-DE", { month: "short" })
      .format(new Date())
      .replace(".", "");
    await expect(
      page.getByRole("listitem").filter({ hasText: title }).locator(".stamp"),
    ).toHaveText(new RegExp(`^\\d{1,2}\\. ${month}\\.? 20\\d\\d, \\d{2}:\\d{2}$`));
  });

  test("an address that leads nowhere says so in German", async ({ page }) => {
    await page.goto("/nowhere/at/all");

    await expect(page.getByText("An dieser Adresse ist nichts gewoben.")).toBeVisible();
    await expect(
      page.getByRole("link", { name: "Zurück zu deinen Projekten" }),
    ).toBeVisible();
  });

  test("the alarm and the changes it holds speak German", async ({ page }) => {
    await page.route("**/api/service/refusals", (route) =>
      route.fulfill({
        json: [
          {
            id: 1,
            listener: "catalogue-idea",
            routing: "idea.captured",
            attempts: 5,
            why: "the projection had not landed",
            plainly: null,
            occurred_at: "2026-10-10T09:00:00Z",
            given_up_at: "2026-10-10T09:01:00Z",
            acknowledged_at: null,
          },
        ],
      }),
    );

    await page.goto("/");

    const status = page.getByRole("contentinfo", { name: "Status" });
    await expect(status.getByRole("status")).toContainText(
      "Einige Änderungen konnten nicht übernommen werden.",
    );
    await expect(status.getByRole("button", { name: "Schließen" })).toHaveCount(1);
    await status.getByRole("button", { name: "Anzeigen" }).click();
    const refused = page.getByRole("dialog", {
      name: "Änderungen, die nicht übernommen werden konnten",
    });
    await expect(refused).toContainText(
      "Sie wurden gespeichert, kamen aber nicht überall an, wo sie hin sollten.",
    );
    const entry = refused
      .getByRole("list", { name: "Abgewiesene Änderungen" })
      .getByRole("listitem");
    await expect(entry).toContainText("Eine Änderung kam nicht überall an, wo sie hin sollte.");
    await expect(entry.locator(".when")).toHaveText(
      /^Aufgegeben am 10\. Okt\. 2026, \d{2}:01$/,
    );
    await entry.getByText("Details").click();
    await expect(entry.locator("dt")).toHaveText(["Empfänger", "Nachricht", "Versuche", "Grund"]);
    await expect(entry.getByRole("button")).toHaveText(["Noch einmal versuchen", "Bestätigen"]);
    await expect(refused.getByRole("button", { name: "Schließen" }).last()).toBeVisible();
    await expect(
      status.getByRole("button", {
        name: "Änderungen, die nicht übernommen werden konnten",
      }),
    ).toHaveCount(1);
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

  await expect(page.getByRole("article", { name: "Das Buch" })).toHaveAttribute(
    "lang",
    "en",
  );
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
