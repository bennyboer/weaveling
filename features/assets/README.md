# assets

This feature keeps the files in Weaveling that are not text. For now, these are only images. The first use is an account's picture. Later, ideas will hold images too.

**An `Asset` is whose it is, and the picture.** An asset belongs either to a project or to an account. A project's asset can be read by everyone who may read the project. An account's asset can be seen by everyone who shares a project with that account. An asset is uploaded once, and it is deleted by an event.

Its `Picture` stores:

- the original's `Format`, which is recognised by its bytes (JPEG, PNG, GIF or WebP),
- how wide and how high the original is,
- its `original_size`,
- the widths of the variants that were made from it.

`total_size` is every byte stored for the asset: the original plus all its variants. This is the number that counts against how much the installation may store. The messages about a project's asset name its project. The messages about an account's asset name no project.

**The original is never handed out.** It is only kept so that variants can be made from it. The original can hold private data: a photo from a phone, for example, records where it was taken. The variants are WebP images made at upload, and only they are shown.

**An upload is read as an image behind the `Images` port.** `WebpImages` in `adapters/images` implements this port. It is the only place where the image encoder is chosen. It works like this:

- The kind of image is recognised by its bytes: JPEG, PNG, GIF or WebP. For a GIF, only its first frame is used. Anything else, an SVG for example, is refused as not an image.
- An image is refused as too large if one side is longer than 16,384 pixels, or if it has more than fifty million pixels. This check happens after reading only the header. So a small file that claims to be a huge picture uses no memory.
- The image is turned upright, as its camera recorded.
- Variants are made in every width of 384, 768, 1,536 and 3,072 that is narrower than the image. If the image is narrower than the widest of these, a variant in its own width is made too. A variant is never wider than the image. Each variant is made from the next wider one.
- The variants are encoded as WebP by `gamut-webp` at quality 75. They keep transparency only if the image has any.

The widths are stored in the `Picture`. `Prepared::total_size` counts the original and every variant. This work is slow. So the upload runs it on blocking threads beside the async runtime, and only for a few uploads at a time.

**The bytes are kept apart from the events.** They are stored behind the `Contents` port. Each stored piece is identified by its asset and by either `Original` or a variant's `Width`. There are three places to store them:

- in memory,
- in a folder: each asset gets a folder of its own. Each piece is first written next to its final name and then moved into place, so it is never read half-written.
- in the database: `asset_contents` on PostgreSQL.

In local mode, the pieces go into the data's own folder. On a server, they go into the database, unless the operator names a folder. A folder cannot take part in a transaction, so no write of pieces is part of a transaction. A piece is stored before the asset is recorded as uploaded. A piece is removed after the asset is recorded as deleted.

**One way in: `AssetsCore`.**

- `upload_into` stores an image in a project for anyone who may write in that project. It refuses someone who may only read the project (`NotAnAuthor`), and someone who may not even read it (`ProjectNotThere`).
- `content` returns an image to anyone who may read it. It returns the narrowest variant that is at least as wide as asked. If there is no such variant, it returns the widest one. If no width is asked for, it returns the widest one. It never returns the original. A deleted asset, or one the caller may not see, is not found.
- `delete` records the asset as deleted, for anyone who may write where the asset is. Then it removes the asset's stored pieces. Deleting it again changes nothing.

Only the account itself may change or delete its own asset. Who may see it is described below.

**Limits are one notion.** A `Limit` is the most a `Bound` may hold, together with how much that bound holds now. A bound is an upload, the installation, an account or a project. `Limits` names the most for each bound. A bound is missing from it if it has no limit. An upload is refused by the first limit it would pass. The error is `LimitReached`, and it says which limit was passed, its maximum and how much that bound holds.

The upload's own limit is checked before anything is made from the upload. So an upload that is too large is not even read. The other limits are checked in the transaction that records the asset as uploaded. They use what the catalog holds, and the installation's limit is checked first. What an account uploads counts against that account, wherever the upload goes. Without accounts, nobody is held to an account's limit.

**The catalog is written in the transaction that tells an asset uploaded or deleted,** and not by a listener afterwards, as in other features. So what each bound holds is never out of date when an upload is checked against it. On PostgreSQL, two uploads may be checked against the same total at the same time. The serializable transactions keep them apart, and one of them is tried again.

