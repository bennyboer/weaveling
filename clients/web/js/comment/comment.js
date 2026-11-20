import {
  baseKeymap,
  chainCommands,
  exitCode,
  splitBlock,
  toggleMark,
} from "prosemirror-commands";
import { history, redo, undo } from "prosemirror-history";
import { InputRule, inputRules } from "prosemirror-inputrules";
import { keymap } from "prosemirror-keymap";
import { Schema } from "prosemirror-model";
import { EditorState, TextSelection } from "prosemirror-state";
import { EditorView } from "prosemirror-view";

import { linkTyped, safeLink } from "../editor/schema.js";

const BOLD = /^(bold(er)?|[6-9]\d\d)$/;

export const schema = new Schema({
  nodes: {
    doc: { content: "block+" },
    paragraph: {
      content: "inline*",
      group: "block",
      parseDOM: [{ tag: "p" }, { tag: "div" }, { tag: "h1, h2, h3, h4, h5, h6" }],
      toDOM: () => ["p", 0],
    },
    bullet_list: {
      content: "list_item+",
      group: "block",
      parseDOM: [{ tag: "ul" }],
      toDOM: () => ["ul", 0],
    },
    ordered_list: {
      content: "list_item+",
      group: "block",
      parseDOM: [{ tag: "ol" }],
      toDOM: () => ["ol", 0],
    },
    list_item: {
      content: "inline*",
      defining: true,
      parseDOM: [{ tag: "li" }],
      toDOM: () => ["li", 0],
    },
    text: { group: "inline" },
    hard_break: {
      inline: true,
      group: "inline",
      selectable: false,
      parseDOM: [{ tag: "br" }],
      toDOM: () => ["br"],
    },
  },
  marks: {
    strong: {
      parseDOM: [
        { tag: "strong" },
        { tag: "b" },
        { style: "font-weight", getAttrs: (weight) => BOLD.test(weight) && null },
      ],
      toDOM: () => ["strong", 0],
    },
    em: {
      parseDOM: [{ tag: "em" }, { tag: "i" }, { style: "font-style=italic" }],
      toDOM: () => ["em", 0],
    },
    link: {
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
      toDOM: (mark) => [
        "a",
        { href: mark.attrs.href, rel: "noopener noreferrer nofollow" },
        0,
      ],
    },
  },
});

/// The runs of one line, as the comment keeps them.
function runsOf(line) {
  const runs = [];
  line.forEach((child) => {
    if (child.type === schema.nodes.hard_break) {
      runs.push({ kind: "break" });
      return;
    }
    const run = { kind: "text", text: child.text };
    for (const mark of child.marks) {
      if (mark.type === schema.marks.strong) {
        run.strong = true;
      } else if (mark.type === schema.marks.em) {
        run.emphasis = true;
      } else if (mark.type === schema.marks.link) {
        run.link = mark.attrs.href;
      }
    }
    runs.push(run);
  });

  return runs;
}

/// The comment written, as blocks: each paragraph, and each list with one line of runs for each item.
export function blocksOf(doc) {
  const blocks = [];
  doc.forEach((block) => {
    if (block.type === schema.nodes.paragraph) {
      blocks.push({ kind: "paragraph", runs: runsOf(block) });
    } else {
      const items = [];
      block.forEach((item) => items.push(runsOf(item)));
      blocks.push({
        kind: "list",
        ordered: block.type === schema.nodes.ordered_list,
        items,
      });
    }
  });

  return blocks;
}

function lineOf(runs) {
  return (runs ?? []).flatMap((run) => {
    if (run.kind === "break") {
      return [schema.nodes.hard_break.create()];
    }
    if (!run.text) {
      return [];
    }
    const marks = [];
    if (run.strong) {
      marks.push(schema.marks.strong.create());
    }
    if (run.emphasis) {
      marks.push(schema.marks.em.create());
    }
    const href = safeLink(run.link);
    if (href) {
      marks.push(schema.marks.link.create({ href }));
    }

    return [schema.text(run.text, marks)];
  });
}

/// The document a comment is written into again, from its blocks.
export function docOf(blocks) {
  const made = (blocks ?? []).map((block) =>
    block.kind === "list"
      ? (block.ordered ? schema.nodes.ordered_list : schema.nodes.bullet_list).create(
          null,
          (block.items.length ? block.items : [[]]).map((item) =>
            schema.nodes.list_item.create(null, lineOf(item)),
          ),
        )
      : schema.nodes.paragraph.create(null, lineOf(block.runs)),
  );

  return schema.nodes.doc.create(
    null,
    made.length ? made : [schema.nodes.paragraph.create()],
  );
}

