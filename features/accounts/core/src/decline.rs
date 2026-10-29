#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    AddressHasAccount,
    TurnedDown,
}
