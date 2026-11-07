use super::AccountsCore;
use crate::key_ceremonies::KeyCeremonies;

impl AccountsCore {
    pub fn holding_key_ceremonies(self, ceremonies: KeyCeremonies) -> Self {
        Self {
            key_ceremonies: Some(ceremonies),
            ..self
        }
    }

    pub fn keeps_keys(&self) -> bool {
        self.key_ceremonies.is_some()
    }
}
