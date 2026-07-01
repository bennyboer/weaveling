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

A **thread** is a coloured strand followed through the manuscript — a subplot, a motif, a mystery and its clues. Overlaid on the structure, a thread shows which scenes carry it, and where it goes quiet for too long. This is the most literally *Weaveling* view: it lets the author see and tend the individual strands running through the weave.

### Research / Sources

A place to collect references, quotes, images, and links — especially for historical work — and pin them to the nodes they inform. Distinct from the Codex: the Codex is the author's invented world; Research is the real-world material the story draws from.

## Export

Typesetting the finished weave is an **export function**, not a view. The in-order walk of the structure is rendered into a polished artifact — **PDF, EPUB**, or other formats — the moment the mess becomes a book. This is where the typography obsession pays off.

## How It's Built

Weaveling is a browser-based client–server app: a Rust modular-monolith backend, PostgreSQL, event-sourcing for structure and CRDTs for prose (so editing is real-time collaborative *and* works offline). See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full picture.

## Status

Early days — this document captures the dream. The solution is still being woven.
