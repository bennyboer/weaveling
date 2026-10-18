import { test, expect } from "@playwright/test";

import { aBookInTheWritingView, scene, theBook } from "./support/writing";

for (let run = 0; run < 12; run++) {
  test(`probe ${run}`, async ({ page }) => {
    const made = await aBookInTheWritingView(page, "Probe", { "Part One": 1 });
    const [first] = made["Part One"];
    await scene(page, first).click();
    await page.keyboard.type("Existing.");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Enter");
    await page.keyboard.type("Begun right here.");
    await expect.poll(async () => (await theBook(page))[0][1].length).toBe(2);
    const [, begun] = (await theBook(page))[0][1];
    await page.keyboard.press("ControlOrMeta+End");
    await page.keyboard.press("Shift+ArrowDown");
    await page.keyboard.press("Delete");
    await page.getByRole("dialog", { name: "Remove what is chosen?" }).getByRole("button", { name: "Take out of the book" }).click();
    const outside = page.getByRole("link", { name: /Not in the book/ });
    await expect(outside).toBeVisible();
    const toldBeforeClick = await outside.textContent();
    const serverBeforeClick = JSON.stringify(await theBook(page));
    await outside.click();
    await page.waitForTimeout(300);
    const shownAfter = await page.getByRole("link", { name: /Not in the book/ }).textContent();
    await page.waitForTimeout(2000);
    const serverLater = JSON.stringify(await theBook(page));
    console.log(`RUN ${run} begun=${begun.slice(-6)} linkBefore=${toldBeforeClick} serverBefore=${serverBeforeClick.includes(begun)} shownAfter=${shownAfter} serverLater=${serverLater.includes(begun)}`);
  });
}
