use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RefusalDTO {
    pub id: i64,
    pub listener: String,
    pub routing: String,
    pub attempts: i32,
    pub why: String,
    pub plainly: Option<String>,
    pub occurred_at: String,
    pub given_up_at: String,
    pub acknowledged_at: Option<String>,
    pub may_be_tried_again: bool,
}

pub mod refused {
    pub const TOO_LATE_TO_TRY_AGAIN: &str = "too-late-to-try-again";

    pub const EVERY: [&str; 1] = [TOO_LATE_TO_TRY_AGAIN];
}
