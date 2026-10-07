use sqlx::migrate::Migrator;

const LEDGER: &str = "_sqlx_migrations_outbox";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}
