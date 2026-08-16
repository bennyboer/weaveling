import * as Y from "yjs";
import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { prosemirrorJSONToYXmlFragment, yXmlFragmentToProsemirrorJSON } from "y-prosemirror";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [exchange, phase] = process.argv.slice(2);
if (!exchange || !phase) {
  console.error("usage: node prosemirror.mjs <exchange-dir> <emit|verify>");
  process.exit(1);
}

const read = (name) => new Uint8Array(readFileSync(join(exchange, name)));
const write = (name, bytes) => writeFileSync(join(exchange, name), Buffer.from(bytes));
const writeText = (name, text) => write(name, Buffer.from(text, "utf8"));

// the schema a book editor would actually use: paragraphs, headings, quotes,
// lists, images and inline emphasis
const schema = new Schema({
  nodes: addListNodes(basic.spec.nodes, "paragraph block*", "block"),
  marks: basic.spec.marks,
});

// a node's prose is NOT one paragraph — this is what an author might legitimately
// put inside a single structure-tree node
const manuscript = {
  type: "doc",
  content: [
    { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "The Loom" }] },
    {
      type: "paragraph",
      content: [
        { type: "text", text: "The loom stood silent in the grey morning light." },
      ],
    },
    {
      type: "blockquote",
      content: [
        {
          type: "bullet_list",
          content: [
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "warp threads" }] }],
            },
            {
              type: "list_item",
              content: [{ type: "paragraph", content: [{ type: "text", text: "weft threads" }] }],
            },
          ],
        },
      ],
    },
    {
      // `image` is an inline node, so it lives inside a paragraph, not beside one
      type: "paragraph",
      content: [
        {
          type: "image",
          attrs: { src: "blob://weaveling/loom-sketch", alt: "a sketch of the loom" },
        },
      ],
    },
    { type: "paragraph", content: [{ type: "text", text: "She had not touched it since spring." }] },
  ],
};

function seeded(clientId) {
  const doc = new Y.Doc();
  doc.clientID = clientId;
  prosemirrorJSONToYXmlFragment(schema, manuscript, doc.getXmlFragment("prose"));

  return doc;
}

if (phase === "emit") {
  const doc = seeded(200);
  write("pm-doc.bin", Y.encodeStateAsUpdate(doc));

  // what the server ought to be able to extract, for search, the in-order walk and export
  const parsed = schema.nodeFromJSON(manuscript);
  writeText("pm-expected-text.txt", parsed.textBetween(0, parsed.content.size, "\n"));

  // heavy editing on the real shared type, so the numbers describe real prose
  const churn = seeded(201);
  const paragraph = churn.getXmlFragment("prose").get(1);
  const prose = paragraph.get(0);
  // the log has to start with the document itself, or a merge of it rebuilds nothing
  const log = [Y.encodeStateAsUpdate(churn)];
  let seen = Y.encodeStateVector(churn);

  for (let round = 0; round < 500; round += 1) {
    const length = prose.length;
    const at = (round * 7) % Math.max(length - 9, 1);
    churn.transact(() => {
      prose.delete(at, Math.min(8, length - at));
      prose.insert(at, "rewoven ");
    });
    log.push(Y.encodeStateAsUpdate(churn, seen));
    seen = Y.encodeStateVector(churn);
  }

  write("pm-churn-snapshot.bin", Y.encodeStateAsUpdate(churn));
  write("pm-churn-compacted.bin", Y.mergeUpdates(log));
  writeText("pm-churn-log-bytes.txt", String(log.reduce((n, u) => n + u.length, 0)));
  writeText("pm-churn-text.txt", churn.getXmlFragment("prose").get(1).get(0).toString());
}

if (phase === "verify") {
  // a Rust-authored edit must still parse back into a valid ProseMirror document
  const back = new Y.Doc();
  Y.applyUpdate(back, read("pm-after-rust.bin"));

  const json = yXmlFragmentToProsemirrorJSON(back.getXmlFragment("prose"));
  const node = schema.nodeFromJSON(json);
  node.check();

  writeText("pm-reparsed-text.txt", node.textBetween(0, node.content.size, "\n"));
  writeText("pm-reparsed-kinds.txt", node.content.content.map((child) => child.type.name).join(","));
}
