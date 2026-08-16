import * as Y from "yjs";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const exchange = process.argv[2];
if (!exchange) {
  console.error("usage: node roundtrip.mjs <exchange-dir>");
  process.exit(1);
}

const read = (name) => new Uint8Array(readFileSync(join(exchange, name)));
const write = (name, bytes) => writeFileSync(join(exchange, name), Buffer.from(bytes));
const writeText = (name, text) => write(name, Buffer.from(text, "utf8"));

function converge({ prefix, clientId, edit }) {
  const doc = new Y.Doc();
  doc.clientID = clientId;

  // 1. adopt the document yrs authored
  Y.applyUpdate(doc, read(`${prefix}base.bin`));
  const prose = doc.getText("prose");
  writeText(`${prefix}js-saw-base.txt`, prose.toString());

  // 2. edit concurrently, without having seen the Rust edit.
  //    the state vector came from yrs, so this proves state vectors cross the boundary too.
  edit(prose);
  write(`${prefix}js.bin`, Y.encodeStateAsUpdate(doc, read(`${prefix}base-sv.bin`)));

  // 3. take the concurrent Rust edit and converge
  Y.applyUpdate(doc, read(`${prefix}rust.bin`));
  writeText(`${prefix}js-final.txt`, prose.toString());
  write(`${prefix}js-final-sv.bin`, Y.encodeStateVector(doc));
}

// different positions — the easy case
converge({
  prefix: "apart-",
  clientId: 200,
  edit: (prose) => prose.insert(0, "JS wrote first. "),
});

// the same position — only agreeing tie-break rules converge here
converge({
  prefix: "tie-",
  clientId: 200,
  edit: (prose) => prose.insert(1, "B"),
});
