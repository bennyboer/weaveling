import { chosenFigure } from "./figures.js";
import { SHOWN_WIDE, assetOf, sizeOf } from "./schema.js";

const FIGURE_HTML = /<figure[^>]*\bdata-asset=/i;

export function copiedPicture(view) {
  const chosen = chosenFigure(view.state);
  const asset = chosen && assetOf(chosen.attrs.asset);
  if (!asset || !navigator.clipboard?.write || typeof ClipboardItem === "undefined") {
    return false;
  }

  const size = sizeOf(chosen.attrs.size);
  const caption = chosen.textContent;
  const picture = pictureOf(asset, size);
  const told = {
    "image/png": picture,
    "text/html": picture.then(async (png) =>
      new Blob([figureHtml(asset, size, caption, await dataUrlOf(png))], {
        type: "text/html",
      }),
    ),
  };
  if (caption.trim() !== "") {
    told["text/plain"] = new Blob([caption], { type: "text/plain" });
  }
  navigator.clipboard.write([new ClipboardItem(told)]).catch(() => {});

  return false;
}

export function holdsAFigure(data) {
  return FIGURE_HTML.test(data?.getData("text/html") ?? "");
}

async function pictureOf(asset, size) {
  const fetched = await fetch(
    `/api/assets/${asset}/content?width=${SHOWN_WIDE[size]}`,
    { credentials: "same-origin" },
  );
  if (!fetched.ok) {
    throw new Error(`the picture could not be fetched: ${fetched.status}`);
  }
  const bitmap = await createImageBitmap(await fetched.blob());
  const canvas = document.createElement("canvas");
  canvas.width = bitmap.width;
  canvas.height = bitmap.height;
  canvas.getContext("2d").drawImage(bitmap, 0, 0);
  bitmap.close();

  return new Promise((resolve, reject) =>
    canvas.toBlob(
      (png) => (png ? resolve(png) : reject(new Error("no picture was drawn"))),
      "image/png",
    ),
  );
}

function dataUrlOf(blob) {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function figureHtml(asset, size, caption, picture) {
  return (
    `<figure data-asset="${asset}" data-size="${size}">` +
    `<img src="${picture}" alt="${escaped(caption)}">` +
    `<figcaption>${escaped(caption)}</figcaption>` +
    `</figure>`
  );
}

function escaped(text) {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}
