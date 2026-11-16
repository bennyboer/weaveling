import * as Y from "yjs";
import {
  EditorState,
  NodeSelection,
  Plugin,
  Selection,
  TextSelection,
} from "prosemirror-state";
import { Decoration, DecorationSet, EditorView } from "prosemirror-view";
import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";
import { baseKeymap, toggleMark } from "prosemirror-commands";
import { keymap } from "prosemirror-keymap";
import {
  defaultCursorBuilder,
  defaultSelectionBuilder,
  redo,
  undo,
  yCursorPlugin,
  ySyncPlugin,
  yUndoPlugin,
} from "y-prosemirror";

import { SceneSync } from "./sync.js";

export { SyncConnection } from "./sync.js";

const FRAGMENT = "prose";

const COLOUR = /^#[0-9a-fA-F]{6}$/;
const COLOUR_UNTOLD = "#ffa500";
const LONGEST_NAME = 64;

let holdingTheCaret = null;

const BY_PARAGRAPH = /Mac|iP(hone|[oa]d)/.test(navigator.platform)
  ? "Alt"
  : "Ctrl";

const LINKABLE = /^(https?:\/\/[^\s]+|mailto:[^\s]+)$/i;
const HAS_SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const AN_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

function safeLink(href) {
  const told = String(href ?? "").trim();

  return LINKABLE.test(told) ? told : null;
}

function linkTyped(typed) {
  const told = String(typed ?? "").trim();
  if (told === "" || HAS_SCHEME.test(told)) {
    return safeLink(told);
  }

  return safeLink(AN_EMAIL.test(told) ? `mailto:${told}` : `https://${told}`);
}

const AN_ASSET = /^[A-Za-z0-9_]{1,64}$/;
const SIZES = ["small", "medium", "full"];
const SHOWN_WIDE = { small: 768, medium: 1536, full: 1536 };

function sizeOf(told) {
  return SIZES.includes(told) ? told : "full";
}

function assetOf(told) {
  const asset = String(told ?? "");

  return AN_ASSET.test(asset) ? asset : "";
}

const figure = {
  attrs: { asset: { default: "" }, size: { default: "full" } },
  content: "text*",
  marks: "",
  group: "block",
  defining: true,
  parseDOM: [
    {
      tag: "figure[data-asset]",
      contentElement: "figcaption",
      getAttrs: (dom) => {
        const asset = assetOf(dom.dataset.asset);

        return asset ? { asset, size: sizeOf(dom.dataset.size) } : false;
      },
    },
  ],
  toDOM: (node) => [
    "figure",
    {
      class: `figure ${sizeOf(node.attrs.size)}`,
      "data-asset": assetOf(node.attrs.asset),
      "data-size": sizeOf(node.attrs.size),
    },
    ["figcaption", 0],
  ],
};

const ROOM_FOR_ADDING = 3.5;

function placeholderEnd(paragraph) {
  if (!(paragraph instanceof HTMLElement)) {
    return null;
  }
  const shown = getComputedStyle(paragraph, "::before");
  if (shown.content === "none" || shown.content === "normal") {
    return null;
  }
  const said = shown.content.replace(/^"|"$/g, "");
  const measure = document.createElement("span");
  measure.textContent = said;
  for (const each of [
    "fontFamily",
    "fontSize",
    "fontWeight",
    "fontStyle",
    "letterSpacing",
    "wordSpacing",
    "fontFeatureSettings",
  ]) {
    measure.style[each] = shown[each];
  }
  measure.style.position = "absolute";
  measure.style.display = "block";
  measure.style.textWrap = shown.textWrap;
  measure.style.visibility = "hidden";
  measure.style.left = "0";
  measure.style.top = "0";
  measure.style.width = `${paragraph.clientWidth}px`;
  document.body.append(measure);
  const text = document.createRange();
  text.selectNodeContents(measure);
  const lines = text.getClientRects();
  const last = lines[lines.length - 1];
  const start = measure.getBoundingClientRect();
  measure.remove();
  if (!last) {
    return null;
  }
  const at = paragraph.getBoundingClientRect();
  const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
  const left = at.left + (last.right - start.left);
  const top = at.top + (last.top - start.top);
  const bottom = at.top + (last.bottom - start.top);
  if (left + ROOM_FOR_ADDING * rem <= at.right) {
    return { left, top, bottom };
  }

  const below = bottom + 0.5 * rem;

  return { left: at.left - 0.75 * rem, top: below, bottom: below + 2.75 * rem };
}

