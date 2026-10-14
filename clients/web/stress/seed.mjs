import * as Y from "yjs";
import * as encoding from "lib0/encoding";
import * as decoding from "lib0/decoding";
import { writeFileSync } from "node:fs";

const API = process.env.STRESS_API ?? "http://127.0.0.1:3000/api";
const PARTS = Number(process.env.STRESS_PARTS ?? 20);
const CHAPTERS = Number(process.env.STRESS_CHAPTERS ?? 15);
const SCENES = Number(process.env.STRESS_SCENES ?? 10_000);
const WORDS = Number(process.env.STRESS_WORDS ?? 100);
const AT_ONCE = Number(process.env.STRESS_AT_ONCE ?? 16);

const VOCABULARY = (
  "the loom stood silent in the grey light of morning while she counted threads " +
  "and remembered how her mother had sung to the shuttle long before the war " +
  "came down the valley road with its carts and its rumours and its hunger"
).split(" ");

const MESSAGE_SYNC = 0;
const STEP_ONE = 0;
const STEP_TWO = 1;
const UPDATE = 2;

async function call(method, path, body) {
  const answer = await fetch(`${API}${path}`, {
    method,
    headers: { "content-type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!answer.ok) {
    throw new Error(`${method} ${path} answered ${answer.status}: ${await answer.text()}`);
  }
  const text = await answer.text();
  return text ? JSON.parse(text) : null;
}

async function inTurn(items, atOnce, each) {
  const results = new Array(items.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: atOnce }, async () => {
      while (next < items.length) {
        const at = next++;
        results[at] = await each(items[at], at);
      }
    }),
  );
  return results;
}

function proseOf(nth) {
  const doc = new Y.Doc();
  const fragment = doc.getXmlFragment("prose");
  const paragraphs = [];
  let left = WORDS;
  let word = nth;

  while (left > 0) {
    const length = Math.min(left, 30 + (nth % 20));
    const words = Array.from({ length }, () => VOCABULARY[word++ % VOCABULARY.length]);
    const paragraph = new Y.XmlElement("paragraph");
    paragraph.insert(0, [new Y.XmlText(`${words.join(" ")}.`)]);
    paragraphs.push(paragraph);
    left -= length;
  }
  fragment.insert(0, paragraphs);

  return Y.encodeStateAsUpdate(doc);
}

function framed(kind, write) {
  const encoder = encoding.createEncoder();
  encoding.writeVarUint(encoder, MESSAGE_SYNC);
  encoding.writeVarUint(encoder, kind);
  write(encoder);
  return encoding.toUint8Array(encoder);
}

function written(scene, update) {
  return new Promise((done, failed) => {
    const socket = new WebSocket(`${API.replace(/^http/, "ws")}/sync/${scene}`);
    socket.binaryType = "arraybuffer";
    const vector = Y.encodeStateVector((() => {
      const doc = new Y.Doc();
      Y.applyUpdate(doc, update);
      return doc;
    })());

    socket.onopen = () => {
      socket.send(framed(UPDATE, (encoder) => encoding.writeVarUint8Array(encoder, update)));
      socket.send(framed(STEP_ONE, (encoder) => encoding.writeVarUint8Array(encoder, vector)));
    };
    socket.onmessage = (event) => {
      const decoder = decoding.createDecoder(new Uint8Array(event.data));
      if (decoding.readVarUint(decoder) !== MESSAGE_SYNC) return;
      if (decoding.readVarUint(decoder) !== STEP_TWO) return;
      socket.close();
      done();
    };
    socket.onerror = () => failed(new Error(`the sync socket for ${scene} failed`));
  });
}

function seconds(since) {
  return ((performance.now() - since) / 1000).toFixed(1);
}

const began = performance.now();
const project = await call("POST", "/projects", { name: `Stress ${SCENES}` });
const outline = (await call("POST", "/outlines", { project: project.id })).id;
console.log(`project ${project.id}, outline ${outline}`);

let phase = performance.now();
const chapters = [];
let previousPart = null;
for (let part = 1; part <= PARTS; part++) {
  const made = await call("POST", `/outlines/${outline}/sections`, {
    title: `Part ${part}`,
    under: null,
    after: previousPart,
  });
  previousPart = made.section;
  let previousChapter = null;
  for (let chapter = 1; chapter <= CHAPTERS; chapter++) {
    const nested = await call("POST", `/outlines/${outline}/sections`, {
      title: `Chapter ${part}.${chapter}`,
      under: made.section,
      after: previousChapter,
    });
    previousChapter = nested.section;
    chapters.push(nested.section);
  }
}
console.log(`${PARTS + chapters.length} sections in ${seconds(phase)}s`);

phase = performance.now();
const scenes = await inTurn(
  Array.from({ length: SCENES }, (_, nth) => nth),
  AT_ONCE,
  async () => (await call("POST", "/scenes", { project: project.id })).id,
);
console.log(`${scenes.length} scenes in ${seconds(phase)}s`);

phase = performance.now();
await inTurn(scenes, AT_ONCE, async (scene, nth) => {
  await written(scene, proseOf(nth));
  if ((nth + 1) % 1000 === 0) console.log(`  ${nth + 1} written, ${seconds(phase)}s`);
});
console.log(`${scenes.length} scenes written, ~${WORDS} words each, in ${seconds(phase)}s`);

phase = performance.now();
const perChapter = Math.ceil(SCENES / chapters.length);
let attached = 0;
for (const [nth, chapter] of chapters.entries()) {
  let behind = null;
  for (const scene of scenes.slice(nth * perChapter, (nth + 1) * perChapter)) {
    await call("POST", `/outlines/${outline}/attachments`, {
      attachment: { kind: "scene", id: scene },
      section: chapter,
      after: behind ? { kind: "scene", id: behind } : null,
    });
    behind = scene;
    attached++;
    if (attached % 1000 === 0) console.log(`  ${attached} attached, ${seconds(phase)}s`);
  }
}
console.log(`${attached} scenes attached in ${seconds(phase)}s`);

writeFileSync(
  new URL(`./seeded-${SCENES}.json`, import.meta.url),
  JSON.stringify({ project: project.id, outline, scenes: scenes.length }, null, 2),
);
console.log(`seeded in ${seconds(began)}s; open /projects/${project.id}/write`);
