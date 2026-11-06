use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondFactor {
    pub since: OffsetDateTime,
    pub recovery_codes_left: usize,
}
