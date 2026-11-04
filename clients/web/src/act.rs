use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use leptos::prelude::*;
use leptos::task;

type Doing<T> = Rc<dyn Fn(T, Here) -> Pin<Box<dyn Future<Output = ()>>>>;

/// Something one click or key starts, once at a time: started again while it runs, it does nothing, so a double click or a held key never does it twice. What it is about, a section written into, says which of several buttons shows it busy.
pub struct Act<T: Send + Sync + 'static = ()> {
    running: RwSignal<Option<T>>,
    /// Holds nothing; it only lives as long as the view the act was built in. Like every signal it belongs to the reactive owner current at `Act::new`, and Leptos disposes it when that view is taken off the page, while the act's task runs on. So build an act in the view whose leaving should stop it, not higher up.
    here: StoredValue<()>,
    doing: StoredValue<Doing<T>, LocalStorage>,
}

impl<T: Send + Sync + 'static> Clone for Act<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: Send + Sync + 'static> Copy for Act<T> {}

/// Whether the view an act was started from is still shown, so that what it does once its answer comes, such as going elsewhere, is not done after its author left.
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

    /// The `aria-busy` of the button that starts it: set while it runs, which is also what draws the button as busy.
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
    /// True while the view is shown: reading a disposed value gives nothing back.
    pub fn still(self) -> bool {
        self.0.try_get_value().is_some()
    }
}
