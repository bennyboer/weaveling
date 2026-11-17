# readings

This feature lets the book be read: the live book for members, and later reading copies that are frozen and shared by link. See M18 in the [roadmap](../../ROADMAP.md).

- `GET /readings/live?project=…` returns the live book as it is written now: its sections and scenes in reading order, each scene with its text as blocks and runs.

**The book is put together from two catalogs.** `OutlineCatalog` holds each project's title and its sections in reading order, with how deep each is nested and the scenes it holds. `SceneTextCatalog` holds each scene's text as blocks. `ReadingsCore::live_book` lays out each section, then its scenes. A scene whose text is not kept yet is left out rather than shown empty, and a project without an outline is not there. The feature will keep both catalogs itself, from the outline's events and from the text the scenes feature publishes once it has settled, as statistics keeps its word counts.

**For now both catalogs are a fixed sample.** `SampleBook` in `adapters/catalog` answers the same small book for every project, and nobody is asked whether they may read it. Keeping the catalogs from messages, and allowing members only, come in the next steps.

**`Contents` is the core's, `ContentsDTO` the contract's.** The REST adapter turns one into the other. A section carries its depth, so the parts are one flat list in reading order. A scene keeps the id of the scene it came from. Runs carry only the marks a reader sees: emphasis, strong, code and links.

**Crates:** `core` (`ReadingsCore`, `Contents`, the catalog ports, links to projects, sections, scenes and assets) · `contract` · `adapters/catalog` · `adapters/rest` · `wiring` · `tests`.
