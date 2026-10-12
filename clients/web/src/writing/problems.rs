use leptos::prelude::*;

use crate::http::ApiError;

#[derive(Clone, Copy)]
pub struct Problems {
    latest: RwSignal<Option<ApiError>>,
}

impl Problems {
    pub fn none() -> Self {
        Self {
            latest: RwSignal::new(None),
        }
    }

    pub fn report(self, failure: ApiError) {
        self.latest.set(Some(failure));
    }

    pub fn latest(self) -> Option<ApiError> {
        self.latest.get()
    }
}
