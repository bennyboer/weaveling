# assets

The files Weaveling keeps that are not text, images for now: an account's picture first, later images in ideas.

**An `Asset` is whose it is, and the picture.** It belongs to a project, read by those who may read it, or to an account, seen by those who share a project with it, and is uploaded once and deleted by an event. Its `Picture` tells the original's `Format`, known by its bytes (JPEG, PNG, GIF or WebP), how wide and high it is, how large, and the widths it was made into; `kept` is every byte held for it, the original and its variants, which is what counts against what the installation may hold. A project's asset tells its project with its messages; an account's tells none.

**The original is never handed out,** only kept to make variants from, as a phone's photo tells where it was taken; the variants, WebP made at upload, are what is shown.

Events are kept by the [`eventsourcing`](../../libraries/eventsourcing) store through the codec in `adapters/store`; `contract` holds what other features may hear: `asset.uploaded` and `asset.deleted`.
