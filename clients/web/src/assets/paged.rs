use std::future::Future;

use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::asking::Asking;
use crate::http::ApiError;

/// A list read a page at a time: what is shown, the cursor of the next page, and whether a page is loading.
/// Asking again keeps only the newest answer, so a page asked for before the list was read anew never lands in it.
pub struct Paged<T: Send + Sync + 'static> {
    pub listed: RwSignal<Vec<T>>,
    pub next: RwSignal<Option<String>>,
    pub loading: RwSignal<bool>,
    asking: Asking,
}

impl<T: Send + Sync + 'static> Clone for Paged<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for Paged<T> {}

impl<T: Clone + Send + Sync + 'static> Paged<T> {
    pub fn new() -> Self {
        Self {
            listed: RwSignal::new(Vec::new()),
            next: RwSignal::new(None),
            loading: RwSignal::new(false),
            asking: Asking::new(),
        }
    }

    /// Loads a page with `fetch`, which is given the cursor.
    /// Without a cursor the page replaces the list, with one it is added to it.
    /// A failure is handed to `refused`, and leaves the list as it was.
    pub fn load<Fu>(
        self,
        after: Option<String>,
        refused: impl FnOnce(ApiError) + 'static,
        fetch: impl FnOnce(Option<String>) -> Fu + 'static,
    ) where
        Fu: Future<Output = Result<(Vec<T>, Option<String>), ApiError>> + 'static,
    {
        let asked = self.asking.next();
        let adding = after.is_some();
        self.loading.set(true);

        spawn_local(async move {
            let told = fetch(after).await;
            if !self.asking.is_latest(asked) {
                return;
            }
            match told {
                Ok((page, next)) => {
                    if adding {
                        self.listed.update(|shown| shown.extend(page));
                    } else {
                        self.listed.set(page);
                    }
                    self.next.set(next);
                }
                Err(failed) => refused(failed),
            }
            self.loading.set(false);
        });
    }

    /// Whether there is nothing to show: no entries, no further page, and nothing loading.
    pub fn nothing(self) -> bool {
        self.listed.with(Vec::is_empty) && self.next.with(Option::is_none) && !self.loading.get()
    }
}
