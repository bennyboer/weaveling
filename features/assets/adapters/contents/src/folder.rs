use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use assets_core::{AssetId, Contents, ContentsError, Piece};
use async_trait::async_trait;
use tokio::fs;

use crate::unreachable;

/// Stores each asset in a folder of its own under the root, with each piece as a file in it.
/// A piece is first written next to its final name and then moved into place.
/// So a piece that is read is never half written.
#[derive(Debug, Clone)]
pub struct FolderContents {
    root: PathBuf,
}

impl FolderContents {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn folder_of(&self, asset: &AssetId) -> PathBuf {
        self.root.join(asset.to_string())
    }
}

#[async_trait]
impl Contents for FolderContents {
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError> {
        let folder = self.folder_of(asset);
        fs::create_dir_all(&folder).await.map_err(unreachable)?;
        let written = folder.join(format!(".{}.part", piece.name()));
        fs::write(&written, bytes).await.map_err(unreachable)?;

        fs::rename(&written, folder.join(piece.name()))
            .await
            .map_err(unreachable)
    }

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError> {
        read_if_there(&self.folder_of(asset).join(piece.name())).await
    }

    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError> {
        match fs::remove_dir_all(self.folder_of(asset)).await {
            Err(failure) if failure.kind() != ErrorKind::NotFound => Err(unreachable(failure)),
            _ => Ok(()),
        }
    }

    async fn stored(
        &self,
        after: Option<&AssetId>,
        at_most: u64,
    ) -> Result<Vec<AssetId>, ContentsError> {
        let mut entries = match fs::read_dir(&self.root).await {
            Ok(entries) => entries,
            Err(failure) if failure.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(failure) => return Err(unreachable(failure)),
        };
        let after = after.map(ToString::to_string);
        let mut stored = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(unreachable)? {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if after.as_ref().is_some_and(|last| name <= *last) {
                continue;
            }
            if let Ok(asset) = name.parse::<AssetId>() {
                stored.push((name, asset));
            }
        }
        stored.sort_by(|one, other| one.0.cmp(&other.0));
        stored.truncate(usize::try_from(at_most).unwrap_or(usize::MAX));

        Ok(stored.into_iter().map(|(_, asset)| asset).collect())
    }
}

async fn read_if_there(path: &Path) -> Result<Option<Vec<u8>>, ContentsError> {
    match fs::read(path).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(failure) if failure.kind() == ErrorKind::NotFound => Ok(None),
        Err(failure) => Err(unreachable(failure)),
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::suite::Workbench;

    static FOLDERS: AtomicUsize = AtomicUsize::new(0);

    struct InAFolder(FolderContents, PathBuf);

    #[async_trait]
    impl Workbench for InAFolder {
        type Store = FolderContents;

        async fn setup() -> Self {
            let root = env::temp_dir().join(format!(
                "weaveling-asset-contents-{}-{}",
                std::process::id(),
                FOLDERS.fetch_add(1, Ordering::Relaxed)
            ));
            let _gone = fs::remove_dir_all(&root).await;

            Self(FolderContents::new(&root), root)
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {
            let _gone = fs::remove_dir_all(&self.1).await;
        }
    }

    crate::conformance_tests!(InAFolder);
}
