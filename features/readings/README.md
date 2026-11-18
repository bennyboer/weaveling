# readings

This feature lets the book be read: the live book for members, and later reading copies that are frozen and shared by link. See M18 in the [roadmap](../../ROADMAP.md).

- `GET /readings/live?project=…` returns the live book as it is written now: its sections and scenes in reading order, each scene with its text as blocks and runs.

**The book is put together from two catalogs.** `OutlineCatalog` holds each project's title and its sections in reading order, with how deep each is nested and the scenes it holds. `SceneTextCatalog` holds each scene's text as blocks. `ReadingsCore::live_book` lays out each section, then its scenes. A scene whose text is not kept yet is left out rather than shown empty, and a project without an outline is not there. The feature will keep both catalogs itself, from the outline's events and from the text the scenes feature publishes once it has settled, as statistics keeps its word counts.

**A project's book is there only for those in it.** `live_book` asks the [`Authorizer`](../../libraries/authorizing) for `Read`. Someone who may not read the project, or finds it in the trash, gets a 404, the same as for a project that is not there. If who may read it cannot be known, that is a failure, not a no. Who may read a project is kept by the feature itself, through [`access`](../../libraries/access/README.md), in its own database. Two listeners receive their messages through the feature's own inbox:

- `readings-remember-roles` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `readings-remember-trash` receives when projects are put in the trash and restored.

The chore `forget-old-readings-tombstones` deletes the records of accounts that left long ago, and `forget-old-readings-hearings` the old records of messages received. Without accounts, everyone is allowed.

**A scene's text is kept as the scenes feature tells it.** The listener `readings-keep-scene-texts` receives `state.scene-text.changed` and `scene.deleted` through the feature's own inbox. `ReadingsCore::text_changed` keeps a text only if it was read at a newer prose version than the one kept, so a late message does not undo a newer one. `scene_deleted` forgets the text and lays a tombstone, so a late text does not bring the scene back. The chore `forget-old-readings-tombstones` forgets these tombstones too. `InMemorySceneTextCatalog` keeps them in memory, and `PostgresSceneTextCatalog` and `SqliteSceneTextCatalog` in the feature's own database, in `scene_texts` and `scene_text_tombstones`. A scene's blocks are kept as JSON in the catalog's own form (`KeptBlock` in `kept.rs`), apart from the API's, so the reading view's needs and what the databases hold change each on their own, the latter only with a migration. Keeping is one statement, which writes only a newer version and nothing for a scene with a tombstone. The suite in `adapters/catalog` holds all three to the same cases.

**For now both catalogs are a fixed sample.** `SampleBook` in `adapters/catalog` answers the same small book for every project, and the wiring uses it for both catalogs until the outline is kept from messages too; then both are switched together.

**`Contents` is the core's, `ContentsDTO` the contract's.** The REST adapter turns one into the other. A section carries its depth, so the parts are one flat list in reading order. A scene keeps the id of the scene it came from. Runs carry only the marks a reader sees: emphasis, strong, code and links.

**Crates:** `core` (`ReadingsCore`, `Contents`, the catalog ports, links to projects, sections, scenes and assets) · `contract` · `adapters/catalog` · `adapters/messaging` (the listeners) · `adapters/rest` · `wiring` · `tests`.