function saidNothing(doc) {
  return doc.textContent.trim() === "";
}

/// A line typed as "- " or "1. " at the start of a paragraph becomes a list.
function listing(pattern, list) {
  return new InputRule(pattern, (state, match, start, end) => {
    const $start = state.doc.resolve(start);
    if ($start.parent.type !== schema.nodes.paragraph || $start.depth !== 1) {
      return null;
    }
    const paragraph = $start.parent;
    const rest = paragraph.content.cut(end - $start.start());
    const before = $start.before();

    return state.tr.replaceWith(
      before,
      before + paragraph.nodeSize,
      list.create(null, schema.nodes.list_item.create(null, rest)),
    ).scrollIntoView();
  });
}

/// Enter on an empty last item leaves the list for a new paragraph after it.
function leavingTheList(state, dispatch) {
  const { $from, empty } = state.selection;
  const item = $from.parent;
  if (!empty || item.type !== schema.nodes.list_item || item.content.size > 0) {
    return false;
  }
  const list = $from.node(-1);
  if ($from.index(-1) !== list.childCount - 1) {
    return false;
  }
  if (dispatch) {
    const listStart = $from.before(-1);
    const listEnd = $from.after(-1);
    const tr = state.tr;
    if (list.childCount === 1) {
      tr.replaceWith(listStart, listEnd, schema.nodes.paragraph.create());
      tr.setSelection(TextSelection.create(tr.doc, listStart + 1));
    } else {
      tr.delete($from.before(), $from.after());
      const at = listEnd - item.nodeSize;
      tr.insert(at, schema.nodes.paragraph.create());
      tr.setSelection(TextSelection.create(tr.doc, at + 1));
    }
    dispatch(tr.scrollIntoView());
  }

  return true;
}

const hardBreak = chainCommands(exitCode, (state, dispatch) => {
  if (dispatch) {
    dispatch(
      state.tr.replaceSelectionWith(schema.nodes.hard_break.create()).scrollIntoView(),
    );
  }

  return true;
});

/// The field a comment is written in, with bold, italics, links and lists, and undo.
/// It hands back what is written as the comment's blocks, as JSON.
export class CommentEditor {
  constructor(host, held, changed, save) {
    this.changed = changed;
    this.view = new EditorView(host, {
      state: EditorState.create({
        doc: docOf(JSON.parse(held || "[]")),
        plugins: [
          history(),
          inputRules({
            rules: [
              listing(/^\s*([-*])\s$/, schema.nodes.bullet_list),
              listing(/^\s*(\d+)\.\s$/, schema.nodes.ordered_list),
            ],
          }),
          keymap({
            "Mod-z": undo,
            "Mod-y": redo,
            "Shift-Mod-z": redo,
            "Mod-b": toggleMark(schema.marks.strong),
            "Mod-i": toggleMark(schema.marks.em),
            "Mod-Enter": () => {
              save();
              return true;
            },
            "Shift-Enter": hardBreak,
            Enter: chainCommands(leavingTheList, splitBlock),
          }),
          keymap(baseKeymap),
        ],
      }),
      attributes: {
        class: "comment-field",
        role: "textbox",
        "aria-multiline": "true",
        "aria-label": host.dataset.label ?? "",
      },
      dispatchTransaction: (tr) => {
        const before = saidNothing(this.view.state.doc);
        this.view.updateState(this.view.state.apply(tr));
        const now = saidNothing(this.view.state.doc);
        if (tr.docChanged && before !== now) {
          this.changed(now);
        }
      },
    });
  }

  /// What is written, as the comment's blocks in JSON.
  written() {
    return JSON.stringify(blocksOf(this.view.state.doc));
  }

  saysNothing() {
    return saidNothing(this.view.state.doc);
  }

  toggleBold() {
    toggleMark(schema.marks.strong)(this.view.state, this.view.dispatch);
  }

  toggleItalic() {
    toggleMark(schema.marks.em)(this.view.state, this.view.dispatch);
  }

  hasSelection() {
    return !this.view.state.selection.empty;
  }

  /// Links what is selected to the address typed; whether it was an address that may be linked.
  setLink(typed) {
    const href = linkTyped(typed);
    if (!href) {
      return false;
    }
    const { from, to } = this.view.state.selection;
    this.view.dispatch(
      this.view.state.tr
        .removeMark(from, to, schema.marks.link)
        .addMark(from, to, schema.marks.link.create({ href })),
    );

    return true;
  }

  focus() {
    this.view.focus();
  }

  destroy() {
    this.view.destroy();
  }
}
