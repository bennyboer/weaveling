import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import { EditorState } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { baseKeymap } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { redo, undo, yCursorPlugin, ySyncPlugin, yUndoPlugin } from "y-prosemirror";

const FRAGMENT = "prose";

const schema = new Schema({
  nodes: addListNodes(basic.spec.nodes, "paragraph block*", "block"),
  marks: basic.spec.marks,
});

function endpoint() {
  const scheme = window.location.protocol === "https:" ? "wss:" : "ws:";

  return `${scheme}//${window.location.host}/api/sync`;
}

export class ProseEditor {
  constructor(host, passage, author, color, onConnected) {
    this.doc = new Y.Doc();
    this.fragment = this.doc.getXmlFragment(FRAGMENT);

    this.provider = new WebsocketProvider(endpoint(), passage, this.doc);
    this.provider.awareness.setLocalStateField("user", { name: author, color });
    this.provider.on("status", ({ status }) => onConnected(status === "connected"));

    this.view = new EditorView(host, {
      state: EditorState.create({
        schema,
        plugins: [
          ySyncPlugin(this.fragment),
          yCursorPlugin(this.provider.awareness),
          yUndoPlugin(),
          keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
          keymap(baseKeymap),
        ],
      }),
    });
  }

  destroy() {
    this.view.destroy();
    this.provider.destroy();
    this.doc.destroy();
  }

  focus() {
    this.view.focus();
  }

  plainText() {
    const doc = this.view.state.doc;

    return doc.textBetween(0, doc.content.size, "\n");
  }
}
