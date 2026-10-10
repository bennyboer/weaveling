use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RefusalDTO {
    pub id: i64,
    pub listener: String,
    pub routing: String,
    pub attempts: i32,
    pub why: String,
    pub occurred_at: String,
    pub given_up_at: String,
    pub acknowledged_at: Option<String>,
}