class FigureView {
  constructor(node, labels) {
    this.node = node;
    this.labels = labels;
    this.dom = document.createElement("figure");
    this.picture = document.createElement("img");
    this.picture.alt = "";
    this.picture.draggable = false;
    this.picture.contentEditable = "false";
    this.picture.addEventListener("error", () => {
      this.dom.classList.add("missing");
      this.dom.dataset.missing = labels().missing ?? "";
    });
    this.contentDOM = document.createElement("figcaption");
    this.dom.append(this.picture, this.contentDOM);
    this.show(node);
  }

  show(node) {
    const asset = assetOf(node.attrs.asset);
    const size = sizeOf(node.attrs.size);
    this.dom.classList.add("figure");
    for (const each of SIZES) {
      this.dom.classList.toggle(each, each === size);
    }
    this.dom.dataset.asset = asset;
    this.dom.dataset.size = size;
    this.dom.classList.toggle("missing", !asset);
    this.contentDOM.classList.toggle("empty", node.content.size === 0);
    this.contentDOM.dataset.placeholder = this.labels().caption ?? "";
    const src = asset
      ? `/api/assets/${asset}/content?width=${SHOWN_WIDE[size]}`
      : "";
    if (this.picture.getAttribute("src") !== src) {
      this.picture.setAttribute("src", src);
    }
  }

  update(node) {
    if (node.type !== this.node.type) {
      return false;
    }
    this.node = node;
    this.show(node);

    return true;
  }

  ignoreMutation(mutation) {
    return !this.contentDOM.contains(mutation.target);
  }
}

const schema = new Schema({
  nodes: addListNodes(basic.spec.nodes, "paragraph block*", "block")
    .remove("image")
    .addToEnd("figure", figure),
  marks: basic.spec.marks
    .update("link", {
      attrs: { href: {} },
      inclusive: false,
      parseDOM: [
        {
          tag: "a[href]",
          getAttrs: (dom) => {
            const href = safeLink(dom.getAttribute("href"));

            return href ? { href } : false;
          },
        },
      ],
      toDOM: (mark) => {
        const href = safeLink(mark.attrs.href);

        return [
          "a",
          href
            ? { href, title: href, rel: "noopener noreferrer nofollow" }
            : {},
          0,
        ];
      },
    })
    .addToStart("quote", {
      attrs: { side: { default: "opening" } },
      inclusive: false,
      parseDOM: [
        {
          tag: "span.quote",
          getAttrs: (dom) => ({
            side: dom.dataset.side === "closing" ? "closing" : "opening",
          }),
        },
      ],
      toDOM: (mark) => [
        "span",
        { class: "quote", "data-side": mark.attrs.side },
        0,
      ],
    })
    .addToEnd("speech", { toDOM: () => ["span", 0] }),
});

