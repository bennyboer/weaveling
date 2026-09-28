# Conventions

How this codebase is written, so that a reviewer never has to explain the same thing twice. [ARCHITECTURE.md](./ARCHITECTURE.md) says what the parts are and why; this says how they are spelled.

## Naming

### The register depends on the layer

Weaveling's domain language is deliberate — a piece is *captured*, a project is *started*, an outline *attaches* a piece to a section — and that language belongs in the domain: aggregates, commands, events, errors, and the service methods an author's action maps onto. `capture`, `retitle`, `discard` are right, and `create`, `update`, `delete` would be worse.

**Below the domain it is the wrong register.** Adapters, wiring and libraries are plumbing, and plumbing wants a plain technical name that says what the thing produces or does. Evocative naming there does not add meaning; it hides the type.

| Layer | Register | Example |
| --- | --- | --- |
| `core` — aggregate, command, event, error, service | The domain's own words | `PieceCommand::Capture`, `ProjectEvent::Started` |
| `adapters/*`, `wiring`, `libraries/*` | Plain and technical | `to_dto`, `to_response`, `ProjectEventPublisher` |
| Test function names | A sentence that states the claim | `a_notification_delivers_long_before_the_next_poll_would` |
| Test helpers | Read at the call site, article-prefixed | `a_title()`, `an_author()`, `at(1_000)` |

### A converter is named for what it returns

`to_<thing>`. Not for what the caller happens to be doing with it, and not as a participle describing the result.

```rust
fn to_dto(summary: &ProjectSummary) -> ProjectDTO     // yes
fn to_response(status, id, standing) -> Response      // yes
fn to_summary(row: &PgRow) -> Result<ProjectSummary>  // yes

fn listed(summary: &ProjectSummary) -> ProjectDTO     // no — names the caller's use
fn reported(status, id, standing) -> Response         // no — veils what it produces
fn summarised(row: &PgRow) -> Result<ProjectSummary>  // no — participle for a conversion
```

Rust reserves `to_*` for methods by convention, but these are free functions passed to `map`, where `found.iter().map(to_dto)` reads exactly right.

**If two converters in one file both want the name, that is the signal to have one.** `listed` existed only because `to_dto` was taken by a single-use converter beside it; inlining that one freed the name and left each adapter with exactly one.

### A type is named for what it is, spelled out

No bare abstract nouns. `Publishing` said nothing that `ProjectEventPublisher` does not say better, and it was the only bare word in an export list that already read `UnreadableProjectEvent`, `ProjectCatalogProjector`, `ProjectSummary`.

The shapes each feature repeats:

- `<Thing>Catalog`, `<Thing>Summary`, `<Thing>CatalogProjector`
- `<Thing>Event`, `<Thing>Command`, `<Thing>Error`, `<Thing>EventDTO`, `Stored<Thing>Event`
- `<Thing>EventPublisher`, `Unreadable<Thing>Event`
- `<Thing>Service`, `<Thing>ServiceError`
- `InMemory<Thing>Catalog`, `Postgres<Thing>Catalog`

Singular, built on the aggregate's own name — `ProjectEventPublisher`, not `ProjectsEventPublisher`. The crate is plural (`projects_messaging`); the types inside it are not.

### Reading something out of something else

`<thing>_in(<source>)` when a value is extracted from a message or envelope: `project_in(message)`, `piece_in(message)`, `event_in(message)`, `published_in(message)`. This is distinct from `to_*`, which converts a whole value rather than picking one out.

### Where participles do belong

As predicates and states, not as conversions: `is_deleted`, `is_snapshot`, `is_publishable`, `Delivery::Kept`. A participle answers *what is true of this*, never *what does this return*.

### Name the mechanism, not the feeling

The same trap in a different shape: a word chosen because it evokes the right *idea* rather than because it names what happens. It reads well once and tells a reader nothing the second time.

- `Nudges` / `outbox.nudges()` became **`Notifications`** / `outbox.notifications()`. Both backends already call it notify — PostgreSQL `pg_notify` and `LISTEN`, tokio's `Notify` — so "nudge" was a third word for a thing that had two perfectly good ones.
- `announce` became **`enqueue`**, because the in-memory outbox already had `enqueue` and the two were one operation under two names in one crate. What it does is put a row in a queue.

**Prose is the exception, and deliberately so.** A test name and an assertion message may keep the domain word — "a refused append announces nothing" says what an outbox row *means* to a reader, where `enqueue` says what the code *does*. Meaning in the prose, mechanism in the code; they are allowed to differ as long as each is in its own place.

## Comments

**There are none.** The code says what it does; the tests say what it is for. The single exception is an **assertion message**, which carries the *why* a plain reader could not recover:

```rust
assert!(
    until(|| heard.how_many() == 1).await,
    "polling is five minutes away, so only the notification can have woken it"
);
```

Say why the expectation holds or what breaks if it does not — never restate the assertion in prose.

## Tests

- The name is the claim, as a sentence: `a_deleted_project_refuses_everything`, `a_batch_that_collides_halfway_writes_none_of_itself`.
- Helpers are article-prefixed so call sites read as English: `a_project(&service, "Tapestry")`, `an_author()`, `a_workbench()`.
- A behaviour two backends must share is a **conformance suite**: free functions taking `&impl Trait`, a `Workbench` trait (`setup`/`store`/`cleanup`), and a `conformance_tests!` macro each backend invokes. One case list, both implementations.
- A guard worth having is worth proving load-bearing — break it deliberately and watch the test fail.

## Rust

- Import rather than qualify. `use sqlx::postgres::PgListener;`, never `sqlx::postgres::PgListener` inline. The exception is a one-off `std::time::Duration` beside a `time::Duration` already in scope.
- `thiserror` for every error type; `#[error(transparent)]` when wrapping another error whole.
- `expect` carries the reason it cannot fail: `.expect("a plain title is fine")`.

## SQL

- Statements are `const` items in SCREAMING_SNAKE, directly above the function that runs them. When a file grows past a couple of operations, give each operation its own file with its constant beside it.
- Id columns are `TEXT COLLATE "C"` — base62 ids only sort by age under byte order, and a glibc locale puts `a` before `A`.
- Every schema change is a migration file. `sqlx::migrate!` embeds them **at compile time** and cannot tell cargo the `.sql` files are inputs, so touching a `.rs` file in the crate is what makes a new migration take effect.