`asset_summaries` stores each asset's owner, uploader, format, size and how much it takes up. It exists in memory, on PostgreSQL and on SQLite. **`asset_usage` keeps a running total** for the installation, for each account and for each project. A total goes up when an asset is stored and goes down when an asset is removed. So the check of an upload reads three rows by their keys, instead of adding up every asset. Two uploads by different accounts into different projects only touch the same row for the installation's total.

**The limit an operator gives a single account is kept by the feature too,** in `AccountLimits`. It is stored in `asset_account_limits` on PostgreSQL and SQLite, and in memory. All three pass one shared conformance suite. An account without a limit of its own is held to the limit for every account. When its own limit is taken back, it is held to that general limit again. An upload by an account is checked against the account's own limit, instead of the limit for every account. This limit is read in the transaction that records the upload. Without accounts, there is no account limit to replace, so a given limit does not count.

**Who may do what is kept by the feature itself,** through [`access`](../../libraries/access/README.md). This data is stored in the assets' own database. Three listeners receive their messages through the inbox:

- `RememberRolesListener` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `RememberTrashListener` receives when projects are put in the trash and restored.
- `RememberOperatorsListener` receives when accounts become operators, stop being operators, or are deleted.

A chore deletes old tombstones. If there are accounts, the core asks `KeptAccess` who may do what. Without accounts, everyone is allowed. **An account's own asset is seen by the account and by whoever shares a project with it.** If there are no accounts, nobody sees it.

**The names of accounts and projects are kept by the feature too,** so that an operator can find what each account and project holds by its name. Two listeners receive their messages through the inbox:

- `RememberPeopleListener` receives sign-ups (both kinds) and account deletions.
- `RememberProjectNamesListener` receives when projects are started, renamed and deleted.

They store the names in the catalog's `asset_people` and `asset_project_names`. Each name has a version, so a message that arrives late changes nothing. A deletion puts a tombstone in place of the name. The same chore that deletes the access tombstones also deletes these.

**The assets of a deleted project or account are deleted with it.** When `RememberProjectNamesListener` hears a project deleted, or `RememberPeopleListener` an account deleted, it queues a follow-up, `follow-up.asset.sweep-owner`, in the same transaction. `SweepAssetsOfDeletedOwnerListener` then runs `AssetsCore::sweep` for that owner. The sweep deletes the owner's assets as the system, 50 per transaction, and removes their stored bytes after each commit. An account's sweep only deletes its own images. What it uploaded into projects stays with the projects. Once assets can be referenced, the sweep will have to skip images that are still used somewhere (see the ROADMAP).

**Stored pieces that belong to no asset are removed once a day.** They are left behind if the process stops between deleting an asset and removing its bytes, or in the middle of a failed upload. The chore `forget-stray-asset-contents` lists the stored assets through `Contents::stored`, asks the catalog which of them it still holds (`held_of`), and removes the pieces of the others. It only touches assets whose id was made over an hour ago. An asset's id records when it was made, and an upload stores its pieces right after making the id, so an upload in progress is never touched.

**Pieces first, the event after.** An upload stores its original and its variants before the asset is recorded as uploaded. If the upload is refused or fails, it removes them again. Uploads are prepared on blocking threads beside the async runtime, at most `PREPARED_AT_ONCE` at a time.

Events are stored by the [`eventsourcing`](../../libraries/eventsourcing) store, through the codec in `adapters/store`. `contract` holds the events other features may receive: `asset.uploaded` and `asset.deleted`. `adapters/messaging` publishes them through `message_for`. An upload message says:

- whose the asset is (a project's or an account's),
- its format and its size in pixels,
- the original's size in bytes, and the size of everything stored for it together.

It does not say which widths were made. Only the feature itself hands out the variants. A snapshot is never published.

`wiring` assembles the feature with `assembled_assets(storage, context, Keeping { limits, folder })`. If the operator names a folder, the pieces are stored there. Otherwise they are stored where the data is: in memory, in PostgreSQL's `asset_contents`, or, with SQLite, in an `assets` folder next to its `assets.sqlite`, because SQLite does not store the pieces itself. The server assembles the feature next to the other features. `Adapters::keeping_assets` takes the `[assets]` section of `weaveling.toml` through `AssetSettings::keeping`. This holds an account to its limit only if there are accounts. `Adapters::assembled` keeps the defaults.

