use assets_core::{AssetEvent, Format, Owner, Picture, ProjectLink};
use eventsourcing::{AccountId, Codec};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredAssetEvent {
    Uploaded {
        owner: StoredOwner,
        picture: StoredPicture,
        total_size: u64,
    },
    Deleted,
    Snapshotted {
        owner: StoredOwner,
        picture: StoredPicture,
        total_size: u64,
        #[serde(with = "time::serde::rfc3339")]
        uploaded_at: OffsetDateTime,
        #[serde(default, with = "time::serde::rfc3339::option")]
        deleted_at: Option<OffsetDateTime>,
    },
}

#[derive(Serialize, Deserialize)]
enum StoredOwner {
    Project(String),
    Account(String),
}

#[derive(Serialize, Deserialize)]
struct StoredPicture {
    format: StoredFormat,
    width: u32,
    height: u32,
    original_size: u64,
    variants: Vec<u32>,
}

#[derive(Serialize, Deserialize)]
enum StoredFormat {
    Jpeg,
    Png,
    Gif,
    Webp,
}

pub fn codec() -> Codec<AssetEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredAssetEvent::from(event))
                .expect("a stored asset event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredAssetEvent>(body)
                .ok()
                .map(AssetEvent::from)
        },
    }
}

impl From<&AssetEvent> for StoredAssetEvent {
    fn from(event: &AssetEvent) -> Self {
        match event {
            AssetEvent::Uploaded {
                owner,
                picture,
                total_size,
            } => Self::Uploaded {
                owner: StoredOwner::from(owner),
                picture: StoredPicture::from(picture),
                total_size: *total_size,
            },
            AssetEvent::Deleted => Self::Deleted,
            AssetEvent::Snapshotted {
                owner,
                picture,
                total_size,
                uploaded_at,
                deleted_at,
            } => Self::Snapshotted {
                owner: StoredOwner::from(owner),
                picture: StoredPicture::from(picture),
                total_size: *total_size,
                uploaded_at: *uploaded_at,
                deleted_at: *deleted_at,
            },
        }
    }
}

impl From<StoredAssetEvent> for AssetEvent {
    fn from(stored: StoredAssetEvent) -> Self {
        match stored {
            StoredAssetEvent::Uploaded {
                owner,
                picture,
                total_size,
            } => Self::Uploaded {
                owner: Owner::from(owner),
                picture: Picture::from(picture),
                total_size,
            },
            StoredAssetEvent::Deleted => Self::Deleted,
            StoredAssetEvent::Snapshotted {
                owner,
                picture,
                total_size,
                uploaded_at,
                deleted_at,
            } => Self::Snapshotted {
                owner: Owner::from(owner),
                picture: Picture::from(picture),
                total_size,
                uploaded_at,
                deleted_at,
            },
        }
    }
}

impl From<&Owner> for StoredOwner {
    fn from(owner: &Owner) -> Self {
        match owner {
            Owner::Project(project) => Self::Project(project.to_string()),
            Owner::Account(account) => Self::Account(account.to_string()),
        }
    }
}

impl From<StoredOwner> for Owner {
    fn from(stored: StoredOwner) -> Self {
        match stored {
            StoredOwner::Project(project) => Self::Project(ProjectLink::from(project.as_str())),
            StoredOwner::Account(account) => Self::Account(AccountId::from(account)),
        }
    }
}

impl From<&Picture> for StoredPicture {
    fn from(picture: &Picture) -> Self {
        Self {
            format: StoredFormat::from(picture.format),
            width: picture.width,
            height: picture.height,
            original_size: picture.original_size,
            variants: picture.variants.clone(),
        }
    }
}

impl From<StoredPicture> for Picture {
    fn from(stored: StoredPicture) -> Self {
        Self {
            format: Format::from(stored.format),
            width: stored.width,
            height: stored.height,
            original_size: stored.original_size,
            variants: stored.variants,
        }
    }
}

impl From<Format> for StoredFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Jpeg => Self::Jpeg,
            Format::Png => Self::Png,
            Format::Gif => Self::Gif,
            Format::Webp => Self::Webp,
        }
    }
}

impl From<StoredFormat> for Format {
    fn from(stored: StoredFormat) -> Self {
        match stored {
            StoredFormat::Jpeg => Self::Jpeg,
            StoredFormat::Png => Self::Png,
            StoredFormat::Gif => Self::Gif,
            StoredFormat::Webp => Self::Webp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(event: AssetEvent) -> Option<AssetEvent> {
        let codec = codec();

        (codec.event)((codec.body)(&event))
    }

    fn a_picture(format: Format) -> Picture {
        Picture {
            format,
            width: 1_280,
            height: 1_024,
            original_size: 230_000,
            variants: vec![384, 768, 1_280],
        }
    }

    #[test]
    fn every_asset_event_reads_back_as_it_was_written() {
        // given: every asset event, for every owner and format
        for event in [
            AssetEvent::Uploaded {
                owner: Owner::Project(ProjectLink::from("project_loom")),
                picture: a_picture(Format::Jpeg),
                total_size: 400_000,
            },
            AssetEvent::Uploaded {
                owner: Owner::Account(AccountId::from("account_ada")),
                picture: a_picture(Format::Png),
                total_size: 1,
            },
            AssetEvent::Deleted,
            AssetEvent::Snapshotted {
                owner: Owner::Project(ProjectLink::from("project_loom")),
                picture: a_picture(Format::Gif),
                total_size: 400_000,
                uploaded_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                    .expect("a plausible moment"),
                deleted_at: None,
            },
            AssetEvent::Snapshotted {
                owner: Owner::Account(AccountId::from("account_ada")),
                picture: a_picture(Format::Webp),
                total_size: 9,
                uploaded_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                    .expect("a plausible moment"),
                deleted_at: Some(
                    OffsetDateTime::from_unix_timestamp(1_700_000_600).expect("a plausible moment"),
                ),
            },
        ] {
            // then: it reads back as it was written
            assert_eq!(round_trip(event.clone()), Some(event));
        }
    }

    #[test]
    fn what_is_no_asset_event_reads_as_nothing() {
        // expect: what is no asset event reads as nothing
        assert_eq!(
            (codec().event)(serde_json::json!({ "Forgotten": {} })),
            None
        );
    }
}
