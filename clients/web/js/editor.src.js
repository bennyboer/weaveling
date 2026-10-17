import * as Y from "yjs";
import { EditorState, Plugin, Selection, TextSelection } from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { baseKeymap, toggleMark } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { redo, undo, yCursorPlugin, ySyncPlugin, yUndoPlugin } from "y-prosemirror";

import { SceneSync } from "./sync.js";

export { SyncConnection } from "./sync.js";

const FRAGMENT = "prose";

const BY_PARAGRAPH = /Mac|iP(hone|[oa]d)/.test(navigator.platform) ? "Alt" : "Ctrl";

const schema = new Schema({
  nodes: addListNodes(basic.spec.nodes, "paragraph block*", "block"),
  marks: basic.spec.marks.addToStart("speech", {
    parseDOM: [{ tag: "q" }],
    toDOM: () => ["q", { class: "speech" }, 0],
  }),
});

const QUOTATION_CHANGED = "quotation-changed";

function speaking(state) {
  const { empty, $from } = state.selection;
  const speech = schema.marks.speech;

  return (
    empty &&
    Boolean(speech.isInSet(state.storedMarks ?? [])) &&
    !speech.isInSet($from.marks())
  );
}

function showingSpeechBegun(editor) {
  return new Plugin({
    props: {
      decorations(state) {
        if (!speaking(state)) {
          return null;
        }
        const begun = document.createElement("span");
        begun.className = "speech-begun";
        begun.textContent = editor.quotationMarks.opening;

        return DecorationSet.create(state.doc, [
          Decoration.widget(state.selection.head, begun, { side: -1, key: begun.textContent }),
        ]);
      },
    },
  });
}

function quotedText(doc, opening, closing) {
  const blocks = [];

  doc.descendants((node) => {
    if (!node.isTextblock) {
      return true;
    }
    let text = "";
    let inSpeech = false;
    node.forEach((child) => {
      const speaks = Boolean(schema.marks.speech.isInSet(child.marks));
      if (speaks !== inSpeech) {
        text += speaks ? opening : closing;
        inSpeech = speaks;
      }
      text += child.text ?? "";
    });
    if (inSpeech) {
      text += closing;
    }
    blocks.push(text);

    return false;
  });

  return blocks.join("\n");
}

function markingWhereWritten() {
  return new Plugin({
    props: {
      decorations(state) {
        const { $head } = state.selection;

        if ($head.depth < 1) {
          return null;
        }

        return DecorationSet.create(state.doc, [
          Decoration.node($head.before(1), $head.after(1), { class: "where-written" }),
        ]);
      },
    },
  });
}

function textOf(doc) {
  return doc.textBetween(0, doc.content.size, " ", " ");
}

