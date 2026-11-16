use assets_core::{AssetEvent, Format, ImageInfo, Owner, ProjectLink};
use eventsourcing::{AccountId, Codec};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredAssetEvent {
    Uploaded {
        owner: StoredOwner,
        #[serde(rename = "picture")]
        image: StoredImageInfo,
        total_size: u64,
    },
    Deleted,
    Snapshotted {
        owner: StoredOwner,
        #[serde(rename = "picture")]
        image: StoredImageInfo,
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
struct StoredImageInfo {
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
                image,
                total_size,
            } => Self::Uploaded {
                owner: StoredOwner::from(owner),
                image: StoredImageInfo::from(image),
                total_size: *total_size,
            },
            AssetEvent::Deleted => Self::Deleted,
            AssetEvent::Snapshotted {
                owner,
                image,
                total_size,
                uploaded_at,
                deleted_at,
            } => Self::Snapshotted {
                owner: StoredOwner::from(owner),
                image: StoredImageInfo::from(image),
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
                image,
                total_size,
            } => Self::Uploaded {
                owner: Owner::from(owner),
                image: ImageInfo::from(image),
                total_size,
            },
            StoredAssetEvent::Deleted => Self::Deleted,
            StoredAssetEvent::Snapshotted {
                owner,
                image,
                total_size,
                uploaded_at,
                deleted_at,
            } => Self::Snapshotted {
                owner: Owner::from(owner),
                image: ImageInfo::from(image),
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

impl From<&ImageInfo> for StoredImageInfo {
    fn from(image: &ImageInfo) -> Self {
        Self {
            format: StoredFormat::from(image.format),
            width: image.width,
            height: image.height,
            original_size: image.original_size,
            variants: image.variants.clone(),
        }
    }
}

impl From<StoredImageInfo> for ImageInfo {
    fn from(stored: StoredImageInfo) -> Self {
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

    fn an_image(format: Format) -> ImageInfo {
        ImageInfo {
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
                image: an_image(Format::Jpeg),
                total_size: 400_000,
            },
            AssetEvent::Uploaded {
                owner: Owner::Account(AccountId::from("account_ada")),
                image: an_image(Format::Png),
                total_size: 1,
            },
            AssetEvent::Deleted,
            AssetEvent::Snapshotted {
                owner: Owner::Project(ProjectLink::from("project_loom")),
                image: an_image(Format::Gif),
                total_size: 400_000,
                uploaded_at: OffsetDateTime::from_unix_timestamp(1_700_000_000)
                    .expect("a plausible moment"),
                deleted_at: None,
            },
            AssetEvent::Snapshotted {
                owner: Owner::Account(AccountId::from("account_ada")),
                image: an_image(Format::Webp),
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
    fn an_upload_is_stored_with_its_image_under_the_name_it_always_had() {
        // given: an upload
        let uploaded = AssetEvent::Uploaded {
            owner: Owner::Project(ProjectLink::from("project_loom")),
            image: an_image(Format::Png),
            total_size: 400_000,
        };

        // when: it is written
        let body = (codec().body)(&uploaded);

        // then: its image is stored as its picture
        assert_eq!(
            body["Uploaded"]["picture"]["width"], 1_280,
            "events already stored name it so, and must still read back"
        );
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
