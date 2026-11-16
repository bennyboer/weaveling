import { Plugin } from "prosemirror-state";
import { Decoration, DecorationSet } from "prosemirror-view";

import { figureSpot } from "./figures.js";

function keyOf(ticket, nth) {
  return `${ticket}:${nth}`;
}

function placeOf(doc, at) {
  return figureSpot(doc, at).from;
}

export class Uploads {
  constructor({ labels, redraw, refocus }) {
    this.labels = labels;
    this.redraw = redraw;
    this.refocus = refocus;
    this.imagesGiven = () => {};
    this.waiting = new Map();
    this.uploads = new Map();
    this.ended = new Set();
    this.waited = 0;
  }

  plugin() {
    return new Plugin({
      props: {
        decorations: (state) => this.shownIn(state.doc),
      },
    });
  }

  waitFor(at) {
    this.waited += 1;
    this.waiting.set(this.waited, at);

    return this.waited;
  }

  placeOf(ticket) {
    return this.waiting.get(ticket);
  }

  movedTo(ticket, at) {
    if (this.waiting.has(ticket)) {
      this.waiting.set(ticket, at);
    }
  }

  mapped(mapping) {
    for (const [ticket, at] of this.waiting) {
      this.waiting.set(ticket, mapping.map(at));
    }
  }

  given(files, at) {
    const images = Array.from(files ?? []).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (images.length === 0) {
      return false;
    }
    this.imagesGiven(images, this.waitFor(at));

    return true;
  }

  forget(ticket) {
    this.ended.add(ticket);
    this.letGoOf(ticket);
  }

  letGoOf(ticket) {
    const held = [...this.uploads.values()].some(
      (upload) => upload.ticket === ticket,
    );
    if (!held && this.ended.has(ticket)) {
      this.ended.delete(ticket);
      this.waiting.delete(ticket);
    }
  }

  started(ticket, nth, name, label) {
    this.uploads.set(keyOf(ticket, nth), {
      ticket,
      nth,
      name,
      label,
      fraction: 0,
      refused: null,
    });
    this.redraw();
  }

  progressed(ticket, nth, fraction) {
    const upload = this.uploads.get(keyOf(ticket, nth));
    if (!upload) {
      return;
    }
    const shown = Math.round(Math.min(Math.max(fraction, 0), 1) * 100);
    if (shown !== Math.round(upload.fraction * 100)) {
      upload.fraction = shown / 100;
      this.redraw();
    }
  }

  refused(ticket, nth, told) {
    const upload = this.uploads.get(keyOf(ticket, nth));
    if (upload) {
      upload.refused = told;
      this.redraw();
    }
  }

  done(ticket, nth) {
    this.uploads.delete(keyOf(ticket, nth));
    this.letGoOf(ticket);
    this.redraw();
  }

  shownIn(doc) {
    if (this.uploads.size === 0) {
      return DecorationSet.empty;
    }
    const shown = [];
    for (const [key, upload] of this.uploads) {
      const at = this.waiting.get(upload.ticket);
      if (at === undefined) {
        continue;
      }
      const state =
        upload.refused === null ? Math.round(upload.fraction * 100) : "refused";
      shown.push(
        Decoration.widget(placeOf(doc, at), () => this.drawn(upload), {
          key: `upload-${key}-${state}`,
          side: 1,
          ignoreSelection: true,
          stopEvent: () => true,
        }),
      );
    }

    return DecorationSet.create(doc, shown);
  }

  drawn(upload) {
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
      dismiss.textContent = "×";
      dismiss.setAttribute("aria-label", this.labels().dismiss ?? "");
      dismiss.addEventListener("mousedown", (event) => event.preventDefault());
      dismiss.addEventListener("click", () => {
        this.done(upload.ticket, upload.nth);
        this.refocus();
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
    bar.setAttribute(
      "aria-valuenow",
      String(Math.round(upload.fraction * 100)),
    );
    bar.style.setProperty("--done", `${upload.fraction * 100}%`);
    shown.append(name, bar);

    return shown;
  }
}
