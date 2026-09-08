use sqlx::migrate::Migrator;

const LEDGER: &str = "_sqlx_migrations_events";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ledger_is_renamed_at_all() {
        assert_eq!(migrations().table_name, LEDGER);
    }

    #[test]
    fn the_ledger_is_never_renamed_to_the_one_a_feature_will_want() {
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
    }

    #[test]
    fn there_is_something_to_lay_down() {
        assert!(!migrations().migrations.is_empty());
    }
}
