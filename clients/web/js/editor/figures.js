import { NodeSelection, Plugin, TextSelection } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

import { SHOWN_WIDE, SIZES, assetOf, schema, sizeOf } from "./schema.js";

export class FigureView {
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

export function chosenFigure(state) {
  const { selection } = state;

  return selection instanceof NodeSelection &&
    selection.node.type.name === "figure"
    ? selection.node
    : null;
}

export function captioning() {
  return new Plugin({
    props: {
      decorations: (state) => {
        const { selection } = state;
        if (chosenFigure(state)) {
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

export function clickedFigure(view, node, at, event) {
  if (node.type.name !== "figure" || event.target?.tagName !== "IMG") {
    return false;
  }
  view.dispatch(
    view.state.tr.setSelection(NodeSelection.create(view.state.doc, at)),
  );
  view.focus();

  return true;
}

export function placed(state, asset, at) {
  const figure = schema.nodes.figure.create({ asset, size: "full" });
  const $at = state.doc.resolve(Math.min(at, state.doc.content.size));
  const tr = state.tr;
  if (
    $at.parent.type.name === "paragraph" &&
    $at.parent.content.size === 0 &&
    $at.depth > 0
  ) {
    tr.replaceWith($at.before(), $at.after(), figure);
  } else {
    const after = $at.depth > 0 ? $at.after(1) : $at.pos;
    tr.insert(after, figure);
  }

  return tr.scrollIntoView();
}

export function sizing(size) {
  return (state, dispatch) => {
    const chosen = chosenFigure(state);
    if (!chosen) {
      return false;
    }
    const { from } = state.selection;
    const tr = state.tr.setNodeMarkup(from, undefined, {
      ...chosen.attrs,
      size: sizeOf(size),
    });
    dispatch(tr.setSelection(NodeSelection.create(tr.doc, from)));

    return true;
  };
}

export function removing(state, dispatch) {
  if (!chosenFigure(state)) {
    return false;
  }
  dispatch(state.tr.deleteSelection());

  return true;
}

export function leftCaption(state, dispatch) {
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

export function backIntoFigure(state, dispatch) {
  const { $from, empty } = state.selection;
  if (!empty || $from.parent.type.name !== "figure" || $from.parentOffset > 0) {
    return false;
  }
  dispatch(
    state.tr.setSelection(NodeSelection.create(state.doc, $from.before())),
  );

  return true;
}