const OPENS_AFTER = /[\s([{\u2014\u2013\-/]$/;

function quoteRuns(doc) {
  const runs = [];

  doc.descendants((node, position) => {
    if (!node.isTextblock) {
      return true;
    }
    let run = null;
    node.forEach((child, offset) => {
      const from = position + 1 + offset;
      const quote = schema.marks.quote.isInSet(child.marks);
      if (quote && run && run.side === quote.attrs.side && run.to === from) {
        run.to = from + child.nodeSize;
        return;
      }
      run = quote
        ? { from, to: from + child.nodeSize, side: quote.attrs.side }
        : null;
      if (run) {
        runs.push(run);
      }
    });

    return false;
  });

  return runs;
}

function writtenText(doc) {
  const blocks = [];

  doc.descendants((node) => {
    if (node.type.name === "figure") {
      return false;
    }
    if (!node.isTextblock) {
      return true;
    }
    blocks.push(node.textContent);

    return false;
  });

  return blocks.join("\n");
}

function uploadsShown(editor) {
  return new Plugin({
    props: {
      decorations: (state) => editor.uploadsAt(state.doc),
    },
  });
}

function uploadPlace(doc, at) {
  const $at = doc.resolve(Math.min(at, doc.content.size));
  if ($at.depth === 0) {
    return $at.pos;
  }
  const empty = $at.parent.type.name === "paragraph" && $at.parent.content.size === 0;

  return empty ? $at.before(1) : $at.after(1);
}

function captioning() {
  return new Plugin({
    props: {
      decorations: (state) => {
        const { selection } = state;
        if (
          selection instanceof NodeSelection &&
          selection.node.type.name === "figure"
        ) {
          return DecorationSet.create(state.doc, [
            Decoration.node(selection.from, selection.to, {
              class: "captioning",
            }),
          ]);
        }
        const { $from } = selection;
        if ($from.parent.type.name === "figure") {
          return DecorationSet.create(state.doc, [
            Decoration.node($from.before(), $from.after(), {
              class: "captioning",
            }),
          ]);
        }

        return DecorationSet.empty;
      },
    },
  });
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
          Decoration.node($head.before(1), $head.after(1), {
            class: "where-written",
          }),
        ]);
      },
    },
  });
}

function asTold(user) {
  return {
    name: String(user.name ?? "").slice(0, LONGEST_NAME),
    color: COLOUR.test(user.color) ? user.color : COLOUR_UNTOLD,
  };
}

function textOf(doc) {
  const blocks = [];

  doc.descendants((node) => {
    if (node.type.name === "figure") {
      return false;
    }
    if (!node.isTextblock) {
      return true;
    }
    blocks.push(node.textBetween(0, node.content.size, " ", " "));

    return false;
  });

  return blocks.join(" ");
}

export class ProseEditor {
  constructor(host, scene, account, onConnected, connection) {
    this.doc = new Y.Doc();
    this.fragment = this.doc.getXmlFragment(FRAGMENT);

    this.sync = new SceneSync(connection, scene, this.doc, onConnected);
    this.connection = connection;
    this.account = account;
    this.sync.awareness.setLocalState(null);
    this.present = () => {};
    this.tellPresent = () => this.present(JSON.stringify(this.othersHere()));
    this.sync.awareness.on("change", this.tellPresent);

    this.leaving = () => false;
    this.selecting = () => false;
    this.quotationMarks = { opening: "\u201c", closing: "\u201d" };
    this.breaking = null;
    this.changed = () => {};
    this.caretMoved = () => {};
    this.selectionShown = () => {};
    this.toToolbar = () => {};
    this.linkAsked = () => {};
    this.imagesGiven = () => {};
    this.labels = {};
    this.waiting = new Map();
    this.uploads = new Map();
    this.ended = new Set();
    this.waited = 0;
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
          const shown = editor.shownSelection(tr.doc);

          if (shown && !shown.eq(tr.selection)) {
            tr.setSelection(shown);
          }
        }
        this.updateState(this.state.apply(tr));
        if (tr.docChanged) {
          editor.mapWaiting(tr.mapping);
        }

