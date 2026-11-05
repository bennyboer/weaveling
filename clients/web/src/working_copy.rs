use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use leptos::prelude::*;
use leptos::task;

use crate::http::ApiError;

const TRIES_AGAINST_NEWER: u8 = 3;

type Answer<A> = Pin<Box<dyn Future<Output = Result<A, ApiError>>>>;
type Sending<A, I> = Rc<dyn Fn(A, I) -> Answer<A>>;
type Reading<A> = Rc<dyn Fn() -> Answer<A>>;
type Failing = Rc<dyn Fn(ApiError)>;

/// An aggregate as the server confirmed it, at the version it stands at.
pub trait Versioned: Clone + Send + Sync + 'static {
    fn version(&self) -> u64;
}

/// What the author means to do to an aggregate: shown at once, sent in its turn, and laid over the confirmed state until the server takes it in or refuses it. Not a change the server tells, which is confirmed and carries a version.
pub trait Intent<A>: Clone + Send + Sync + 'static {
    fn lay_over(&self, on: &mut A);

    /// This intent and the one waiting before it, as one, where the newer makes the older's sending needless: a card nudged twice is sent once, where it ended up.
    fn merged_after(&self, earlier: &Self) -> Option<Self>;
}

/// What the author works on: the aggregate as the server last confirmed it, with the author's own intents laid over it until each is answered. They go out one at a time and in the order they were made, each built on the version it was worked out from and refused if another author got in between, and an intent waiting to go is merged with the next where it can be, so a held key sends what it ended at rather than every step.
pub struct WorkingCopy<A: Send + Sync + 'static, I: Send + Sync + 'static> {
    confirmed: RwSignal<Option<A>>,
    pending: RwSignal<Vec<I>>,
    sending: RwSignal<bool>,
    awaited: StoredValue<Vec<Option<Rc<Landing>>>, LocalStorage>,
    send: StoredValue<Sending<A, I>, LocalStorage>,
    read: StoredValue<Reading<A>, LocalStorage>,
    failed: StoredValue<Failing, LocalStorage>,
}

impl<A: Send + Sync + 'static, I: Send + Sync + 'static> Clone for WorkingCopy<A, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Send + Sync + 'static, I: Send + Sync + 'static> Copy for WorkingCopy<A, I> {}

impl<A: Versioned, I: Intent<A>> WorkingCopy<A, I> {
    /// `send` carries out one intent on the version it is given, `read` asks for the aggregate whole, and `failed` hears what was refused.
    pub fn new<Sent, Read>(
        send: impl Fn(A, I) -> Sent + 'static,
        read: impl Fn() -> Read + 'static,
        failed: impl Fn(ApiError) + 'static,
    ) -> Self
    where
        Sent: Future<Output = Result<A, ApiError>> + 'static,
        Read: Future<Output = Result<A, ApiError>> + 'static,
    {
        let send: Sending<A, I> = Rc::new(move |on, intent| Box::pin(send(on, intent)));
        let read: Reading<A> = Rc::new(move || Box::pin(read()));
        let failed: Failing = Rc::new(failed);

        Self {
            confirmed: RwSignal::new(None),
            pending: RwSignal::new(Vec::new()),
            sending: RwSignal::new(false),
            awaited: StoredValue::new_local(Vec::new()),
            send: StoredValue::new_local(send),
            read: StoredValue::new_local(read),
            failed: StoredValue::new_local(failed),
        }
    }

    /// The aggregate as the author sees it: confirmed, with every intent of theirs not yet answered laid over it.
    pub fn shown(self) -> Option<A> {
        self.confirmed.get().map(|mut shown| {
            self.pending.with(|pending| {
                for intent in pending {
                    intent.lay_over(&mut shown);
                }
            });

            shown
        })
    }

    pub fn confirmed(self) -> Option<A> {
        self.confirmed.get()
    }

    pub fn confirmed_untracked(self) -> Option<A> {
        self.confirmed.get_untracked()
    }

