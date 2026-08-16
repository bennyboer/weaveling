import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import WebSocket from "ws";
import { writeFileSync } from "node:fs";
import { join } from "node:path";

const [port, exchange] = process.argv.slice(2);
if (!port || !exchange) {
  console.error("usage: node two-clients.mjs <port> <exchange-dir>");
  process.exit(1);
}

const ROOM = "chapter-one";
const ENDPOINT = `ws://127.0.0.1:${port}/sync`;

const report = (name, value) =>
  writeFileSync(join(exchange, name), String(value), "utf8");

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function until(what, describe) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (what()) return true;
    await wait(50);
  }
  throw new Error(`timed out waiting for ${describe}`);
}

function join_room(clientId, name) {
  const doc = new Y.Doc();
  doc.clientID = clientId;
  const provider = new WebsocketProvider(ENDPOINT, ROOM, doc, {
    WebSocketPolyfill: WebSocket,
  });
  provider.awareness.setLocalStateField("user", { name });

  return { doc, provider, fragment: doc.getXmlFragment("prose") };
}

function paragraph(text) {
  const element = new Y.XmlElement("paragraph");
  element.insert(0, [new Y.XmlText(text)]);

  return element;
}

const read = (fragment) =>
  fragment
    .toArray()
    .map((node) => node.toString().replace(/<[^>]*>/g, ""))
    .join("\n");

const ada = join_room(100, "Ada");
await until(() => ada.provider.wsconnected, "Ada to connect");
ada.fragment.insert(0, [paragraph("The loom stood silent in the grey morning light.")]);
await wait(200);

// Bo arrives after the edit already happened, so the room itself has to serve it
const bo = join_room(200, "Bo");
await until(() => bo.provider.wsconnected, "Bo to connect");
await until(() => read(bo.fragment).includes("grey morning"), "Bo to be caught up");
report("bo-caught-up.txt", read(bo.fragment));

// concurrent edits in both directions
ada.fragment.insert(1, [paragraph("Ada wrote second.")]);
bo.fragment.insert(1, [paragraph("Bo wrote second.")]);
await until(
  () => read(ada.fragment) === read(bo.fragment) && read(ada.fragment).split("\n").length === 3,
  "the two clients to converge",
);
report("converged.txt", read(ada.fragment));

// awareness travels without the server understanding a byte of it
await until(() => {
  const seen = [...ada.provider.awareness.getStates().values()];
  return seen.some((state) => state.user && state.user.name === "Bo");
}, "Ada to see Bo's awareness");
report(
  "awareness.txt",
  [...ada.provider.awareness.getStates().values()]
    .map((state) => (state.user ? state.user.name : "?"))
    .sort()
    .join(","),
);

report("server-view.txt", await fetch(`http://127.0.0.1:${port}/rooms/${ROOM}`).then((r) => r.text()));

ada.provider.destroy();
bo.provider.destroy();
await wait(100);
process.exit(0);