        if (tr.docChanged) {
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
                  this.holdTheCaret();

                  return this.focusedWhereTheBrowserPutIt(view);
                },
              },
              handleClick: (view, pos, event) =>
                this.openedLink(view, pos, event),
              handleClickOn: (view, _, node, at, event) =>
                this.chosenFigure(view, node, at, event),
              handlePaste: (view, event) =>
                this.givenImages(
                  view,
                  event.clipboardData?.files,
                  view.state.selection.from,
                ),
              handleDrop: (view, event) => {
                const dropped = view.posAtCoords({
                  left: event.clientX,
                  top: event.clientY,
                });

                return this.givenImages(
                  view,
                  event.dataTransfer?.files,
                  dropped?.pos ?? view.state.selection.from,
                );
              },
              handleTextInput: (view, from, to, text) =>
                this.typedMark(view, from, to, text),
              clipboardTextSerializer: (slice) => writtenText(slice.content),
            },
          }),
          markingWhereWritten(),
          captioning(),
          uploadsShown(this),
          ySyncPlugin(this.fragment),
          yCursorPlugin(this.sync.awareness, {
            cursorBuilder: (_, client) =>
              defaultCursorBuilder(asTold(this.whoIs(client)), client),
            selectionBuilder: (_, client) =>
              defaultSelectionBuilder(asTold(this.whoIs(client)), client),
          }),
          yUndoPlugin(),
          keymap({ "Mod-z": undo, "Mod-y": redo, "Mod-Shift-z": redo }),
          keymap({
            "Mod-i": toggleMark(schema.marks.em),
            "Mod-b": toggleMark(schema.marks.strong),
            "Mod-Shift-2": (state, dispatch) => this.quoting(state, dispatch),
            "Mod-k": (state) => {
              if (state.selection.empty) {
                return false;
              }
              this.linkAsked();

              return true;
            },
            "Alt-F10": (state) => {
              const { $from, empty } = state.selection;
              const onEmptyLine =
                $from.parent.type.name === "paragraph" &&
                $from.parent.content.size === 0;
              if (empty && !onEmptyLine) {
                return false;
              }
              this.toToolbar();

              return true;
            },
          }),
          keymap({
            Enter: (state, dispatch) =>
              this.leftCaption(state, dispatch) ||
              this.breakAtEnd(state, dispatch),
            Backspace: (state, dispatch) =>
              this.backIntoFigure(state, dispatch),
            ArrowUp: (state, _, view) =>
              this.leave(state, view, "before", "up"),
            ArrowLeft: (state, _, view) =>
              this.leave(state, view, "before", "left"),
            ArrowDown: (state, _, view) =>
              this.leave(state, view, "after", "down"),
            ArrowRight: (state, _, view) =>
              this.leave(state, view, "after", "right"),
            "Shift-ArrowUp": (state, _, view) =>
              this.selectBeyond(state, view, "before", "up"),
            "Shift-ArrowDown": (state, _, view) =>
              this.selectBeyond(state, view, "after", "down"),
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
    dispatch?.(
      state.tr
        .setSelection(TextSelection.create(doc, anchor, to))
        .scrollIntoView(),
    );

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
      $from.parent.type.name !== "paragraph" ||
      doc.childCount < 2 ||
      $from.parent.content.size > 0
    ) {
      return false;
    }

    dispatch(state.tr.delete($from.before(1), $from.after(1)));
    this.breaking();

    return true;
  }

  leftCaption(state, dispatch) {
    const { $from } = state.selection;
    if ($from.parent.type.name !== "figure") {
      return false;
    }
    const after = $from.after();
    const tr = state.tr;
    if (tr.doc.nodeAt(after)?.type.name !== "paragraph") {
      tr.insert(after, schema.nodes.paragraph.create());
    }
    dispatch(
      tr.setSelection(TextSelection.create(tr.doc, after + 1)).scrollIntoView(),
    );

    return true;
  }

  backIntoFigure(state, dispatch) {
    const { $from, empty } = state.selection;
    if (
      !empty ||
      $from.parent.type.name !== "figure" ||
      $from.parentOffset > 0
    ) {
      return false;
    }
    dispatch(
      state.tr.setSelection(NodeSelection.create(state.doc, $from.before())),
    );

    return true;
  }

  onCaretMoved(caretMoved) {
    this.caretMoved = caretMoved;
  }

  onSelectionShown(shown) {
    this.selectionShown = shown;
  }

  onToToolbar(toToolbar) {
    this.toToolbar = toToolbar;
  }

  onLinkAsked(linkAsked) {
    this.linkAsked = linkAsked;
  }

  onImagesGiven(imagesGiven) {
    this.imagesGiven = imagesGiven;
  }

  setLabels(told) {
    this.labels = JSON.parse(told);
  }

  waitFor(at) {
    this.waited += 1;
    this.waiting.set(this.waited, at);

    return this.waited;
  }

  mapWaiting(mapping) {
    for (const [ticket, at] of this.waiting) {
      this.waiting.set(ticket, mapping.map(at));
    }
  }

  givenImages(view, files, at) {
    const images = Array.from(files ?? []).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (images.length === 0) {
      return false;
    }
    this.imagesGiven(images, this.waitFor(at));

    return true;
  }

  askedForImage() {
    return this.waitFor(this.view.state.selection.from);
  }

  insertFigure(asset, ticket) {
    const at = this.waiting.get(ticket);
    if (at === undefined || !assetOf(asset)) {
      return;
    }
    const { state } = this.view;
    const placed = schema.nodes.figure.create({ asset, size: "full" });
    const $at = state.doc.resolve(Math.min(at, state.doc.content.size));
    const tr = state.tr;
    if (
      $at.parent.type.name === "paragraph" &&
      $at.parent.content.size === 0 &&
      $at.depth > 0
    ) {
      tr.replaceWith($at.before(), $at.after(), placed);
    } else {
      const after = $at.depth > 0 ? $at.after(1) : $at.pos;
      tr.insert(after, placed);
    }
    this.view.dispatch(tr.scrollIntoView());
  }

  forgetWaiting(ticket) {
    this.ended.add(ticket);
    this.letGoOfTicket(ticket);
  }

  letGoOfTicket(ticket) {
    const held = [...this.uploads.values()].some((upload) => upload.ticket === ticket);
    if (!held && this.ended.has(ticket)) {
      this.ended.delete(ticket);
      this.waiting.delete(ticket);
    }
  }

  uploadStarted(ticket, nth, name, label) {
    this.uploads.set(`${ticket}:${nth}`, { ticket, name, label, fraction: 0, refused: null });
    this.redrawUploads();
  }

  uploadProgress(ticket, nth, fraction) {
    const upload = this.uploads.get(`${ticket}:${nth}`);
    if (!upload) {
      return;
    }
    const shown = Math.round(Math.min(Math.max(fraction, 0), 1) * 100);
    if (shown !== Math.round(upload.fraction * 100)) {
      upload.fraction = shown / 100;
      this.redrawUploads();
    }
  }

  uploadRefused(ticket, nth, told) {
    const upload = this.uploads.get(`${ticket}:${nth}`);
    if (upload) {
      upload.refused = told;
      this.redrawUploads();
    }
  }

  uploadDone(ticket, nth) {
    this.uploads.delete(`${ticket}:${nth}`);
    this.letGoOfTicket(ticket);
    this.redrawUploads();
  }

  redrawUploads() {
    if (this.view && !this.view.isDestroyed) {
      this.view.dispatch(this.view.state.tr.setMeta("uploads", true));
      this.tellSelection();
    }
  }

  uploadsAt(doc) {
    if (this.uploads.size === 0) {
      return DecorationSet.empty;
    }
    const shown = [];
    for (const [key, upload] of this.uploads) {
      const at = this.waiting.get(upload.ticket);
      if (at === undefined) {
        continue;
      }
      const state = upload.refused === null ? Math.round(upload.fraction * 100) : "refused";
      shown.push(
        Decoration.widget(uploadPlace(doc, at), () => this.uploadShown(key, upload), {
          key: `upload-${key}-${state}`,
          side: 1,
          ignoreSelection: true,
          stopEvent: () => true,
        }),
      );
    }

    return DecorationSet.create(doc, shown);
  }

  uploadShown(key, upload) {
    const shown = document.createElement("div");
    shown.className = "uploading";
    shown.contentEditable = "false";
    const name = document.createElement("span");
    name.className = "uploading-name";
    name.textContent = upload.label;
    if (upload.refused !== null) {
      shown.classList.add("refused");
      shown.setAttribute("role", "alert");
      name.textContent = upload.name;
      const told = document.createElement("span");
      told.className = "uploading-refused";
      told.textContent = upload.refused;
      const dismiss = document.createElement("button");
      dismiss.type = "button";
      dismiss.className = "dismiss";
      dismiss.textContent = "\u00d7";
      dismiss.setAttribute("aria-label", this.labels.dismiss ?? "");
      dismiss.addEventListener("mousedown", (event) => event.preventDefault());
      dismiss.addEventListener("click", () => {
        const [ticket, nth] = key.split(":").map(Number);
        this.uploadDone(ticket, nth);
        this.view.focus();
      });
      shown.append(name, told, dismiss);

      return shown;
    }
    const bar = document.createElement("div");
    bar.className = "uploading-bar";
    bar.setAttribute("role", "progressbar");
    bar.setAttribute("aria-label", upload.label);
    bar.setAttribute("aria-valuemin", "0");
    bar.setAttribute("aria-valuemax", "100");
    bar.setAttribute("aria-valuenow", String(Math.round(upload.fraction * 100)));
    bar.style.setProperty("--done", `${upload.fraction * 100}%`);
    shown.append(name, bar);

    return shown;
  }

  chosenFigure(view, node, at, event) {
    if (node.type.name !== "figure" || event.target?.tagName !== "IMG") {
      return false;
    }
    view.dispatch(
      view.state.tr.setSelection(NodeSelection.create(view.state.doc, at)),
    );
    view.focus();

    return true;
  }

  setFigureSize(size) {
    const { selection } = this.view.state;
    if (
      !(selection instanceof NodeSelection) ||
      selection.node.type.name !== "figure"
    ) {
      return;
    }
    const tr = this.view.state.tr.setNodeMarkup(selection.from, undefined, {
      ...selection.node.attrs,
      size: sizeOf(size),
    });
    this.view.dispatch(
      tr.setSelection(NodeSelection.create(tr.doc, selection.from)),
    );
    this.tellSelection();
  }

  removeFigure() {
    const { selection } = this.view.state;
    if (
      !(selection instanceof NodeSelection) ||
      selection.node.type.name !== "figure"
    ) {
      return;
    }
    this.view.dispatch(this.view.state.tr.deleteSelection());
    this.view.focus();
  }

  linkAt(state) {
    const { from, to } = state.selection;
    let href;
    let one = true;
    state.doc.nodesBetween(from, to, (node) => {
      if (!node.isText) {
        return;
      }
      const link = schema.marks.link.isInSet(node.marks);
      const here = link ? safeLink(link.attrs.href) : null;
      if (href === undefined) {
        href = here;
      } else if (href !== here) {
        one = false;
      }
    });

    return one ? (href ?? null) : null;
  }

  setLink(typed) {
    const href = linkTyped(typed);
    if (!href) {
      return false;
    }
    this.command((state, dispatch) => {
      const { from, to } = state.selection;
      dispatch(
        state.tr
          .removeMark(from, to, schema.marks.link)
          .addMark(from, to, schema.marks.link.create({ href })),
      );

      return true;
    });

    return true;
  }

  removeLink() {
    this.command((state, dispatch) => {
      const { from, to } = state.selection;
      dispatch(state.tr.removeMark(from, to, schema.marks.link));

      return true;
    });
  }

  openedLink(view, pos, event) {
    if (!event.ctrlKey && !event.metaKey) {
      return false;
    }
    const node = view.state.doc.nodeAt(pos);
    const link =
      (node && schema.marks.link.isInSet(node.marks)) ||
      schema.marks.link.isInSet(view.state.doc.resolve(pos).marks());
    const href = link ? safeLink(link.attrs.href) : null;
    if (!href) {
      return false;
    }
    window.open(href, "_blank", "noopener,noreferrer");

    return true;
  }

  tellSelection() {
    const inToolbar = document.activeElement?.closest?.(".formatting, .adding");
    if (this.pressing || !this.view || (!this.view.hasFocus() && !inToolbar)) {
      return;
    }
    const { state } = this.view;
    const { from, to, empty } = state.selection;
    const row = this.view.dom.closest(".scene-row");
    if (!row) {
      this.selectionShown("");
      return;
    }
    const around = row.getBoundingClientRect();
    const chosen =
      state.selection instanceof NodeSelection ? state.selection.node : null;
    if (chosen?.type.name === "figure") {
      const shown = this.view.nodeDOM(from)?.getBoundingClientRect();
      if (shown) {
        this.selectionShown(
          JSON.stringify({
            above: shown.top - around.top,
            below: shown.bottom - around.top,
            across: shown.left + shown.width / 2 - around.left,
            bold: false,
            italic: false,
            link: null,
            figure: sizeOf(chosen.attrs.size),
          }),
        );
        return;
      }
    }
    if (empty && state.selection instanceof TextSelection) {
      const { $from } = state.selection;
      if (
        $from.parent.type.name === "paragraph" &&
        $from.parent.content.size === 0
      ) {
        const line =
          placeholderEnd(this.view.nodeDOM($from.before())) ??
          this.view.coordsAtPos(from);
        this.selectionShown(
          JSON.stringify({
            above: line.top - around.top,
            below: line.bottom - around.top,
            across: line.left - around.left,
            bold: false,
            italic: false,
            link: null,
            empty_line: true,
          }),
        );
        return;
      }
    }
    if (empty || !(state.selection instanceof TextSelection)) {
      this.selectionShown("");
      return;
    }
    const start = this.view.coordsAtPos(from);
    const end = this.view.coordsAtPos(to);
    const middle =
      start.top === end.top ? (start.left + end.left) / 2 : start.left;

    this.selectionShown(
      JSON.stringify({
        above: start.top - around.top,
        below: end.bottom - around.top,
        across: Math.min(Math.max(middle - around.left, 0), around.width),
        bold: this.marked(state, schema.marks.strong),
        italic: this.marked(state, schema.marks.em),
        link: this.linkAt(state),
      }),
    );
  }

  marked(state, mark) {
    const { from, to } = state.selection;
    let all = true;
    state.doc.nodesBetween(from, to, (node) => {
      if (node.isText && !mark.isInSet(node.marks)) {
        all = false;
      }
    });

    return all;
  }

  toggleBold() {
    this.command(toggleMark(schema.marks.strong));
  }

  toggleItalic() {
    this.command(toggleMark(schema.marks.em));
  }

  quoteSelection() {
    this.command((state, dispatch) => this.quoting(state, dispatch));
  }

  clearFormatting() {
    this.command((state, dispatch) => {
      const { from, to } = state.selection;
      dispatch(
        state.tr
          .removeMark(from, to, schema.marks.strong)
          .removeMark(from, to, schema.marks.em)
          .removeMark(from, to, schema.marks.link),
      );

      return true;
    });
  }

  command(command) {
    command(this.view.state, (tr) => this.view.dispatch(tr));
    this.tellSelection();
  }

  onPresent(present) {
    this.present = present;
    this.tellPresent();
  }

  holdTheCaret() {
    if (holdingTheCaret === this) {
      return;
    }
    holdingTheCaret?.letGoOfTheCaret();
    holdingTheCaret = this;
    this.sync.awareness.setLocalState(
      this.account ? { account: this.account } : {},
    );
  }

  letGoOfTheCaret() {
    this.sync.awareness.setLocalState(null);
  }

  othersHere() {
    const here = new Set();
    for (const [client, state] of this.sync.awareness.getStates()) {
      if (client !== this.doc.clientID && typeof state?.account === "string") {
        here.add(state.account);
      }
    }

    return [...here];
  }

  whoIs(client) {
    const account = this.sync.awareness.getStates().get(client)?.account;
    const person = this.connection.personOf(account);
    const color = person
      ? getComputedStyle(document.documentElement)
          .getPropertyValue(`--${person.tint}`)
          .trim()
      : "";

    return { name: person?.name ?? "", color };
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
      const line =
        node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
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
      const line =
        node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
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
  }

  inserted(tr, at, side, marks) {
    const written = this.quotationMarks[side];
    const quoted = schema.marks.quote
      .create({ side })
      .addToSet(
        marks.filter(
          (mark) =>
            mark.type !== schema.marks.quote &&
            mark.type !== schema.marks.speech,
        ),
      );
    tr.insert(at, schema.text(written, quoted));

    return written.length;
  }

  writtenText() {
    return writtenText(this.view.state.doc);
  }

  quoting(state, dispatch) {
    const { from, to, empty, $from } = state.selection;
    const tr = state.tr;

    if (empty) {
      const before = $from.parent.textBetween(
        0,
        $from.parentOffset,
        undefined,
        "\ufffc",
      );
      const opensHere = before === "" || OPENS_AFTER.test(before);
      const length = this.inserted(
        tr,
        from,
        opensHere ? "opening" : "closing",
        state.storedMarks ?? $from.marks(),
      );
      dispatch?.(
        tr
          .setSelection(TextSelection.create(tr.doc, from + length))
          .scrollIntoView(),
      );

      return true;
    }

    const runs = quoteRuns(state.doc);
    const opening = runs.find(
      (run) => run.side === "opening" && run.to === from,
    );
    const closing = runs.find(
      (run) => run.side === "closing" && run.from === to,
    );
    if (opening && closing) {
      tr.delete(closing.from, closing.to);
      tr.delete(opening.from, opening.to);
      dispatch?.(
        tr.setSelection(
          TextSelection.create(
            tr.doc,
            opening.from,
            opening.from + (to - from),
          ),
        ),
      );

      return true;
    }

    this.inserted(tr, to, "closing", state.doc.resolve(to).marks());
    const shift = this.inserted(
      tr,
      from,
      "opening",
      state.doc.resolve(from).marks(),
    );
    dispatch?.(
      tr.setSelection(TextSelection.create(tr.doc, from + shift, to + shift)),
    );

    return true;
  }

  typedMark(view, from, to, text) {
    if (text === "'") {
      view.dispatch(view.state.tr.insertText("\u2019", from, to));

      return true;
    }
    if (text !== '"') {
      return false;
    }

    return this.quoting(view.state, view.dispatch);
  }

  onSelecting(selecting) {
    this.selecting = selecting;
  }

  selectBeyond(state, view, towards, moving) {
    const { doc, selection } = state;
    const atEdge =
      view.endOfTextblock(moving) &&
      doc.resolve(selection.head).index(0) ===
        (towards === "before" ? 0 : doc.childCount - 1);

    return (
      atEdge &&
      this.selecting(this.wholly(state) ? `beyond-${towards}` : towards)
    );
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
      selection.from <= Selection.atStart(doc).from &&
      selection.to >= Selection.atEnd(doc).to;

    return everything && this.selecting("all");
  }

  leave(state, view, towards, moving) {
    const { doc } = state;
    const at = this.cursor(state, view);

    if (at === null) {
      return false;
    }

    const edge =
      towards === "before" ? Selection.atStart(doc) : Selection.atEnd(doc);
    const sideways = moving === "left" || moving === "right";
    const atEdge = sideways
      ? at === edge.from
      : view.endOfTextblock(moving) &&
        doc.resolve(at).index(0) ===
          (towards === "before" ? 0 : doc.childCount - 1);

    return atEdge && this.leaving(towards);
  }

  shownSelection(doc) {
    const view = this.view;
    const shown = view?.root.getSelection();

    if (!view || !view.hasFocus() || !shown || shown.rangeCount === 0) {
      return null;
    }
    if (view.state.selection instanceof NodeSelection) {
      return null;
    }
    if (
      !view.dom.contains(shown.anchorNode) ||
      !view.dom.contains(shown.focusNode)
    ) {
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
      return shown.isCollapsed
        ? view.posAtDOM(shown.focusNode, shown.focusOffset)
        : null;
    }

    return state.selection.empty ? state.selection.from : null;
  }

  seed(text) {
    const { state } = this.view;

    this.view.dispatch(
      state.tr.insertText(text, Selection.atEnd(state.doc).from),
    );
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

    this.view.dispatch(
      state.tr.setSelection(TextSelection.near(selection.$from)),
    );
    this.view.focus();
    this.view.input.lastFocus = 0;
  }

  destroy() {
    document.removeEventListener("pointerup", this.letGo);
    if (holdingTheCaret === this) {
      holdingTheCaret = null;
    }
    this.sync.awareness.off("change", this.tellPresent);
    this.view.destroy();
    this.sync.destroy();
    this.doc.destroy();
  }

  focus() {
    this.view.focus();
  }
}
