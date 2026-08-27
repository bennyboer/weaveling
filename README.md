# Weaveling

> Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic.

## What is Weaveling?

Weaveling is a tool for writing books — historical fiction, science-fiction, and pure fantasy alike or whatever you want!

Writers often look at their messy first drafts and think, *"This isn't a masterpiece yet, it's just a little mess."* The name **Weaveling** answers that thought directly: bring your smallest, most fragile threads of an idea, and Weaveling helps you structure them into a full, woven epic.

## Why "Weaveling"?

Stories are woven. They start as loose, tangled threads — a character, a scene, a "what if" — and become a tapestry only once those threads are structured, connected, and given shape.

The `-ling` says something too: a weaveling is small, young, and alive. It's the story in its infancy. Weaveling meets the idea where it is — tiny and unfinished — and grows with it.

## The Vision

Weaveling is built by a software developer who loves typography and art, for the author they want to become. It is opinionated about craft: the writing experience, the way text looks on the page, and the structure beneath a story all matter.

The promise to the writer:

- **No blank-page paralysis.** A first draft is *supposed* to be a little mess. Weaveling embraces that.
- **Structure without straitjackets.** Help turning tangled threads into a coherent narrative, without forcing a rigid template.
- **Typography and craft as first-class citizens.** How the words look and feel is part of the writing, not an afterthought.
- **Just-in-time detail.** Like iterative software development: you never have to specify the whole book up front. Drill deeper only where and when the idea is ready for it.

## The Workspace

Weaveling holds **multiple projects** in one place. A project has a name (a working title or the final book name) and is the container for everything below.

A project is deliberately flexible about scope: it can be a single book, or a whole multi-book series. The top of a project's outline might be one book, or a series whose children are the individual books — that choice is left to the author.

## The Core Idea: Warp & Weft

Underneath every view is a single shared model — one fabric, seen from different angles.

- A **piece** is the raw material: a fragment of the book, carrying a title and its prose. It knows nothing about where it belongs.
- The **outline** is the *warp*: the vertical threads, the spine of the book.
- **Time**, **codex entities**, and **threads** are the *weft*: they run crossways through the pieces.
- A piece is where warp meets weft — where a character appears, in a scene, at a moment in time.

The views are not separate tools stapled together; they are lenses onto the same weave. This is why they stay linked: navigating from one to another is just turning the fabric to catch the light differently.

**No view owns a piece.** The same piece sits somewhere in the outline, at a moment on the timeline, inside two subplots, and on the board where it was born — and none of those is *the* answer to where it is. Which is also why a piece can exist having never been placed at all: unwoven thread is still thread.

## The Views

### Board view

Where ideas land before they are anything. An infinite surface — closer to a wall of index cards than to a document — on which the author shoots in a fragment, gives it a title or doesn't, and can start writing inside it straight away.

Nothing on the board has a place in the book yet. Pieces sit wherever they were dropped, in whatever clusters make sense that day, and spatial memory is allowed to do real work — *the abandoned ideas live bottom-left*. Several authors can brainstorm on one board at once and watch each other move things around.

This is the answer to blank-page paralysis. Capturing a piece demands no decision about where it belongs, so there is no structure to fight before writing a single sentence.

### Outline view

The book as a hierarchy — the shape a reader would recognise as a table of contents. The author drags pieces in from the board and keeps splitting them as far as the budding idea allows. Detail is added *just in time*, not up front: beginning, main part, ending is a perfectly good outline for a long while.

The finest useful grain is roughly **one piece per paragraph** — the maximum depth of structural detail.

**This is the privileged view.** A book is ultimately linear, so reading — and export — is an in-order walk of the outline, stitching every piece's text together. A piece that is *not* in the outline is simply not in the book yet, which is a feature rather than a gap: the board can hold twenty ideas that may never make it, and none of them has to be deleted to stay out of the way.

**Prose lives on every piece.** A piece can hold text at any level. When a piece that already has text is split, the author decides what happens to that text: move parts of it down into the new children, or leave it on the parent.

### Text view

Where the actual prose is written. Deliberately **distraction-free**: just the text, in a large, comfortable font, so the author can concentrate on the words.

Linked to every other view: double-clicking a piece — on the board, in the outline, on the timeline — dives into its text. Codex entity names (see below) are **highlighted** inline, so their sheet is one glance away while writing dialogue.

### Codex

Every book has a cast — and a world. The **Codex** is the compendium of everything in the story: **characters** first and foremost, but also **locations, factions, magic/tech systems, artifacts**, and anything else worth tracking. They share one shape: fields, images, milestones, relations, free text, and back-links to the pieces they appear in.

The Codex is linked to the other views:

- Each entity keeps an **alias list** (names, titles, nicknames), used to detect mentions in the prose.
- In the **text view**, entity names are **highlighted**, so their entry is one glance away.
- A piece shows which entities appear in it — **detected from the text, or attached manually** by the author.
- From an entity's entry, the author can see every piece it appears in, and jump straight there.

### Timeline

Especially for historical novels, the fictional story is woven around events that really happened in time. The author can start *in the timeline*, create pieces from there (they join the pool like any other piece, and reach the outline only when the author is ready), and arrange them in correct chronological relation.

