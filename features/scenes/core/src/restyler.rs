use async_trait::async_trait;
use quotation::QuotationMarks;

use crate::{SceneId, SceneServiceError};

#[async_trait]
pub trait SceneRestyler: Send + Sync {
    async fn restyle(
        &self,
        scene: SceneId,
        style: QuotationMarks,
    ) -> Result<bool, SceneServiceError>;
}
