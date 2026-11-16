import { Schema } from "prosemirror-model";
import { schema as basic } from "prosemirror-schema-basic";
import { addListNodes } from "prosemirror-schema-list";

const LINKABLE = /^(https?:\/\/[^\s]+|mailto:[^\s]+)$/i;
const HAS_SCHEME = /^[a-z][a-z0-9+.-]*:/i;
const AN_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

export function safeLink(href) {
  const told = String(href ?? "").trim();

  return LINKABLE.test(told) ? told : null;
}

export function linkTyped(typed) {
  const told = String(typed ?? "").trim();
  if (told === "" || HAS_SCHEME.test(told)) {
    return safeLink(told);
  }

  return safeLink(AN_EMAIL.test(told) ? `mailto:${told}` : `https://${told}`);
}

const AN_ASSET = /^[A-Za-z0-9_]{1,64}$/;
export const SIZES = ["small", "medium", "full"];
export const SHOWN_WIDE = { small: 768, medium: 1536, full: 1536 };

export function sizeOf(told) {
  return SIZES.includes(told) ? told : "full";
}

export function assetOf(told) {
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

export const schema = new Schema({
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
