#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SigningUp {
    #[default]
    Open,
    ByInvitation,
}
