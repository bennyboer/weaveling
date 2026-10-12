import * as Y from "yjs";
import { WebsocketProvider } from "y-websocket";
import { EditorState, Plugin, Selection, TextSelection } from "prosemirror-state";
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

function openingWordsOf(doc) {
  return doc.textBetween(0, Math.min(doc.content.size, 400), " ");
}

export class ProseEditor {
  constructor(host, scene, author, color, onConnected) {
    this.doc = new Y.Doc();
    this.fragment = this.doc.getXmlFragment(FRAGMENT);

    this.provider = new WebsocketProvider(endpoint(), scene, this.doc);
    this.provider.awareness.setLocalStateField("user", { name: author, color });
    this.provider.on("status", ({ status }) => onConnected(status === "connected"));

    this.leaving = () => false;
    this.breaking = () => {};
    this.changed = () => {};

    const editor = this;

    this.view = new EditorView(host, {
      dispatchTransaction: function (tr) {
        if (!tr.selectionSet && !tr.docChanged) {
          const shown = editor.shownSelection(tr.doc);

          if (shown && !shown.eq(tr.selection)) {
            tr.setSelection(shown);
          }
        }
        this.updateState(this.state.apply(tr));

        if (tr.docChanged) {
          editor.changed(openingWordsOf(this.state.doc));
        }
      },
      state: EditorState.create({
        schema,
        plugins: [
          new Plugin({
            props: {
              handleKeyDown: (view) => {
                const shown = this.shownSelection(view.state.doc);

                if (shown && !shown.eq(view.state.selection)) {
                  view.dispatch(view.state.tr.setSelection(shown));
                }

                return false;
              },
            },
          }),
          ySyncPlugin(this.fragment),
          yCursorPlugin(this.provider.awareness),
          yUndoPlugin(),
          keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
          keymap({
            Enter: (state, dispatch) => this.breakAtEnd(state, dispatch),
            ArrowUp: (state, _, view) => this.leave(state, view, "before", "up"),
            ArrowLeft: (state, _, view) => this.leave(state, view, "before", "left"),
            ArrowDown: (state, _, view) => this.leave(state, view, "after", "down"),
            ArrowRight: (state, _, view) => this.leave(state, view, "after", "right"),
            ...this.jumping("Mod"),
            ...this.jumping("Alt"),
          }),
          keymap(baseKeymap),
        ],
      }),
    });
  }

  jumping(modifier) {
    const across = (towards) => (state, _, view) =>
      this.leave(state, view, towards, towards === "before" ? "left" : "right");

    return {
      [`${modifier}-ArrowLeft`]: across("before"),
      [`${modifier}-ArrowUp`]: across("before"),
      [`${modifier}-ArrowRight`]: across("after"),
      [`${modifier}-ArrowDown`]: across("after"),
    };
  }

  onChanged(changed) {
    this.changed = changed;
    changed(openingWordsOf(this.view.state.doc));
  }

  onBreaking(breaking) {
    this.breaking = breaking;
  }

  breakAtEnd(state, dispatch) {
    const { selection, doc } = state;
    const { $from } = selection;
    const last = $from.depth === 1 && $from.index(0) === doc.childCount - 1;

    if (!selection.empty || !last || doc.childCount < 2 || $from.parent.content.size > 0) {
      return false;
    }

    dispatch(state.tr.delete($from.before(1), $from.after(1)));
    this.breaking();

    return true;
  }

  onLeaving(leaving) {
    this.leaving = leaving;
  }

  leave(state, view, towards, moving) {
    const { doc } = state;
    const at = this.cursor(state, view);

    if (at === null) {
      return false;
    }

    const edge = towards === "before" ? Selection.atStart(doc) : Selection.atEnd(doc);
    const sideways = moving === "left" || moving === "right";
    const atEdge = sideways
      ? at === edge.from
      : view.endOfTextblock(moving) &&
        doc.resolve(at).index(0) === (towards === "before" ? 0 : doc.childCount - 1);

    return atEdge && this.leaving(towards);
  }

  shownSelection(doc) {
    const view = this.view;
    const shown = view?.root.getSelection();

    if (!view || !view.hasFocus() || !shown || shown.rangeCount === 0) {
      return null;
    }
    if (!view.dom.contains(shown.anchorNode) || !view.dom.contains(shown.focusNode)) {
      return null;
    }

    try {
      const anchor = view.posAtDOM(shown.anchorNode, shown.anchorOffset);
      const head = view.posAtDOM(shown.focusNode, shown.focusOffset);

      return TextSelection.between(doc.resolve(anchor), doc.resolve(head));
    } catch {
      return null;
    }
  }

  cursor(state, view) {
    const shown = view.root.getSelection();

    if (shown && shown.rangeCount > 0 && view.dom.contains(shown.focusNode)) {
      return shown.isCollapsed ? view.posAtDOM(shown.focusNode, shown.focusOffset) : null;
    }

    return state.selection.empty ? state.selection.from : null;
  }

  seed(text) {
    const { state } = this.view;

    this.view.dispatch(state.tr.insertText(text, Selection.atEnd(state.doc).from));
    this.focusEnd();
  }

  focusStart() {
    this.focusAt(Selection.atStart(this.view.state.doc));
  }

  focusEnd() {
    this.focusAt(Selection.atEnd(this.view.state.doc));
  }

  focusAt(selection) {
    const { state } = this.view;

    this.view.dispatch(state.tr.setSelection(TextSelection.near(selection.$from)));
    this.view.focus();
    this.view.input.lastFocus = 0;
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
