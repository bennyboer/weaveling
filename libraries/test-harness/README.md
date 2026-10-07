# test-harness

A throwaway PostgreSQL schema per test, or a throwaway directory of SQLite files, so the database suites run in parallel and no test can see another's rows.

`PostgresFixture::setup()` claims a name like `fixture_1758000000_a3f9c210`, and `create_schema("ideas")` makes one namespace per feature under it — because [each feature keeps its data apart](../../ARCHITECTURE.md#dependency-rules), and a test that proves it needs the same separation. `cleanup()` drops them.

**Only two fixtures run at once.** Each schema gets its own pool of up to five connections, so a whole-service test holds one pool per feature — seven today — and nine of those in parallel asked for more than the server's hundred. `setup()` waits its turn, and `cleanup()` (or a panic dropping the fixture) hands it on. Two fixtures of seven pools need at most 72; past nine features, lower the pool size or the turns. A test that needs two fixtures at once takes them with `two()`, both turns in one go — calling `setup()` twice would hold one turn while waiting for another, and two such tests deadlock.

**A killed test leaves its schema behind**, so setup also sweeps anything older than an hour. That is deliberately not a `Drop` impl: dropping a schema is async, and a panicking test is exactly the case where nothing gets a chance to run.

**`SqliteFixture` is the same idea for local mode.** `setup()` claims a directory named like a schema stem under the system's temp directory, `create_database("ideas")` opens `ideas.sqlite` in it, and `cleanup()` closes every pool it handed out before removing the directory — on Windows an open pool keeps its file locked. It needs no server and takes no turn. A killed test's directory is swept after an hour, the same rule as a schema. It opens files with the settings `wiring::sqlite` uses — write-ahead journal, foreign keys on, a busy timeout — repeated rather than shared, because the harness is a dev-dependency of `eventsourcing`, which `wiring` depends on; a test pins them so the two cannot drift silently.

It reads `DATABASE_URL`, defaulting to the local `compose.yaml`. When it cannot connect it says `try docker compose up -d` — which is almost always the real answer, and worth reading before diagnosing anything subtler.