`adapters/rest` serves three routes under `/api`:

- `POST /assets?project=` stores the image, sent as the raw body, in a project. `POST /assets` without a project stores it as the signed-in account's own image (`AssetsCore::upload_as_own`). Such an image counts only against the installation and the account. Without accounts, this answers `403 asset-no-account`. The body is read up to the upload limit. If no upload limit is set, the body may even be larger than axum's default of two megabytes. The route answers `201` with an `AssetDTO` and its `ETag`.
- `GET /assets` lists what the signed-in account uploaded, both into projects and as its own images.
  - It lists fifty at a time.
  - `?sort=newest` is the default. `?sort=largest` is the other choice. Entries that are equal are ordered by their ids.
  - The next page is asked with `?after=` set to the page's `next`. This is a cursor made of the last listed asset's size, moment and id (`UploadsDTO`).
  - Each entry says whose the asset is, and the project's name as the assets feature received it. It also gives the picture, the size and the moment of upload, and whether the account may delete the asset. The account may always delete its own images. It may delete an image in a project only while it may write in that project, just as `DELETE` checks.
  - It is read by `uploaded_by` on the catalog. The indexes are `(uploader, total_size DESC, id)` and `(uploader, uploaded_at DESC, id)`.
  - Without accounts, this answers `403 asset-no-account`. A wrong sort or cursor is refused as described below.
- `GET /assets/{id}/content?width=` returns the narrowest variant that is at least as wide as asked, or else the widest. It is sent as `image/webp` with `Cache-Control: private, max-age=31536000, immutable`, because an asset's pieces never change.
- `DELETE /assets/{id}` answers `204`.
- `PUT /assets/accounts/{account}/limit` with `{ "most": bytes }` gives an account a limit of its own. `DELETE` on the same route takes it back. Both answer `204`. Only an operator may use them, everyone else gets `403 not-an-operator`. The assets feature knows who is an operator from the accounts' messages, through `RememberOperatorsListener`. A limit larger than every backend can count, `MOST_COUNTED`, is refused with `422 asset-limit-uncountable`.
- `GET /assets/storage` tells an operator how much the instance holds and the most it may hold (`InstanceStorageDTO`).
- `GET /assets/storage/accounts` lists each account that uploaded something. For each account, it says how much it holds, the most it may hold, and whether that is its own limit.
- `GET /assets/storage/projects` lists each project. For each project, it says how much it holds, the most it may hold, and who its owners are.
- About these two lists:
  - Each entry has its name, and an account also has its address, as the assets feature received them. If the feature received none, they are missing.
  - Both list fifty at a time.
  - `?search=` lists only those whose name or address contains the term, ignoring case. A term of only spaces counts as no search. With a term, the names are searched first. A `CROSS JOIN` makes SQLite keep that order.
  - `?sort=most-used` is the default: the ones holding the most come first, and equal ones are ordered by their ids. `?sort=id` is the other choice.
  - The next page is asked with `?after=` set to the page's `next`. This is a cursor made of the last listed entry's total and id.
  - An unknown sort is `400 asset-sort-unknown`. A cursor that is not valid is `400 asset-cursor-invalid`. Someone who is not an operator gets `403 not-an-operator`.
  - They are read from the running totals, by `held_by` on the catalog. An index on `asset_usage (bound, total DESC, id)` keeps the ones holding the most quick to find. Someone who no longer holds anything is not listed. The owners are read from the assets' own `access`. Each owner comes with their name and email from `asset_people` (`people` on the catalog), or without them if the assets feature never heard them.
- `GET /assets/usage` tells the signed-in account how much its uploads take up, wherever they went, and the most they may take up. That maximum is either its own limit or the limit for every account. The answer is `UsageDTO { used, most }`. Without accounts, this answers `403 asset-no-account`.

A reached limit is `413 asset-limit-reached`. It names its `bound` (`upload`, `instance`, `account` or `project`), its `most`, and how much the bound holds, `used`. A refused image is `422`, with `asset-not-an-image`, `asset-image-unreadable` or `asset-image-too-large`. The client has no texts for these errors yet. They will be added to its catalogue together with the assets manager. The client serves no routes yet.
