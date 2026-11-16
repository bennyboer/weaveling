import { NodeSelection, Plugin, TextSelection } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

import { chosenFigure } from "./figures.js";
import { marked } from "./formatting.js";
import { linkAt } from "./links.js";
import { schema, sizeOf } from "./schema.js";

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

export function selectionTold(view) {
  const { state } = view;
  const { from, to, empty } = state.selection;
  const row = view.dom.closest(".scene-row");
  if (!row) {
    return "";
  }
  const around = row.getBoundingClientRect();
  const chosen = chosenFigure(state);
  if (chosen) {
    const shown = view.nodeDOM(from)?.getBoundingClientRect();
    if (shown) {
      return JSON.stringify({
        above: shown.top - around.top,
        below: shown.bottom - around.top,
        across: shown.left + shown.width / 2 - around.left,
        bold: false,
        italic: false,
        link: null,
        figure: sizeOf(chosen.attrs.size),
      });
    }
  }
  if (empty && state.selection instanceof TextSelection) {
    const { $from } = state.selection;
    if (
      $from.parent.type.name === "paragraph" &&
      $from.parent.content.size === 0
    ) {
      const line =
        placeholderEnd(view.nodeDOM($from.before())) ?? view.coordsAtPos(from);

      return JSON.stringify({
        above: line.top - around.top,
        below: line.bottom - around.top,
        across: line.left - around.left,
        bold: false,
        italic: false,
        link: null,
        empty_line: true,
      });
    }
  }
  if (empty || !(state.selection instanceof TextSelection)) {
    return "";
  }
  const start = view.coordsAtPos(from);
  const end = view.coordsAtPos(to);
  const middle =
    start.top === end.top ? (start.left + end.left) / 2 : start.left;

  return JSON.stringify({
    above: start.top - around.top,
    below: end.bottom - around.top,
    across: Math.min(Math.max(middle - around.left, 0), around.width),
    bold: marked(state, schema.marks.strong),
    italic: marked(state, schema.marks.em),
    link: linkAt(state),
  });
}

export function onEmptyLine(state) {
  const { $from } = state.selection;

  return (
    $from.parent.type.name === "paragraph" && $from.parent.content.size === 0
  );
}

export function shownSelection(view, doc) {
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

export function markingWhereWritten() {
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
