import { safeLink, schema } from "./schema.js";

export function linkAt(state) {
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

export function linking(href) {
  return (state, dispatch) => {
    const { from, to } = state.selection;
    dispatch(
      state.tr
        .removeMark(from, to, schema.marks.link)
        .addMark(from, to, schema.marks.link.create({ href })),
    );

    return true;
  };
}

export function unlinking(state, dispatch) {
  const { from, to } = state.selection;
  dispatch(state.tr.removeMark(from, to, schema.marks.link));

  return true;
}

export function openedLink(view, pos, event) {
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
