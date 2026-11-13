# test-harness

A throwaway PostgreSQL schema per test, or a throwaway directory of SQLite files. So the database suites run in parallel, and no test can see another test's rows.

`PostgresFixture::setup()` claims a name like `fixture_1758000000_a3f9c210`. `create_schema("ideas")` creates one namespace per feature under that name. The reason: [each feature keeps its data apart](../../ARCHITECTURE.md#dependency-rules), and a test that proves this needs the same separation. `cleanup()` drops the schemas.

The ideas catalog is tested on a schema of its own:

```rust
use test_harness::PostgresFixture;

let fixture = PostgresFixture::setup().await;
let pool = fixture.create_schema("ideas").await;
ideas_catalog::postgres::migrations()
    .run(&pool)
    .await
    .expect("the schema should lay down in an empty namespace");

let catalog = PostgresIdeaCatalog::new(pool);

fixture.cleanup().await;
```

**Only two fixtures run at once.** Each schema gets its own pool of up to five connections. So a whole-service test holds one pool per feature, which is seven today. Nine of those tests in parallel asked for more connections than the server's hundred. `setup()` waits for its turn. `cleanup()`, or a panic that drops the fixture, passes the turn on. Two fixtures with seven pools each need at most 72 connections. Beyond nine features, lower the pool size or the number of turns.

A test that needs two fixtures at once takes them with `two()`, which takes both turns in one go. Calling `setup()` twice would hold one turn while waiting for another. Two such tests would deadlock.

**A killed test leaves its schema behind.** So setup also sweeps away anything older than an hour. This is deliberately not a `Drop` impl. Dropping a schema is async. And a panicking test is exactly the case where nothing gets a chance to run.

**`SqliteFixture` is the same idea for local mode.**

- `setup()` claims a directory under the system's temp directory, named like a schema stem.
- `create_database("ideas")` opens `ideas.sqlite` in it.
- `cleanup()` closes every pool it handed out, and then removes the directory. On Windows an open pool keeps its file locked.

It needs no server and takes no turn. A killed test's directory is swept away after an hour, the same rule as for a schema.

It opens files with the same settings that `wiring::sqlite` uses: a write-ahead journal, foreign keys on, and a busy timeout. These settings are repeated, not shared. The reason: the harness is a dev-dependency of `eventsourcing`, and `wiring` depends on `eventsourcing`. A test pins the settings, so the two copies cannot drift apart silently.

It reads `DATABASE_URL`, and defaults to the local `compose.yaml`. When it cannot connect, it says `try docker compose up -d`. That is almost always the real answer. It is worth reading before looking for anything more subtle.
