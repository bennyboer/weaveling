let comment = null;

export async function commentEditorLoaded() {
  comment ??= await import("./comment/comment.js");
}

export class CommentEditor {
  constructor(...given) {
    return new comment.CommentEditor(...given);
  }
}
