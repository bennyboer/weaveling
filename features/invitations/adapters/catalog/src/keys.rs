use async_trait::async_trait;
use invitations_core::{CatalogError, TokenKey, TokenKeys};

#[derive(Debug)]
pub struct InMemoryTokenKeys {
    key: TokenKey,
}

impl InMemoryTokenKeys {
    pub fn new() -> Self {
        Self {
            key: TokenKey::drawn(),
        }
    }
}

impl Default for InMemoryTokenKeys {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TokenKeys for InMemoryTokenKeys {
    async fn key(&self) -> Result<TokenKey, CatalogError> {
        Ok(self.key.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_process_keeps_one_key_and_another_draws_its_own() {
        // given: the keys of a process
        let keys = InMemoryTokenKeys::new();

        // then: it keeps one key
        assert_eq!(
            keys.key().await.expect("a key is at hand"),
            keys.key().await.expect("a key is at hand")
        );
        // and: another process draws its own
        assert_ne!(
            keys.key().await.expect("a key is at hand"),
            InMemoryTokenKeys::new()
                .key()
                .await
                .expect("a key is at hand"),
            "a link mailed before a restart in memory leads nowhere after it"
        );
    }
}
