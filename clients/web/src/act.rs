use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task;

type Doing<T> = Rc<dyn Fn(T, Here) -> Pin<Box<dyn Future<Output = ()>>>>;

/// An action that a click or a key press starts.
///
/// Only one run happens at a time.
/// Starting it again while it runs does nothing, so a double click or a held key never runs it twice.
/// The value it runs with, for example the section being written into, decides which of several buttons shows as busy.
pub struct Act<T: Send + Sync + 'static = ()> {
    running: RwSignal<Option<T>>,
    /// Holds nothing. It only exists as long as the view the act was built in.
    ///
    /// Like every signal, it belongs to the reactive owner that is current at `Act::new`.
    /// Leptos disposes it when that view is removed from the page, while the act's task keeps running.
    /// So build an act in the view whose removal should stop it, not higher up.
    here: StoredValue<()>,
    doing: StoredValue<Doing<T>, LocalStorage>,
}

impl<T: Send + Sync + 'static> Clone for Act<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for Act<T> {}

/// Tells whether the view an act was started from is still shown.
/// This way, what the act does after its answer comes, such as navigating elsewhere, is skipped if the user has already left.
#[derive(Clone, Copy)]
pub struct Here(StoredValue<()>);

impl<T: Clone + PartialEq + Send + Sync + 'static> Act<T> {
    pub fn new<Fu>(doing: impl Fn(T, Here) -> Fu + 'static) -> Self
    where
        Fu: Future<Output = ()> + 'static,
    {
        let doing: Doing<T> = Rc::new(move |about, here| Box::pin(doing(about, here)));

        Self {
            running: RwSignal::new(None),
            here: StoredValue::new(()),
            doing: StoredValue::new_local(doing),
        }
    }

    pub fn start(self, about: T) {
        if self.running.with_untracked(Option::is_some) {
            return;
        }
        let Some(doing) = self.doing.try_get_value() else {
            return;
        };
        self.running.set(Some(about.clone()));
        let done = doing(about, Here(self.here));

        task::spawn_local(async move {
            done.await;
            _ = self.running.try_set(None);
        });
    }

    pub fn running(self) -> bool {
        self.running.with(Option::is_some)
    }

    /// The `aria-busy` value for the button that starts the act.
    /// It is set while the act runs, and it also makes the button look busy.
    pub fn busy(self) -> Option<&'static str> {
        self.running().then_some("true")
    }

    pub fn busy_on(self, about: &T) -> Option<&'static str> {
        self.running
            .with(|running| running.as_ref() == Some(about))
            .then_some("true")
    }
}

impl Here {
    /// Returns true while the view is shown.
    /// Reading a disposed value returns nothing, which is how a removed view is noticed.
    pub fn still(self) -> bool {
        self.0.try_get_value().is_some()
    }
}