export class ProseEditor {
  constructor(host, scene, author, color, onConnected, connection) {
    this.doc = new Y.Doc();
    this.fragment = this.doc.getXmlFragment(FRAGMENT);

    this.sync = new SceneSync(connection, scene, this.doc, onConnected);
    this.sync.awareness.setLocalStateField("user", { name: author, color });

    this.leaving = () => false;
    this.selecting = () => false;
    this.quotationMarks = { opening: "\u201c", closing: "\u201d" };
    this.breaking = null;
    this.changed = () => {};
    this.caretMoved = () => {};

    const editor = this;

    this.view = new EditorView(host, {
      dispatchTransaction: function (tr) {
        if (this.isDestroyed) {
          return;
        }
        if (!tr.selectionSet && !tr.docChanged) {
          const shown = editor.shownSelection(tr.doc);

          if (shown && !shown.eq(tr.selection)) {
            tr.setSelection(shown);
          }
        }
        this.updateState(this.state.apply(tr));

        if (tr.docChanged) {
          editor.changed(textOf(this.state.doc));
        }
        if ((tr.docChanged || tr.selectionSet) && this.hasFocus()) {
          editor.caretMoved(this.state.selection.head);
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
              handleDOMEvents: {
                focus: (view) => this.focusedWhereTheBrowserPutIt(view),
              },
              handleTextInput: (view, from, to, text) => this.typedMark(view, from, to, text),
            },
          }),
          markingWhereWritten(),
          showingSpeechBegun(this),
          ySyncPlugin(this.fragment),
          yCursorPlugin(this.sync.awareness),
          yUndoPlugin(),
          keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
          keymap({
            "Mod-i": toggleMark(schema.marks.em),
            "Mod-b": toggleMark(schema.marks.strong),
          }),
          keymap({
            Enter: (state, dispatch) => this.breakAtEnd(state, dispatch),
            ArrowUp: (state, _, view) => this.leave(state, view, "before", "up"),
            ArrowLeft: (state, _, view) => this.leave(state, view, "before", "left"),
            ArrowDown: (state, _, view) => this.leave(state, view, "after", "down"),
            ArrowRight: (state, _, view) => this.leave(state, view, "after", "right"),
            "Shift-ArrowUp": (state, _, view) => this.selectBeyond(state, view, "before", "up"),
            "Shift-ArrowDown": (state, _, view) => this.selectBeyond(state, view, "after", "down"),
            "Mod-a": (state) => this.selectAllOnceMore(state),
            ...this.jumping("Mod"),
            ...this.jumping("Alt"),
            [`${BY_PARAGRAPH}-ArrowUp`]: (state, dispatch, view) =>
              this.byParagraph(state, dispatch, view, "before"),
            [`${BY_PARAGRAPH}-ArrowDown`]: (state, dispatch, view) =>
              this.byParagraph(state, dispatch, view, "after"),
            [`${BY_PARAGRAPH}-Shift-ArrowUp`]: (state, dispatch, view) =>
              this.byParagraph(state, dispatch, view, "before", true),
            [`${BY_PARAGRAPH}-Shift-ArrowDown`]: (state, dispatch, view) =>
              this.byParagraph(state, dispatch, view, "after", true),
          }),
          keymap(baseKeymap),
        ],
      }),
    });
  }

  focusedWhereTheBrowserPutIt(view) {
    if (!view.focused) {
      view.domObserver.stop();
      view.dom.classList.add("ProseMirror-focused");
      view.domObserver.start();
      view.focused = true;
    }

    return false;
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

  byParagraph(state, dispatch, view, towards, extending = false) {
    const { doc, selection } = state;
    const { $head } = selection;
    const after = towards === "after";
    const edge = after ? $head.end() : $head.start();
    let to = null;

    if ($head.parent.isTextblock && $head.pos !== edge) {
      to = edge;
    } else {
      const beyond = Selection.findFrom(
        doc.resolve(after ? $head.after() : $head.before()),
        after ? 1 : -1,
        true,
      );
      if (beyond) {
        to = after ? beyond.$head.end() : beyond.$head.start();
      }
    }

    if (to === null) {
      return extending
        ? this.selecting(this.wholly(state) ? `beyond-${towards}` : towards)
        : this.leave(state, view, towards, after ? "right" : "left");
    }
    const anchor = extending ? selection.anchor : to;
    dispatch?.(state.tr.setSelection(TextSelection.create(doc, anchor, to)).scrollIntoView());

    return true;
  }

  onChanged(changed) {
    this.changed = changed;

    if (this.sync.synced) {
      changed(textOf(this.view.state.doc));
    }
  }

  onBreaking(breaking) {
    this.breaking = breaking;
  }

  breakAtEnd(state, dispatch) {
    const { selection, doc } = state;
    const { $from } = selection;
    const last = $from.depth === 1 && $from.index(0) === doc.childCount - 1;

    if (
      !this.breaking ||
      !selection.empty ||
      !last ||
      doc.childCount < 2 ||
      $from.parent.content.size > 0
    ) {
      return false;
    }

    dispatch(state.tr.delete($from.before(1), $from.after(1)));
    this.breaking();

    return true;
  }

  onCaretMoved(caretMoved) {
    this.caretMoved = caretMoved;
  }

  landAt(at) {
    const land = () => {
      const focused = document.activeElement;

      if (focused && focused !== document.body) {
        return;
      }

      const { doc } = this.view.state;
      const caret = Math.min(Math.max(at, 0), doc.content.size);

      this.focusAt(TextSelection.near(doc.resolve(caret)));
      const { node } = this.view.domAtPos(this.view.state.selection.head);
      const line = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
      line?.scrollIntoView({ block: "center" });
    };

    this.sync.whenSynced(land);
  }

  arriveWhenSynced(towards) {
    this.sync.whenSynced(() => {
      if (towards === "before") {
        this.focusEnd();
      } else {
        this.focusStart();
      }
      const { node } = this.view.domAtPos(this.view.state.selection.head);
      const line = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
      line?.scrollIntoView({ block: "nearest" });
    });
  }

  onSaving(saving) {
    this.sync.onSaving(saving);
  }

  onLeaving(leaving) {
    this.leaving = leaving;
  }

  setQuotationMarks(opening, closing) {
    this.quotationMarks = { opening, closing };

    if (this.view && !this.view.isDestroyed) {
      this.view.dispatch(this.view.state.tr.setMeta(QUOTATION_CHANGED, true));
    }
  }

  quotedText() {
    return quotedText(
      this.view.state.doc,
      this.quotationMarks.opening,
      this.quotationMarks.closing,
    );
  }

  typedMark(view, from, to, text) {
    if (text === "'") {
      view.dispatch(view.state.tr.insertText("\u2019", from, to));

      return true;
    }
    if (text !== '"') {
      return false;
    }
    const { state } = view;
    const speech = schema.marks.speech;

    if (!state.selection.empty) {
      return toggleMark(speech)(state, view.dispatch);
    }
    const marks = state.storedMarks ?? state.selection.$from.marks();
    view.dispatch(
      speech.isInSet(marks)
        ? state.tr.removeStoredMark(speech)
        : state.tr.addStoredMark(speech.create()),
    );

    return true;
  }

  onSelecting(selecting) {
    this.selecting = selecting;
  }

  selectBeyond(state, view, towards, moving) {
    const { doc, selection } = state;
    const atEdge =
      view.endOfTextblock(moving) &&
      doc.resolve(selection.head).index(0) === (towards === "before" ? 0 : doc.childCount - 1);

    return atEdge && this.selecting(this.wholly(state) ? `beyond-${towards}` : towards);
  }

  wholly(state) {
    const { doc, selection } = state;

    return (
      !selection.empty &&
      selection.from <= Selection.atStart(doc).from &&
      selection.to >= Selection.atEnd(doc).to
    );
  }

  selectAllOnceMore(state) {
    const { doc, selection } = state;
    const everything =
      selection.from <= Selection.atStart(doc).from && selection.to >= Selection.atEnd(doc).to;

    return everything && this.selecting("all");
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
    this.sync.destroy();
    this.doc.destroy();
  }

  focus() {
    this.view.focus();
  }
}
