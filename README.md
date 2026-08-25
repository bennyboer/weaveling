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

A project is deliberately flexible about scope: it can be a single book, or a whole multi-book series. The root node of a project might be one book, or a series whose children are the individual books — that choice is left to the author.

## The Core Idea: Warp & Weft

Underneath every view is a single shared model — one fabric, seen from different angles.

- The **structure** is the *warp*: the vertical threads, the spine of the book.
- **Time**, **codex entities**, and **threads** are the *weft*: they run crossways through the structure.
- A **node** is where warp meets weft — where a character appears, in a scene, at a moment in time.

The views are not separate tools stapled together; they are lenses onto the same weave. This is why they stay linked: navigating from one to another is just turning the fabric to catch the light differently.

## The Views

### Structure view

The book drilled down hierarchically as a **tree**. The root is the project (a book, or a series). Beneath it come the books or the crude shape — classically beginning, main part, ending — and the author keeps splitting nodes as far as the budding idea allows. Detail is added *just in time*, not up front.

The finest useful grain is roughly **one node per paragraph** — the maximum depth of structural detail.

**Prose lives on every node.** A node can hold text at any level. When a node that already has text is split, the author decides what happens to that text: move parts of it down into the new child nodes, or leave it on the parent. Reading the book is an in-order walk that stitches every node's text together.

**Inbox.** Nodes don't have to be born inside the structure. An inbox holds nodes that exist but haven't been placed yet (for example, nodes created in the timeline). The author picks them from the inbox and drops them into the tree at the right spot.

### Text view

Where the actual prose is written. Deliberately **distraction-free**: just the text, in a large, comfortable font, so the author can concentrate on the words.

Linked to the structure: e.g. double-clicking a node in the structure view dives into that node's text. Codex entity names (see below) are **highlighted** inline, so their sheet is one glance away while writing dialogue.

### Codex

Every book has a cast — and a world. The **Codex** is the compendium of everything in the story: **characters** first and foremost, but also **locations, factions, magic/tech systems, artifacts**, and anything else worth tracking. They share one shape: fields, images, milestones, relations, free text, and back-links to the nodes they appear in.

The Codex is linked to the other views:

- Each entity keeps an **alias list** (names, titles, nicknames), used to detect mentions in the prose.
- In the **text view**, entity names are **highlighted**, so their entry is one glance away.
- A node shows which entities appear in it — **detected from the text, or attached manually** by the author.
- From an entity's entry, the author can see every node/section/paragraph it appears in, and jump straight there.

### Timeline

Especially for historical novels, the fictional story is woven around events that really happened in time. The author can start *in the timeline*, create nodes from there (they land in the inbox until placed in the structure), and arrange them in correct chronological relation.

Time in Weaveling is an **abstract order**, not a calendar:

- Nodes carry **before / after** relations to one another — like a linked list, but nodes may also sit **in parallel** (happening at the same time).
- Nodes can be tagged into **time buckets** of any precision — "the year 1737", "April 24th 1737", or fuzzy ones like "early spring". Buckets give a place to attach time information without forcing exact dates, so the same model serves real history and invented calendars alike.

Because narrative order and chronological order differ (flashbacks, non-linear storytelling), the timeline is a genuine second ordering over the same nodes — not just a re-sort of the tree.

### Threads

A **thread** is a colored strand followed through the manuscript — a subplot, a motif, a mystery and its clues. Overlaid on the structure, a thread shows which scenes carry it, and where it goes quiet for too long. This is the most literally *Weaveling* view: it lets the author see and tend the individual strands running through the weave.

### Research / Sources

A place to collect references, quotes, images, and links — especially for historical work — and pin them to the nodes they inform. Distinct from the Codex: the Codex is the author's invented world; Research is the real-world material the story draws from.

## Export

Typesetting the finished weave is an **export function**, not a view. The in-order walk of the structure is rendered into a polished artifact — **PDF, EPUB**, or other formats — the moment the mess becomes a book. This is where the typography obsession pays off.

## How It's Built

Weaveling is a browser-based client–server app: a Rust modular-monolith backend, PostgreSQL, event-sourcing for structure and CRDTs for prose (so editing is real-time collaborative *and* works offline). See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full picture, and [ROADMAP.md](./ROADMAP.md) for what's actually being built right now.

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

You can create, rename and delete projects. State lives in memory, so restarting the API empties it — see [ROADMAP.md](./ROADMAP.md) for where PostgreSQL comes in.

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
