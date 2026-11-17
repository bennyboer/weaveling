# readings

This feature lets the book be read: the live book for members, and later reading copies that are frozen and shared by link. See M18 in the [roadmap](../../ROADMAP.md).

- `GET /readings/live?project=…` returns the live book as it is written now: its sections and scenes in reading order, each scene with its text as blocks and runs.

**For now the live book is a fixed sample.** `ReadingsCore::live_book` answers the same small book for every project and asks nobody whether they may read it. The client is built against it first. Putting the book together from the outline and the scenes, and allowing members only, come in the next steps.

**`Contents` is the core's, `ContentsDTO` the contract's.** The REST adapter turns one into the other. A section carries its depth, so the parts are one flat list in reading order. A scene keeps the id of the scene it came from. Runs carry only the marks a reader sees: emphasis, strong, code and links.

**Crates:** `core` (`ReadingsCore`, `Contents`, links to projects, sections, scenes and assets) · `contract` · `adapters/rest` · `wiring` · `tests`.
