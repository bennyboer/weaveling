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
    awaited: StoredValue<Vec<Option<Lander>>, LocalStorage>,
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

    /// Whether any intent of the author's is still on its way or waiting to go: what a button shows as busy, and what saving waits for.
    pub fn unanswered(self) -> bool {
        self.pending.with(|pending| !pending.is_empty())
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
        self.awaited
            .update_value(|awaited| awaited.push(landing.map(Lander)));
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
            if let Some(Lander(landing)) = &landing {
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

type SendingTo<K, A, I> = Rc<dyn Fn(K, A, I) -> Answer<A>>;
type ReadingOne<K, A> = Rc<dyn Fn(K) -> Answer<A>>;
type Copies<K, A, I> = Vec<(K, WorkingCopy<A, I>, Owner)>;

/// A list of aggregates each kept as a `WorkingCopy` of its own, the ideas of a project or the projects of an account: every item has its own version, its own intents and its own turn, so a change to one never waits for another. The list itself is what was last read, each item shown with the author's intents laid over it.
pub struct WorkingCopies<
    K: Send + Sync + 'static,
    A: Send + Sync + 'static,
    I: Send + Sync + 'static,
> {
    copies: StoredValue<Copies<K, A, I>, LocalStorage>,
    listed: RwSignal<Vec<K>>,
    has_been_read: RwSignal<bool>,
    owner: StoredValue<Owner, LocalStorage>,
    send: StoredValue<SendingTo<K, A, I>, LocalStorage>,
    read: StoredValue<ReadingOne<K, A>, LocalStorage>,
    failed: StoredValue<Failing, LocalStorage>,
}

impl<K: Send + Sync + 'static, A: Send + Sync + 'static, I: Send + Sync + 'static> Clone
    for WorkingCopies<K, A, I>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Send + Sync + 'static, A: Send + Sync + 'static, I: Send + Sync + 'static> Copy
    for WorkingCopies<K, A, I>
{
}

impl<K, A, I> WorkingCopies<K, A, I>
where
    K: Clone + PartialEq + Send + Sync + 'static,
    A: Versioned,
    I: Intent<A>,
{
    /// `send` carries out one intent on the item it names, at the version it is given; `read` asks for one item whole. Like every signal it belongs to the reactive owner current when it is made, and so do the copies it keeps.
    pub fn new<Sent, Read>(
        send: impl Fn(K, A, I) -> Sent + 'static,
        read: impl Fn(K) -> Read + 'static,
        failed: impl Fn(ApiError) + 'static,
    ) -> Self
    where
        Sent: Future<Output = Result<A, ApiError>> + 'static,
        Read: Future<Output = Result<A, ApiError>> + 'static,
    {
        let send: SendingTo<K, A, I> =
            Rc::new(move |about, on, intent| Box::pin(send(about, on, intent)));
        let read: ReadingOne<K, A> = Rc::new(move |about| Box::pin(read(about)));
        let failed: Failing = Rc::new(failed);

        Self {
            copies: StoredValue::new_local(Vec::new()),
            listed: RwSignal::new(Vec::new()),
            has_been_read: RwSignal::new(false),
            owner: StoredValue::new_local(Owner::current().unwrap_or_default()),
            send: StoredValue::new_local(send),
            read: StoredValue::new_local(read),
            failed: StoredValue::new_local(failed),
        }
    }

    /// Takes in the list as it was read: an item kept takes in its newer version, a new one gets a copy of its own, and one no longer listed is let go with whatever it still intended.
    pub fn take_in_all(self, listed: Vec<(K, A)>) {
        let keys: Vec<K> = listed.iter().map(|(about, _)| about.clone()).collect();
        let gone: Vec<Owner> = self
            .copies
            .try_update_value(|copies| {
                let mut gone = Vec::new();
                copies.retain(|(about, _, owner)| {
                    let kept = keys.contains(about);
                    if !kept {
                        gone.push(owner.clone());
                    }
                    kept
                });
                gone
            })
            .unwrap_or_default();
        for owner in gone {
            owner.cleanup();
        }
        for (about, told) in listed {
            self.copy_of(&about).take_in(told);
        }
        _ = self.listed.try_set(keys);
        _ = self.has_been_read.try_set(true);
    }

    /// Takes in one item at a version the server told, as `take_in_all` would, adding it to the end of the list when it is new.
    pub fn take_in(self, about: K, told: A) {
        self.copy_of(&about).take_in(told);
        _ = self.listed.try_update(|listed| {
            if !listed.contains(&about) {
                listed.push(about);
            }
        });
    }

    /// Lets one item go, as when it was deleted.
    pub fn forget(self, about: &K) {
        let gone = self
            .copies
            .try_update_value(|copies| {
                let at = copies.iter().position(|(held, _, _)| held == about)?;
                Some(copies.remove(at).2)
            })
            .flatten();
        if let Some(owner) = gone {
            owner.cleanup();
        }
        _ = self
            .listed
            .try_update(|listed| listed.retain(|held| held != about));
    }

    pub fn intend(self, about: &K, intent: I) {
        if let Some(copy) = self.held(about) {
            copy.intend(intent);
        }
    }

    pub fn intended(self, about: &K, intent: I) -> Option<Landed> {
        self.held(about).map(|copy| copy.intended(intent))
    }

    /// Every item listed, as the author sees it.
    pub fn shown(self) -> Vec<A> {
        self.listed
            .get()
            .iter()
            .filter_map(|about| self.held(about)?.shown())
            .collect()
    }

    pub fn shown_one(self, about: &K) -> Option<A> {
        self.listed.track();
        self.held(about)?.shown()
    }

    pub fn confirmed_one(self, about: &K) -> Option<A> {
        self.held(about)?.confirmed_untracked()
    }

    /// Whether the list was read at all yet, which tells a list still loading from one that is empty.
    pub fn is_read(self) -> bool {
        self.has_been_read.try_get().unwrap_or(false)
    }

    pub fn unanswered_on(self, about: &K) -> bool {
        self.listed.track();
        self.held(about).is_some_and(|copy| copy.unanswered())
    }

    /// Every item listed, as the server last confirmed it.
    pub fn confirmed_all(self) -> Vec<(K, A)> {
        self.listed
            .get_untracked()
            .into_iter()
            .filter_map(|about| {
                let confirmed = self.held(&about)?.confirmed_untracked()?;
                Some((about, confirmed))
            })
            .collect()
    }

    pub fn is_listed(self, about: &K) -> bool {
        self.listed.with(|listed| listed.contains(about))
    }

    fn held(self, about: &K) -> Option<WorkingCopy<A, I>> {
        self.copies
            .try_with_value(|copies| {
                copies
                    .iter()
                    .find(|(held, _, _)| held == about)
                    .map(|(_, copy, _)| *copy)
            })
            .flatten()
    }

    fn copy_of(self, about: &K) -> WorkingCopy<A, I> {
        if let Some(copy) = self.held(about) {
            return copy;
        }
        let (Some(send), Some(read), Some(failed), Some(owner)) = (
            self.send.try_get_value(),
            self.read.try_get_value(),
            self.failed.try_get_value(),
            self.owner.try_get_value(),
        ) else {
            return self.orphan();
        };
        let own = owner.child();
        let (sent_about, read_about) = (about.clone(), about.clone());
        let copy = own.with(|| {
            WorkingCopy::new(
                move |on, intent| send(sent_about.clone(), on, intent),
                move || read(read_about.clone()),
                move |failure| failed(failure),
            )
        });
        self.copies
            .update_value(|copies| copies.push((about.clone(), copy, own)));

        copy
    }

    fn orphan(self) -> WorkingCopy<A, I> {
        WorkingCopy::new(
            |_, _| async { Err(ApiError::Unexpected) },
            || async { Err(ApiError::Unexpected) },
            |_| {},
        )
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

/// A landing waited on, held by its working copy: dropped with the copy before the server answered, as when its item is let go, it lands as not landed rather than leaving whoever waits waiting for ever.
struct Lander(Rc<Landing>);

impl Drop for Lander {
    fn drop(&mut self) {
        if self.0.landed.get().is_none() {
            self.0.land(false);
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
