import { test } from "@playwright/test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
test.setTimeout(10 * 60_000);
test("probe", async ({ page }) => {
  const seeded = JSON.parse(readFileSync(join(__dirname, "../stress/seeded-10000.json"), "utf-8"));
  await page.addInitScript(() => {
    (window as any).__long = [];
    new PerformanceObserver((list) => list.getEntries().forEach((e) => (window as any).__long.push([Math.round(e.startTime), Math.round(e.duration)]))).observe({ type: "longtask", buffered: true });
  });
  let t = Date.now();
  await page.goto(`/projects/${seeded.project}/outline`);
  await page.locator(".leaf").first().waitFor({ timeout: 300_000 });
  console.log("outline leaves drawn after", Date.now() - t, "ms; leaves:", await page.locator(".leaf").count());
  const target = page.locator(".leaf a").nth(5000);
  await target.scrollIntoViewIfNeeded();
  const named = await target.textContent();
  const longBefore = await page.evaluate(() => (window as any).__long.length);
  t = Date.now();
  const clickedAt = await page.evaluate(() => performance.now());
  await target.click({ timeout: 300_000 });
  const clickReturned = Date.now() - t;
  await page.locator(".writing .ProseMirror:focus").waitFor({ timeout: 300_000 });
  console.log("leaf", JSON.stringify(named), "click returned after", clickReturned, "ms; focused in the writing view after", Date.now() - t, "ms");
  const long = await page.evaluate(([from, at]) => (window as any).__long.slice(from).map(([s, d]: number[]) => `+${s - at}ms: ${d}ms`), [longBefore, clickedAt]);
  console.log("long tasks after the click:", long.length, "\n" + long.slice(0, 25).join("\n"));
});
