import * as Y from "yjs";
import { EditorState, Plugin, Selection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";
import { baseKeymap, toggleMark } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import { redo, undo, ySyncPlugin, yUndoPlugin } from "y-prosemirror";

import { SceneSync } from "../sync.js";
import {
  FigureView,
  backIntoFigure,
  captioning,
  clickedFigure,
  leftCaption,
  placed,
  removing,
  sizing,
} from "./figures.js";
import { QuotationMarks, clearing } from "./formatting.js";
import { linking, openedLink, unlinking } from "./links.js";
import { Presence } from "./presence.js";
import { assetOf, linkTyped, schema } from "./schema.js";
import {
  markingWhereWritten,
  onEmptyLine,
  selectionTold,
  shownSelection,
} from "./selection.js";
import { textOf, writtenText } from "./text.js";
import { Uploads } from "./uploads.js";
import {
  Walk,
  arrived,
  focusEnd,
  focusStart,
  focusedWhereTheBrowserPutIt,
  landed,
} from "./walking.js";

export { SyncConnection } from "../sync.js";

const FRAGMENT = "prose";

export class ProseEditor {
  constructor(host, scene, account, onConnected, connection) {
    this.doc = new Y.Doc();
    this.fragment = this.doc.getXmlFragment(FRAGMENT);
    this.sync = new SceneSync(connection, scene, this.doc, onConnected);
    this.presence = new Presence(
      this.sync.awareness,
      this.doc.clientID,
      account,
      connection,
    );
    this.walk = new Walk();
    this.quotationMarks = new QuotationMarks();
    this.labels = {};
    this.uploads = new Uploads({
      labels: () => this.labels,
      redraw: () => this.redraw(),
      refocus: () => this.view.focus(),
    });

    this.changed = () => {};
    this.caretMoved = () => {};
    this.selectionShown = () => {};
    this.toToolbar = () => {};
    this.linkAsked = () => {};
    this.pressing = false;
    this.letGo = () => {
      if (this.pressing) {
        this.pressing = false;
        this.tellSelection();
      }
    };
    document.addEventListener("pointerup", this.letGo);

    const editor = this;

    this.view = new EditorView(host, {
      dispatchTransaction: function (tr) {
        if (this.isDestroyed) {
          return;
        }
        if (!tr.selectionSet && !tr.docChanged) {
          const shown = shownSelection(editor.view, tr.doc);

          if (shown && !shown.eq(tr.selection)) {
            tr.setSelection(shown);
          }
        }
        this.updateState(this.state.apply(tr));
        if (tr.docChanged) {
          editor.uploads.mapped(tr.mapping);
          editor.changed(textOf(this.state.doc));
        }
        if ((tr.docChanged || tr.selectionSet) && this.hasFocus()) {
          editor.caretMoved(this.state.selection.head);
        }
        if (tr.docChanged || tr.selectionSet || tr.storedMarksSet) {
          editor.tellSelection();
        }
      },
      nodeViews: {
        figure: (node) => new FigureView(node, () => editor.labels),
      },
      state: EditorState.create({ schema, plugins: this.plugins() }),
    });
  }

  plugins() {
    return [
      new Plugin({
        props: {
          handleKeyDown: (view) => {
            const shown = shownSelection(view, view.state.doc);

            if (shown && !shown.eq(view.state.selection)) {
              view.dispatch(view.state.tr.setSelection(shown));
            }

            return false;
          },
          handleDOMEvents: {
            pointerdown: () => {
              this.pressing = true;
              this.selectionShown("");

              return false;
            },
            blur: (_, event) => {
              if (!event.relatedTarget?.closest?.(".formatting, .adding")) {
                this.selectionShown("");
              }

              return false;
            },
            focus: (view) => {
              this.presence.holdTheCaret();

              return focusedWhereTheBrowserPutIt(view);
            },
          },
          handleClick: (view, pos, event) => openedLink(view, pos, event),
          handleClickOn: (view, _, node, at, event) =>
            clickedFigure(view, node, at, event),
          handlePaste: (view, event) =>
            this.uploads.given(
              event.clipboardData?.files,
              view.state.selection.from,
            ),
          handleDrop: (view, event) => {
            const dropped = view.posAtCoords({
              left: event.clientX,
              top: event.clientY,
            });

            return this.uploads.given(
              event.dataTransfer?.files,
              dropped?.pos ?? view.state.selection.from,
            );
          },
          handleTextInput: (view, from, to, text) =>
            this.quotationMarks.typed(view, from, to, text),
          clipboardTextSerializer: (slice) => writtenText(slice.content),
        },
      }),
      markingWhereWritten(),
      captioning(),
      this.uploads.plugin(),
      ySyncPlugin(this.fragment),
      this.presence.cursors(),
      yUndoPlugin(),
      keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
      keymap({
        "Mod-i": toggleMark(schema.marks.em),
        "Mod-b": toggleMark(schema.marks.strong),
        "Mod-Shift-2": (state, dispatch) =>
          this.quotationMarks.quoting(state, dispatch),
        "Mod-k": (state) => {
          if (state.selection.empty) {
            return false;
          }
          this.linkAsked();

          return true;
        },
        "Alt-F10": (state) => {
          if (state.selection.empty && !onEmptyLine(state)) {
            return false;
          }
          this.toToolbar();

          return true;
        },
      }),
      keymap({
        Enter: (state, dispatch) =>
          leftCaption(state, dispatch) || this.walk.breakAtEnd(state, dispatch),
        Backspace: (state, dispatch) => backIntoFigure(state, dispatch),
        ...this.walk.keys(),
      }),
      keymap(baseKeymap),
    ];
  }

  tellSelection() {
    const inToolbar = document.activeElement?.closest?.(".formatting, .adding");
    if (this.pressing || !this.view || (!this.view.hasFocus() && !inToolbar)) {
      return;
    }
    this.selectionShown(selectionTold(this.view));
  }

  command(command) {
    command(this.view.state, (tr) => this.view.dispatch(tr));
    this.tellSelection();
  }

  redraw() {
    if (this.view && !this.view.isDestroyed) {
      this.view.dispatch(this.view.state.tr.setMeta("uploads", true));
      this.tellSelection();
    }
  }

  onChanged(changed) {
    this.changed = changed;

    if (this.sync.synced) {
      changed(textOf(this.view.state.doc));
    }
  }

  onBreaking(breaking) {
    this.walk.breaking = breaking;
  }

  onCaretMoved(caretMoved) {
    this.caretMoved = caretMoved;
  }

  onPresent(present) {
    this.presence.onPresent(present);
  }

  onSelectionShown(shown) {
    this.selectionShown = shown;
  }

  onToToolbar(toToolbar) {
    this.toToolbar = toToolbar;
  }

  onSaving(saving) {
    this.sync.onSaving(saving);
  }

  onLeaving(leaving) {
    this.walk.leaving = leaving;
  }

  onSelecting(selecting) {
    this.walk.selecting = selecting;
  }

  toggleBold() {
    this.command(toggleMark(schema.marks.strong));
  }

  toggleItalic() {
    this.command(toggleMark(schema.marks.em));
  }

  quoteSelection() {
    this.command((state, dispatch) =>
      this.quotationMarks.quoting(state, dispatch),
    );
  }

  clearFormatting() {
    this.command(clearing);
  }

  setQuotationMarks(opening, closing) {
    this.quotationMarks.set(opening, closing);
  }

  onLinkAsked(linkAsked) {
    this.linkAsked = linkAsked;
  }

  setLink(typed) {
    const href = linkTyped(typed);
    if (!href) {
      return false;
    }
    this.command(linking(href));

    return true;
  }

  removeLink() {
    this.command(unlinking);
  }

  onImagesGiven(imagesGiven) {
    this.uploads.imagesGiven = imagesGiven;
  }

  setLabels(told) {
    this.labels = JSON.parse(told);
  }

  askedForImage() {
    return this.uploads.waitFor(this.view.state.selection.from);
  }

  insertFigure(asset, ticket) {
    const at = this.uploads.placeOf(ticket);
    if (at === undefined || !assetOf(asset)) {
      return;
    }
    this.view.dispatch(placed(this.view.state, asset, at));
  }

  forgetWaiting(ticket) {
    this.uploads.forget(ticket);
  }

  uploadStarted(ticket, nth, name, label) {
    this.uploads.started(ticket, nth, name, label);
  }

  uploadProgress(ticket, nth, fraction) {
    this.uploads.progressed(ticket, nth, fraction);
  }

  uploadRefused(ticket, nth, told) {
    this.uploads.refused(ticket, nth, told);
  }

  uploadDone(ticket, nth) {
    this.uploads.done(ticket, nth);
  }

  setFigureSize(size) {
    this.command(sizing(size));
  }

  removeFigure() {
    this.command(removing);
    this.view.focus();
  }

  writtenText() {
    return writtenText(this.view.state.doc);
  }

  seed(text) {
    const { state } = this.view;

    this.view.dispatch(
      state.tr.insertText(text, Selection.atEnd(state.doc).from),
    );
    focusEnd(this.view);
  }

  landAt(at) {
    this.sync.whenSynced(() => landed(this.view, at));
  }

  arriveWhenSynced(towards) {
    this.sync.whenSynced(() => arrived(this.view, towards));
  }

  focusStart() {
    focusStart(this.view);
  }

  focusEnd() {
    focusEnd(this.view);
  }

  focus() {
    this.view.focus();
  }

  destroy() {
    document.removeEventListener("pointerup", this.letGo);
    this.presence.destroy();
    this.view.destroy();
    this.sync.destroy();
    this.doc.destroy();
  }
}
