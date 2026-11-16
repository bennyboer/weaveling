import { TextSelection } from "prosemirror-state";

import { schema } from "./schema.js";

const OPENS_AFTER = /[\s([{—–\-/]$/;

export function marked(state, mark) {
  const { from, to } = state.selection;
  let all = true;
  state.doc.nodesBetween(from, to, (node) => {
    if (node.isText && !mark.isInSet(node.marks)) {
      all = false;
    }
  });

  return all;
}

export function clearing(state, dispatch) {
  const { from, to } = state.selection;
  dispatch(
    state.tr
      .removeMark(from, to, schema.marks.strong)
      .removeMark(from, to, schema.marks.em)
      .removeMark(from, to, schema.marks.link),
  );

  return true;
}

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

export class QuotationMarks {
  constructor() {
    this.marks = { opening: "“", closing: "”" };
  }

  set(opening, closing) {
    this.marks = { opening, closing };
  }

  inserted(tr, at, side, marks) {
    const written = this.marks[side];
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

  quoting(state, dispatch) {
    const { from, to, empty, $from } = state.selection;
    const tr = state.tr;

    if (empty) {
      const before = $from.parent.textBetween(
        0,
        $from.parentOffset,
        undefined,
        "￼",
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

  typed(view, from, to, text) {
    if (text === "'") {
      view.dispatch(view.state.tr.insertText("’", from, to));

      return true;
    }
    if (text !== '"') {
      return false;
    }

    return this.quoting(view.state, view.dispatch);
  }
}
