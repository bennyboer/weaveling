import { readFileSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { test, expect } from "@playwright/test";

const LABEL = process.env.STRESS_LABEL ?? "before";
const GIVE_UP_AFTER = Number(process.env.STRESS_GIVE_UP_MS ?? 10 * 60_000);
const BOOK = process.env.STRESS_BOOK ?? "10000";
const seeded = JSON.parse(
  readFileSync(join(__dirname, `seeded-${BOOK}.json`), "utf-8"),
);

test("the writing view on a book of many scenes", async ({ page }) => {
  const opened = { sockets: 0, closed: 0 };
  page.on("websocket", (socket) => {
    opened.sockets++;
    socket.on("close", () => opened.closed++);
  });
  const crashed: string[] = [];
  page.on("crash", () => crashed.push("the page crashed"));
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Performance.enable");
  const metrics = async () =>
    Object.fromEntries(
      (await cdp.send("Performance.getMetrics")).metrics.map((metric) => [
        metric.name,
        metric.value,
      ]),
    );

  const began = Date.now();
  await page.goto(`/projects/${seeded.project}/write`);
  const book = page.getByRole("article", { name: "The book" });
  const firstScene = book.locator(".scene .ProseMirror").first();
  let typeable: number | null = null;
  try {
    await expect(firstScene.locator("p").first()).not.toBeEmpty({
      timeout: GIVE_UP_AFTER,
    });
    await expect(firstScene).toBeFocused({
      timeout: Math.max(1, GIVE_UP_AFTER - (Date.now() - began)),
    });
    typeable = Date.now() - began;
    await page.waitForTimeout(5_000);
  } catch {
    typeable = null;
  }
  const settled = await metrics();
  const mounted = await book.locator(".scene").count();
  const editors = await book.locator(".scene .ProseMirror").count();
  const synced = await book
    .locator(".scene .ProseMirror")
    .evaluateAll(
      (editors) => editors.filter((editor) => editor.textContent?.trim()).length,
    );

  let typing: number | null = null;
  if (typeable !== null) {
    const typingBegan = Date.now();
    await page.keyboard.press("End");
    await page.keyboard.type("abcdefghijklmnopqrst");
    await expect(firstScene).toContainText("abcdefghijklmnopqrst");
    typing = Date.now() - typingBegan;
  }

  const scrolling = await page.evaluate(
    () =>
      new Promise<{ median: number; p95: number; over50: number; frames: number }>(
        (done) => {
          const frames: number[] = [];
          let last = performance.now();
          const step = (now: number) => {
            frames.push(now - last);
            last = now;
            window.scrollBy(0, 600);
            if (frames.length < 300) {
              requestAnimationFrame(step);
            } else {
              const sorted = [...frames].sort((a, b) => a - b);
              done({
                median: sorted[Math.floor(sorted.length / 2)],
                p95: sorted[Math.floor(sorted.length * 0.95)],
                over50: frames.filter((frame) => frame > 50).length,
                frames: frames.length,
              });
            }
          };
          requestAnimationFrame(step);
        },
      ),
  );

  const result = {
    label: LABEL,
    scenesInBook: seeded.scenes,
    typeableAfterMs: typeable ?? `not within ${GIVE_UP_AFTER} ms`,
    scenesMounted: mounted,
    editorsMounted: editors,
    scenesWithTextArrived: synced,
    socketsOpen: opened.sockets - opened.closed,
    socketsOpened: opened.sockets,
    jsHeapMb: Math.round(settled.JSHeapUsedSize / 1_048_576),
    domNodes: settled.Nodes,
    twentyKeysMs: typing,
    scrollFrameMs: scrolling,
    crashed,
  };
  console.log(JSON.stringify(result, null, 2));
  mkdirSync("../../target/stress", { recursive: true });
  writeFileSync(`../../target/stress/${LABEL}.json`, JSON.stringify(result, null, 2));
});
