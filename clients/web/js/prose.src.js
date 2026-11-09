let editor = null;

export async function editorLoaded() {
  editor ??= await import("./editor.src.js");
}

export class SyncConnection {
  constructor() {
    return new editor.SyncConnection();
  }
}

export class ProseEditor {
  constructor(...given) {
    return new editor.ProseEditor(...given);
  }
}
