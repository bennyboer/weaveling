# assets

The files Weaveling keeps that are not text, images for now: an account's picture first, later images in ideas.

**An `Asset` is whose it is, and the picture.** It belongs to a project, read by those who may read it, or to an account, seen by those who share a project with it, and is uploaded once and deleted by an event. Its `Picture` tells the original's `Format`, known by its bytes (JPEG, PNG, GIF or WebP), how wide and high it is, how large, and the widths it was made into; `kept` is every byte held for it, the original and its variants, which is what counts against what the installation may hold. A project's asset tells its project with its messages; an account's tells none.

**The original is never handed out,** only kept to make variants from, as a phone's photo tells where it was taken; the variants, WebP made at upload, are what is shown.

**The bytes are kept apart from the events,** behind the `Contents` port, each piece by the asset and either `Original` or a variant's `Width`: in memory, in a folder, each asset a folder of its own and each piece written beside its name and moved in place so it is never read half written, or in the database, `asset_contents` on PostgreSQL. In local mode they go to the data's own folder, on a server into the database unless the operator names a folder. A folder takes no transaction, so nothing here is part of one: a piece is kept before the asset is told uploaded, and forgotten after it is told deleted.

Events are kept by the [`eventsourcing`](../../libraries/eventsourcing) store through the codec in `adapters/store`; `contract` holds what other features may hear: `asset.uploaded` and `asset.deleted`.