    pub fn intend(self, intent: I) {
        let awaited_last = self
            .awaited
            .with_value(|awaited| awaited.last().is_some_and(Option::is_some));
        let merged = self.pending.with_untracked(|pending| {
            let waiting_last = pending.len() > usize::from(self.sending.get_untracked());

            (waiting_last && !awaited_last)
                .then(|| pending.last().and_then(|last| intent.merged_after(last)))
                .flatten()
        });

        match merged {
            Some(merged) => self.pending.update(|pending| {
                if let Some(last) = pending.last_mut() {
                    *last = merged;
                }
            }),
            None => self.queue(intent, None),
        }
        self.send_next();
    }

    /// Intends as `intend` does, and answers once the server took it in, `true`, or refused it, `false`; never merged, as what waits on it has to hear of this one.
    pub fn intended(self, intent: I) -> Landed {
        let landing = Rc::new(Landing::default());
        self.queue(intent, Some(landing.clone()));
        self.send_next();

        Landed(landing)
    }

    fn queue(self, intent: I, landing: Option<Rc<Landing>>) {
        self.pending.update(|pending| pending.push(intent));
        self.awaited.update_value(|awaited| awaited.push(landing));
    }

    /// Takes in the aggregate at a version the server told, older or repeated versions changing nothing.
    pub fn take_in(self, told: A) {
        let newer = self.confirmed.with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| told.version() > held.version())
        });

        if newer {
            self.confirmed.set(Some(told));
        }
        self.send_next();
    }

    /// Asks for the aggregate whole, as when a told change cannot be taken in on what is held.
    pub fn read_again(self) {
        let Some(read) = self.read.try_get_value() else {
            return;
        };

        task::spawn_local(async move {
            match read().await {
                Ok(read) => self.take_in(read),
                Err(failure) => self.fail(failure),
            }
        });
    }

    fn send_next(self) {
        if self.sending.get_untracked() {
            return;
        }
        let Some(on) = self.confirmed.get_untracked() else {
            return;
        };
        let Some(intent) = self
            .pending
            .with_untracked(|pending| pending.first().cloned())
        else {
            return;
        };
        let (Some(send), Some(read)) = (self.send.try_get_value(), self.read.try_get_value())
        else {
            return;
        };
        self.sending.set(true);

        task::spawn_local(async move {
            let mut on = on;
            let mut tries = 0;
            let answered = loop {
                match send(on, intent.clone()).await {
                    Err(ApiError::Outdated(_)) if tries + 1 < TRIES_AGAINST_NEWER => {
                        tries += 1;
                        match read().await {
                            Ok(newer) => {
                                self.take_in_quietly(newer.clone());
                                on = newer;
                            }
                            Err(failure) => break Err(failure),
                        }
                    }
                    answered => break answered,
                }
            };

            let landed = answered.is_ok();
            match answered {
                Ok(answer) => self.take_in_quietly(answer),
                Err(ApiError::Overtaken) => {
                    self.fail(ApiError::Overtaken);
                    self.read_again();
                }
                Err(failure) => self.fail(failure),
            }
            _ = self.pending.try_update(|pending| {
                if !pending.is_empty() {
                    pending.remove(0);
                }
            });
            let landing = self
                .awaited
                .try_update_value(|awaited| (!awaited.is_empty()).then(|| awaited.remove(0)))
                .flatten()
                .flatten();
            if let Some(landing) = landing {
                landing.land(landed);
            }
            _ = self.sending.try_set(false);
            if self.sending.try_get_untracked().is_some() {
                self.send_next();
            }
        });
    }

    fn take_in_quietly(self, told: A) {
        _ = self.confirmed.try_update(|held| {
            if held
                .as_ref()
                .is_none_or(|held| told.version() > held.version())
            {
                *held = Some(told);
            }
        });
    }

    fn fail(self, failure: ApiError) {
        if let Some(failed) = self.failed.try_get_value() {
            failed(failure);
        }
    }
}

#[derive(Default)]
struct Landing {
    landed: Cell<Option<bool>>,
    waker: RefCell<Option<Waker>>,
}

impl Landing {
    fn land(&self, landed: bool) {
        self.landed.set(Some(landed));
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// Whether an intent landed, once the server answered it.
pub struct Landed(Rc<Landing>);

impl Future for Landed {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<bool> {
        match self.0.landed.get() {
            Some(landed) => Poll::Ready(landed),
            None => {
                *self.0.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}
