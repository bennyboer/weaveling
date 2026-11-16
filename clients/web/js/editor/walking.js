import { NodeSelection, Selection, TextSelection } from "prosemirror-state";

const BY_PARAGRAPH = /Mac|iP(hone|[oa]d)/.test(navigator.platform)
  ? "Alt"
  : "Ctrl";

function cursor(state, view) {
  const shown = view.root.getSelection();

  if (shown && shown.rangeCount > 0 && view.dom.contains(shown.focusNode)) {
    return shown.isCollapsed
      ? view.posAtDOM(shown.focusNode, shown.focusOffset)
      : null;
  }

  return state.selection.empty ? state.selection.from : null;
}

function wholly(state) {
  const { doc, selection } = state;

  return (
    !selection.empty &&
    selection.from <= Selection.atStart(doc).from &&
    selection.to >= Selection.atEnd(doc).to
  );
}

export class Walk {
  constructor() {
    this.leaving = () => false;
    this.selecting = () => false;
    this.breaking = null;
  }

  keys() {
    return {
      ArrowUp: (state, _, view) => this.leave(state, view, "before", "up"),
      ArrowLeft: (state, _, view) => this.leave(state, view, "before", "left"),
      ArrowDown: (state, _, view) => this.leave(state, view, "after", "down"),
      ArrowRight: (state, _, view) => this.leave(state, view, "after", "right"),
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
    };
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
        ? this.selecting(wholly(state) ? `beyond-${towards}` : towards)
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

  selectBeyond(state, view, towards, moving) {
    const { doc, selection } = state;
    const atEdge =
      view.endOfTextblock(moving) &&
      doc.resolve(selection.head).index(0) ===
        (towards === "before" ? 0 : doc.childCount - 1);

    return (
      atEdge && this.selecting(wholly(state) ? `beyond-${towards}` : towards)
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
    const at = cursor(state, view);

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
}

export function focusedWhereTheBrowserPutIt(view) {
  if (!view.focused) {
    view.domObserver.stop();
    view.dom.classList.add("ProseMirror-focused");
    view.domObserver.start();
    view.focused = true;
  }

  return false;
}

export function focusAt(view, selection) {
  view.dispatch(
    view.state.tr.setSelection(TextSelection.near(selection.$from)),
  );
  view.focus();
  view.input.lastFocus = 0;
}

export function focusStart(view) {
  focusAt(view, Selection.atStart(view.state.doc));
}

export function focusEnd(view) {
  focusAt(view, Selection.atEnd(view.state.doc));
}

function lineOfCaret(view) {
  const { node } = view.domAtPos(view.state.selection.head);

  return node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
}

export function landed(view, at) {
  const focused = document.activeElement;

  if (focused && focused !== document.body) {
    return;
  }

  const { doc } = view.state;
  const caret = Math.min(Math.max(at, 0), doc.content.size);

  focusAt(view, TextSelection.near(doc.resolve(caret)));
  lineOfCaret(view)?.scrollIntoView({ block: "center" });
}

export function landedOnAsset(view, asset) {
  const focused = document.activeElement;

  if (focused && focused !== document.body) {
    return;
  }

  let found;
  view.state.doc.descendants((node, at) => {
    if (found === undefined && node.type.name === "figure" && node.attrs.asset === asset) {
      found = at;
    }

    return found === undefined;
  });
  if (found === undefined) {
    landed(view, 0);
    return;
  }

  view.dispatch(
    view.state.tr.setSelection(NodeSelection.create(view.state.doc, found)),
  );
  view.focus();
  view.input.lastFocus = 0;
  view.nodeDOM(found)?.scrollIntoView({ block: "center" });
}

export function arrived(view, towards) {
  if (towards === "before") {
    focusEnd(view);
  } else {
    focusStart(view);
  }
  lineOfCaret(view)?.scrollIntoView({ block: "nearest" });
}
