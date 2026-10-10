use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub id: i64,
    pub listener: String,
    pub routing: String,
    pub attempts: i32,
    pub why: String,
    pub plainly: Option<String>,
    pub given_up_at: OffsetDateTime,
    pub acknowledged: bool,
}