Time in Weaveling is an **abstract order**, not a calendar:

- Pieces carry **before / after** relations to one another — like a linked list, but pieces may also sit **in parallel** (happening at the same time).
- Pieces can be tagged into **time buckets** of any precision — "the year 1737", "April 24th 1737", or fuzzy ones like "early spring". Buckets give a place to attach time information without forcing exact dates, so the same model serves real history and invented calendars alike.

Because narrative order and chronological order differ (flashbacks, non-linear storytelling), the timeline is a genuine second ordering over the same pieces — not just a re-sort of the outline.

### Threads

A **thread** is a colored strand followed through the manuscript — a subplot, a motif, a mystery and its clues. Overlaid on the outline, a thread shows which pieces carry it, and where it goes quiet for too long. This is the most literally *Weaveling* view: it lets the author see and tend the individual strands running through the weave.

### Research / Sources

A place to collect references, quotes, images, and links — especially for historical work — and pin them to the pieces they inform. Distinct from the Codex: the Codex is the author's invented world; Research is the real-world material the story draws from.

## Export

Typesetting the finished weave is an **export function**, not a view. The in-order walk of the outline is rendered into a polished artifact — **PDF, EPUB**, or other formats — the moment the mess becomes a book. This is where the typography obsession pays off.

## How It's Built

Weaveling is a browser-based client–server app: a Rust modular-monolith backend, event-sourcing for structure and CRDTs for prose (so editing is real-time collaborative *and* works offline). Which database backs it is deliberately still open. See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full picture, and [ROADMAP.md](./ROADMAP.md) for what's actually being built right now.

## Getting Started

### Prerequisites

- **Rust**, via [rustup](https://rustup.rs). The toolchain, the `rustfmt`/`clippy` components and the `wasm32-unknown-unknown` target are all declared in `rust-toolchain.toml` and installed automatically on first build — nothing to add by hand.
- **[Trunk](https://trunkrs.dev)**, the WASM bundler for the web client. Cargo can't declare tool dependencies, so this one is manual:

  ```bash
  cargo install --locked trunk
  ```

- **Node**, only for the client's end-to-end tests. Not needed to build or run anything:

  ```bash
  cd clients/web && npm install && npx playwright install chromium
  ```

### Running it for development

Two processes, two terminals.

**Terminal 1 — the API** on `http://127.0.0.1:3000`:

```bash
cargo run -p weaveling-service-api
```

`TraceLayer` emits at DEBUG while `tracing_subscriber` defaults to INFO, so you get no request log unless you ask for one:

```bash
RUST_LOG=info,tower_http=debug cargo run -p weaveling-service-api
```

**Terminal 2 — the web client** on `http://localhost:8080`:

```bash
cd clients/web
trunk serve
```

Then **open http://localhost:8080** — that's the one you want. Trunk rebuilds and reloads on save, and proxies `/api` through to the API, so everything is same-origin and there is no CORS to configure. Hitting `:3000` directly gives you the API but no UI.

Start the API first if you care about the first paint; otherwise the client shows its error banner until the API answers and a reload picks it up. Rust changes on the server need a manual restart (or `cargo watch -x 'run -p weaveling-service-api'`); client changes are live.

You can create, rename and delete projects. State lives in memory, so restarting the API empties it — see [ROADMAP.md](./ROADMAP.md) for where a real database comes in.

### Testing the client

The client is tested end to end in a real browser, because the two client bugs that actually shipped were browser behavior rather than logic — see [Client conventions](./ARCHITECTURE.md#client-conventions).

```bash
cd clients/web
npm test
```

Playwright starts the API and Trunk if they aren't running and reuses them if they are, so this works whether or not you already have the dev servers up. The suite takes a few seconds.

Two things to know before writing more of these. Selectors are **role plus accessible name** only — never a CSS class or an index — so restyling can't break a test. And the API is in memory and shared for the whole run, so no test may assume an empty list; each one makes its own uniquely-named project.

### Common commands

| Command | Does |
|---|---|
| `cargo build` | Builds the server-side crates. The web client is excluded from `default-members` — Trunk builds it, for a different target. |
| `cargo test` | Runs the workspace test suite. |
| `cargo fmt --all` | Formats everything. |
| `cargo clippy --workspace --all-targets` | Lints the server side. **Does not cover the client** — `--workspace` doesn't build for `wasm32`. |
| `cargo clippy -p weaveling-client-web --target wasm32-unknown-unknown` | Lints the client. Needed as a separate command, per the row above. |
| `cargo check -p weaveling-client-web --target wasm32-unknown-unknown` | Type-checks the client without invoking Trunk. |
| `trunk build --release` | Produces the optimised client bundle in `clients/web/dist`. |
| `cd clients/web && npm test` | Runs the client's end-to-end tests in a real browser. Starts the API and Trunk itself if they aren't already up, and reuses them if they are. |

### Layout

A Cargo workspace: `clients/`, `services/`, `features/`, `libraries/`. Each feature is an onion — a dependency-free `core`, wrapped in a ring of adapter crates, plus a WASM-safe `contract` crate shared with the client. The rules for what may depend on what are in [ARCHITECTURE.md](./ARCHITECTURE.md#repository-structure).

## Status

Early days — this document captures the dream. The solution is still being woven.
