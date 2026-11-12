use std::path::{Path, PathBuf};
use std::sync::Arc;

use assets_catalog::InMemoryAssetCatalog;
use assets_contents::{FolderContents, InMemoryContents};
use assets_core::{AssetCatalog, AssetEvent, AssetsCore, Contents};
use assets_images::WebpImages;
use axum::Router;
use eventsourcing::EventStore;
use outbox::Outbox;
use transacting::Transactions;
use wiring::{Assembled, Context, EventSourced, Feature, Storage, Unprepared, Wired};

pub use assets_core::Limits;

const FOLDER: &str = "assets";

pub struct Ports {
    pub events: Arc<dyn EventStore<AssetEvent>>,
    pub transactions: Arc<dyn Transactions>,
    pub catalog: Arc<dyn AssetCatalog>,
    pub contents: Arc<dyn Contents>,
    pub outbox: Arc<dyn Outbox>,
    pub limits: Limits,
}

/// What the operator chose for assets: the limits they are held to, and a folder of their own to keep them in, in place of the database or the data's own folder.
#[derive(Debug, Clone, Default)]
pub struct Keeping {
    pub limits: Limits,
    pub folder: Option<PathBuf>,
}

pub fn core(ports: &Ports, context: &Context) -> AssetsCore {
    AssetsCore::new(
        ports.events.clone(),
        ports.transactions.clone(),
        ports.catalog.clone(),
        ports.contents.clone(),
        Arc::new(WebpImages),
        context.authorizer.clone(),
        context.clock.clone(),
    )
    .limited(ports.limits)
}

pub fn wire(_ports: &Ports, _context: &Context) -> Wired {
    // TODO: serve the assets' routes through core(ports, context) once they are written, in 10d-1b
    Wired::serving(Router::new())
}

pub async fn assembled_assets(
    storage: &Storage,
    context: &Context,
    keeping: Keeping,
) -> Result<Assembled, Unprepared> {
    let ports = wiring::ports::<AssetFeature>(storage, context).await?;
    let ports = Ports {
        limits: keeping.limits,
        contents: match keeping.folder {
            Some(folder) => Arc::new(FolderContents::new(folder)),
            None => ports.contents,
        },
        ..ports
    };
    let wired = AssetFeature::wire(&ports, context);

    Ok(Assembled {
        outbox: Some(ports.outbox.clone()),
        routes: wired.routes,
        open_routes: wired.open_routes,
        topics: wired.topics,
        listeners: wired.listeners,
        settling: wired.settling,
        chores: wired.chores,
    })
}

/// Where the data's own folder keeps the assets: a folder beside the feature's database file.
pub fn beside(database: &Path) -> PathBuf {
    database
        .parent()
        .map(|data| data.join(FOLDER))
        .unwrap_or_else(|| PathBuf::from(FOLDER))
}

pub struct AssetFeature;

impl Feature for AssetFeature {
    const NAME: &'static str = "assets";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::in_memory(assets_messaging::message_for, context);

        Ports {
            events,
            transactions,
            catalog: Arc::new(InMemoryAssetCatalog::new()),
            contents: Arc::new(InMemoryContents::new()),
            outbox,
            limits: Limits::default(),
        }
    }

    #[cfg(feature = "postgres")]
    fn on_postgres(pool: sqlx::PgPool, context: &Context) -> Result<Ports, Unprepared> {
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_postgres(
            &pool,
            assets_store::codec(),
            assets_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(assets_catalog::PostgresAssetCatalog::new()),
            contents: Arc::new(assets_contents::PostgresContents::new(pool)),
            outbox,
            limits: Limits::default(),
        })
    }

    #[cfg(feature = "postgres")]
    fn postgres_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::postgres::migrations(),
            outbox::postgres::migrations(),
            assets_catalog::postgres::migrations(),
            assets_contents::postgres::migrations(),
        ]
    }

    #[cfg(feature = "sqlite")]
    fn on_sqlite(pool: sqlx::SqlitePool, context: &Context) -> Result<Ports, Unprepared> {
        let folder = beside(pool.connect_options().get_filename());
        let EventSourced {
            events,
            transactions,
            outbox,
        } = EventSourced::on_sqlite(
            &pool,
            assets_store::codec(),
            assets_messaging::message_for,
            context,
        );

        Ok(Ports {
            events,
            transactions,
            catalog: Arc::new(assets_catalog::SqliteAssetCatalog::new()),
            contents: Arc::new(FolderContents::new(folder)),
            outbox,
            limits: Limits::default(),
        })
    }

    #[cfg(feature = "sqlite")]
    fn sqlite_schema() -> Vec<sqlx::migrate::Migrator> {
        vec![
            eventsourcing::sqlite::migrations(),
            outbox::sqlite::migrations(),
            assets_catalog::sqlite::migrations(),
        ]
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        wire(ports, context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_s_own_folder_keeps_the_assets_beside_their_database() {
        // expect: a folder named for the assets beside the file
        assert_eq!(
            beside(Path::new("/var/lib/weaveling/assets.sqlite")),
            PathBuf::from("/var/lib/weaveling/assets")
        );
        // and: beside where the server runs when the file is named alone
        assert_eq!(beside(Path::new("assets.sqlite")), PathBuf::from("assets"));
    }
}
