# Weaveling — TODO

Scratch roadmap. Not committed order — see [README.md](./README.md) (the dream) and [ARCHITECTURE.md](./ARCHITECTURE.md) (the how) for settled decisions.

## Next up (start here tomorrow)

- [ ] **Build a proper roadmap.** Gut says do this first — turn the notes below into a sequenced plan before writing code.

## Candidate first steps (once roadmap exists)

1. [ ] **Domain model** — sketch the core aggregates & events: Project, Node, structural events (create / move / split / reorder), the codex entity shape, threads, time relations (before/after + buckets). This is the heart of the ES design.
2. [ ] **Scaffold the repo** — Cargo workspace, module skeleton for the modular monolith, pick the full-stack Rust framework (Leptos / Dioxus / Yew), stand up a "hello weave" client + server.
3. [ ] **Spike the risky bits** — prove the scary assumptions early:
   - [ ] yrs ↔ Yjs collaborative editing round-trip (the highest-risk assumption in the whole plan).
   - [ ] embed Typst → compile a `.typ` to PDF.
4. [ ] **Keep designing** — exact event catalogue, node text split-and-move mechanics, how threads are represented.

Suggested order was **#1 → #3**: nail the domain vocabulary while fresh, then de-risk the CRDT round-trip.

## Open design threads (not yet resolved)

- [ ] When a node with text is split: exact UX/data for moving ranges into children vs. leaving on parent.
- [ ] "Present in a scene" vs. "merely mentioned" — do we model both relation types?
- [ ] Time model details: representing parallel nodes + nested time buckets (year ⊃ month ⊃ day).
- [ ] Frontend framework choice within full-stack Rust (Leptos vs Dioxus vs Yew).
- [ ] Event versioning/upcasting strategy.
- [ ] Auth / accounts / project membership / permissions (deferred, but looming for multi-author).

## Parked (decided — don't re-litigate)

- Rust, modular monolith, PostgreSQL, no broker yet (option kept open).
- Two-speed model: ES/CQRS for structure, CRDT (yrs/Yjs) for prose.
- Local-first-capable client.
- Full-stack Rust first; Angular as fallback.
- Export: Typst → PDF, HTML → EPUB.
