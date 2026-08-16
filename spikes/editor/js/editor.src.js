import * as Y from "yjs";
import { EditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { baseKeymap } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import {
  prosemirrorJSONToYXmlFragment,
  redo,
  undo,
  yCursorPlugin,
  ySyncPlugin,
  yUndoPlugin,
} from "y-prosemirror";
import { Awareness } from "y-protocols/awareness";

const schema = new Schema({
  nodes: addListNodes(basic.spec.nodes, "paragraph block*", "block"),
  marks: basic.spec.marks,
});

const FRAGMENT = "prose";
const REMOTE = "remote";

const starter = {
  type: "doc",
  content: [
    { type: "heading", attrs: { level: 1 }, content: [{ type: "text", text: "The Loom" }] },
    {
      type: "paragraph",
      content: [{ type: "text", text: "The loom stood silent in the grey morning light." }],
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
    { type: "paragraph", content: [{ type: "text", text: "She had not touched it since spring." }] },
  ],
};

export class ProseEditor {
  constructor(host, clientId, name, colour, seed, onUpdate) {
    this.doc = new Y.Doc();
    this.doc.clientID = clientId;
    this.fragment = this.doc.getXmlFragment(FRAGMENT);

    this.awareness = new Awareness(this.doc);
    this.awareness.setLocalStateField("user", { name, color: colour });

    this.doc.on("update", (update, origin) => {
      if (origin !== REMOTE) {
        onUpdate(update);
      }
    });

    if (seed) {
      prosemirrorJSONToYXmlFragment(schema, starter, this.fragment);
    }

    this.view = new EditorView(host, {
      state: EditorState.create({
        schema,
        plugins: [
          ySyncPlugin(this.fragment),
          yCursorPlugin(this.awareness),
          yUndoPlugin(),
          keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
          keymap(baseKeymap),
        ],
      }),
    });
  }

  absorb(update) {
    Y.applyUpdate(this.doc, update, REMOTE);
  }

  destroy() {
    this.view.destroy();
    this.awareness.destroy();
    this.doc.destroy();
  }

  focus() {
    this.view.focus();
  }

  plainText() {
    const doc = this.view.state.doc;

    return doc.textBetween(0, doc.content.size, "\n");
  }

  valid() {
    try {
      this.view.state.doc.check();
      return true;
    } catch {
      return false;
    }
  }
}
