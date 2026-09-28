# test-harness

A throwaway PostgreSQL schema per test, so the database suite runs in parallel and no test can see another's rows.

`PostgresFixture::setup()` claims a name like `fixture_1758000000_a3f9c210`, and `create_schema("pieces")` makes one namespace per feature under it — because [each feature keeps its data apart](../../ARCHITECTURE.md#dependency-rules), and a test that proves it needs the same separation. `cleanup()` drops them.

**A killed test leaves its schema behind**, so setup also sweeps anything older than an hour. That is deliberately not a `Drop` impl: dropping a schema is async, and a panicking test is exactly the case where nothing gets a chance to run.

It reads `DATABASE_URL`, defaulting to the local `compose.yaml`. When it cannot connect it says `try docker compose up -d` — which is almost always the real answer, and worth reading before diagnosing anything subtler.
