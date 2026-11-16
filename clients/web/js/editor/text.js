function blocksOf(doc, told) {
  const blocks = [];

  doc.descendants((node) => {
    if (node.type.name === "figure") {
      return false;
    }
    if (!node.isTextblock) {
      return true;
    }
    blocks.push(told(node));

    return false;
  });

  return blocks;
}

export function writtenText(doc) {
  return blocksOf(doc, (node) => node.textContent).join("\n");
}

export function textOf(doc) {
  return blocksOf(doc, (node) =>
    node.textBetween(0, node.content.size, " ", " "),
  ).join(" ");
}
