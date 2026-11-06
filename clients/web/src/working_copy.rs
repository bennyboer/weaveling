use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use leptos::prelude::*;
use leptos::task;
use serde::de::DeserializeOwned;

use crate::asking;
use crate::asking::Asking;
use crate::http::ApiError;
use crate::live::Subscription;
use live_contract::FrameDTO;

const TRIES_AGAINST_NEWER: u8 = 3;

type Answer<A> = Pin<Box<dyn Future<Output = Result<A, ApiError>>>>;
type Sending<A, I> = Rc<dyn Fn(A, I) -> Answer<A>>;
type Reading<A> = Rc<dyn Fn() -> Answer<A>>;
type Failing = Rc<dyn Fn(ApiError)>;
type Noticing<A> = Rc<dyn Fn(&A, &A, &mut dyn FnMut())>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadingAgain {
    Not,
    OnItsWay,
    OnceMore,
}

/// An aggregate as the server confirmed it, at the version it stands at.
pub trait Versioned: Clone + Send + Sync + 'static {
    fn version(&self) -> u64;
}

/// What the author means to do to an aggregate: shown at once, sent in its turn, and laid over the confirmed state until the server takes it in or refuses it. Not a change the server tells, which is confirmed and carries a version.
pub trait Intent<A>: Clone + Send + Sync + 'static {
    fn lay_over(&self, on: &mut A);

    /// This intent and the one waiting before it, as one, where the newer makes the older's sending needless: a card nudged twice is sent once, where it ended up.
    fn merged_after(&self, earlier: &Self) -> Option<Self>;

    /// Whether this intent still makes sense on the version it is about to be sent on, checked before each sending: one that no longer does is refused as it says, and not tried again.
    fn still_fits(&self, _on: &A) -> Result<(), ApiError> {
        Ok(())
    }
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
    reading_again: StoredValue<ReadingAgain>,
    noticing: StoredValue<Option<Noticing<A>>, LocalStorage>,
    told_meanwhile: StoredValue<bool>,
}

impl<A: Send + Sync + 'static, I: Send + Sync + 'static> Clone for WorkingCopy<A, I> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: Send + Sync + 'static, I: Send + Sync + 'static> Copy for WorkingCopy<A, I> {}

impl<A: Versioned, I: Intent<A>> WorkingCopy<A, I> {
    /// `send` carries out one intent on the version it is given, `read` asks for the aggregate whole, and `failed` hears what was refused of an intent nobody waits on, and a reading that failed.
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
            reading_again: StoredValue::new(ReadingAgain::Not),
            noticing: StoredValue::new_local(None),
            told_meanwhile: StoredValue::new(false),
        }
    }

    /// Hears every change of others taken in, told or read, with what was held before and what is held after, `taking_in` being the taking in itself for `noticed` to wrap, as keeping the cursor does; the answers to the author's own intents are never heard. As a change told back could be the author's own, one told while any of theirs is unanswered is held back, and the aggregate read once all of them are answered.
    pub fn noticing_others(self, noticed: impl Fn(&A, &A, &mut dyn FnMut()) + 'static) {
        let noticed: Noticing<A> = Rc::new(noticed);
        _ = self.noticing.try_set_value(Some(noticed));
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

    /// Intends as `intend` does, and answers once the server took it in, or with what it was refused for, after the aggregate was read again where it was overtaken; never merged, as what waits on it has to hear of this one, and a refusal is told to it alone.
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
        let Some(newer) = self.confirmed.try_with_untracked(|held| {
            held.as_ref()
                .is_none_or(|held| told.version() > held.version())
        }) else {
            return;
        };

        if newer {
            _ = self.confirmed.try_set(Some(told));
        }
        self.send_next();
    }

    /// Asks for the aggregate whole, as when a told change cannot be taken in on what is held. At most one reading is on its way: asked again meanwhile, exactly one more follows it, however often it was asked.
    pub fn read_again(self) {
        match self.reading_again.try_get_value() {
            Some(ReadingAgain::Not) => {}
            Some(_) => {
                _ = self.reading_again.try_set_value(ReadingAgain::OnceMore);
                return;
            }
            None => return,
        }
        let Some(read) = self.read.try_get_value() else {
            return;
        };
        self.reading_again.set_value(ReadingAgain::OnItsWay);

        task::spawn_local(async move {
            loop {
                match read().await {
                    Ok(read) => self.take_in_from_others(read),
                    Err(failure) => self.fail(failure),
                }
                let once_more = self
                    .reading_again
                    .try_update_value(|reading| {
                        let once_more = *reading == ReadingAgain::OnceMore;
                        *reading = match once_more {
                            true => ReadingAgain::OnItsWay,
                            false => ReadingAgain::Not,
                        };
                        once_more
                    })
                    .unwrap_or(false);
                if !once_more {
                    break;
                }
            }
        });
    }

    fn take_in_from_others(self, newer: A) {
        let held = self.confirmed.try_get_untracked().flatten();
        let noticed = self.noticing.try_get_value().flatten();
        match (held, noticed) {
            (Some(held), Some(noticed)) if newer.version() > held.version() => {
                let after = newer.clone();
                noticed(&held, &after, &mut || self.take_in(newer.clone()));
            }
            _ => self.take_in(newer),
        }
    }

    fn held_back(self) -> bool {
        let noticing = self
            .noticing
            .try_with_value(Option::is_some)
            .unwrap_or(false);
        let unanswered = self
            .pending
            .try_with_untracked(|pending| !pending.is_empty())
            .unwrap_or(false);
        if noticing && unanswered {
            _ = self.told_meanwhile.try_set_value(true);
        }

        noticing && unanswered
    }

    /// Hears a change the server told: an older or repeated version changes nothing, the next one is taken in as `taken_in` works it out on what is held, and any other, a version skipped or a change not understood, has the aggregate read again.
    pub fn heard<E: DeserializeOwned>(
        self,
        told: FrameDTO,
        taken_in: impl FnOnce(A, E) -> Option<A>,
    ) {
        let Some(Some(held)) = self.confirmed.try_get_untracked() else {
            return;
        };
        if told.version <= held.version() || self.held_back() {
            return;
        }
        match crate::working_copy::told_next(held, told, taken_in) {
            Some(next) => self.take_in_from_others(next),
            None => self.read_again(),
        }
    }

    /// Follows what the server tells of the aggregate from the moment it is first confirmed, on `topic` about what `about` names it by, each change heard as `heard` hears it.
    pub fn following<E: DeserializeOwned>(
        self,
        topic: &'static str,
        about: impl Fn(&A) -> (&'static str, String) + 'static,
        taken_in: impl Fn(A, E) -> Option<A> + Copy + 'static,
    ) {
        let following = StoredValue::new_local(None::<Subscription>);
        Effect::new(move || {
            let Some((named, id)) = self.confirmed.with(|held| held.as_ref().map(&about)) else {
                return;
            };
            if following.with_value(Option::is_some) {
                return;
            }
            following.set_value(Some(Subscription::start(
                topic,
                &[(named, id.as_str())],
                move |told| self.heard(told, taken_in),
                || {},
            )));
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
                if let Err(refusal) = intent.still_fits(&on) {
                    break Err(refusal);
                }
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

            let landed = match answered {
                Ok(answer) => {
                    self.take_in_quietly(answer);
                    Ok(())
                }
                Err(ApiError::Overtaken) => {
                    match read().await {
                        Ok(newer) => self.take_in_quietly(newer),
                        Err(failure) => self.fail(failure),
                    }
                    Err(ApiError::Overtaken)
                }
                Err(failure) => Err(failure),
            };
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
            match (&landing, landed) {
                (Some(Lander(landing)), landed) => landing.land(landed),
                (None, Err(failure)) => self.fail(failure),
                (None, Ok(())) => {}
            }
            _ = self.sending.try_set(false);
            if self.sending.try_get_untracked().is_some() {
                self.send_next();
                self.read_what_was_held_back();
            }
        });
    }

    fn read_what_was_held_back(self) {
        let answered = self
            .pending
            .try_with_untracked(Vec::is_empty)
            .unwrap_or(false);
        if answered
            && self
                .told_meanwhile
                .try_update_value(std::mem::take)
                .unwrap_or(false)
        {
            self.read_again();
        }
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

/// What a told change makes of what is held, when it is the very next version and understood; `None` when it cannot be taken in on what is held, and the aggregate has to be read again.
pub fn told_next<A: Versioned, E: DeserializeOwned>(
    held: A,
    told: FrameDTO,
    taken_in: impl FnOnce(A, E) -> Option<A>,
) -> Option<A> {
    (told.version == held.version() + 1)
        .then(|| serde_json::from_value::<E>(told.event).ok())
        .flatten()
        .and_then(|event| taken_in(held, event))
}

/// What a told change says the list must show before a reading of it is taken in: an item at that version or newer, or an item gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Awaited<K> {
    Held { id: K, at_least: u64 },
    Gone(K),
}

impl<K: PartialEq> Awaited<K> {
    pub fn holds<A: Versioned>(&self, listed: &[(K, A)]) -> bool {
        match self {
            Self::Held { id, at_least } => listed
                .iter()
                .any(|(held, item)| held == id && item.version() >= *at_least),
            Self::Gone(id) => listed.iter().all(|(held, _)| held != id),
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
    asking: Asking,
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
            asking: Asking::new(),
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

    /// Reads the list whole and takes it in as `take_in_all` does, answering with the failure if it could not be read. Read again before the answer came, only the newest reading is taken in; and a reading that does not show yet what was `awaited` is asked again a few times first, as a catalog may lag a moment behind what was told.
    pub fn read_all<Read>(
        self,
        read: impl Fn() -> Read + 'static,
        awaited: Option<Awaited<K>>,
    ) -> impl Future<Output = Result<(), ApiError>> + 'static
    where
        Read: Future<Output = Result<Vec<(K, A)>, ApiError>> + 'static,
    {
        let asked = self.asking.next();

        async move {
            let answer = asking::until_it_holds(
                read,
                |answer: &Result<Vec<(K, A)>, ApiError>| match (answer, &awaited) {
                    (Ok(listed), Some(awaited)) => awaited.holds(listed),
                    _ => true,
                },
                asking::paused,
            )
            .await;
            if self.asking.is_latest(asked) {
                self.take_in_all(answer?);
            }

            Ok(())
        }
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

    /// Reads one item whole and takes it in, holding it from now on; let go before the answer came, it is not taken back, nor is its failure told. Reads crossing need no ordering: the item takes in only what is newer.
    pub fn read_again(
        self,
        about: &K,
    ) -> impl Future<Output = Result<(), ApiError>> + use<K, A, I> {
        self.copy_of(about);
        let read = self.read.try_get_value();
        let about = about.clone();

        async move {
            let found = read.ok_or(ApiError::Unexpected)?(about.clone()).await;
            if self.held(&about).is_some() {
                self.take_in(about, found?);
            }

            Ok(())
        }
    }

    /// Tells a failure as a refusal of an intent nobody waits on is told, as when the list could not be read.
    pub fn fail(self, failure: ApiError) {
        if let Some(failed) = self.failed.try_get_value() {
            failed(failure);
        }
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

    /// Intends as the item's own `intended` does; an item not held refuses it at once.
    pub fn intended(self, about: &K, intent: I) -> Landed {
        match self.held(about) {
            Some(copy) => copy.intended(intent),
            None => Landed::refused(ApiError::Unexpected),
        }
    }

    /// Intends what ends an item, a withdrawal, a dismissal, a trashing, and lets the item go once it landed, answering whether it did; ignored while the item is not listed or a change to it is still unanswered, so a second press never sends it twice. Refused, it is told as an intent nobody waits on is, unless the item was let go meanwhile.
    pub fn ending(self, about: &K, intent: I) -> impl Future<Output = bool> + use<K, A, I> {
        let landing = untrack(|| self.is_listed(about) && !self.unanswered_on(about))
            .then(|| self.intended(about, intent));
        let failed = self.failed.try_get_value();
        let about = about.clone();

        async move {
            let Some(landing) = landing else {
                return false;
            };
            match landing.await {
                Ok(()) => {
                    self.forget(&about);
                    true
                }
                Err(failure) => {
                    if let Some(failed) = failed.filter(|_| self.held(&about).is_some()) {
                        failed(failure);
                    }
                    false
                }
            }
        }
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
    landed: RefCell<Option<Result<(), ApiError>>>,
    waker: RefCell<Option<Waker>>,
}

impl Landing {
    fn land(&self, landed: Result<(), ApiError>) {
        *self.landed.borrow_mut() = Some(landed);
        if let Some(waker) = self.waker.borrow_mut().take() {
            waker.wake();
        }
    }
}

/// A landing waited on, held by its working copy: dropped with the copy before the server answered, as when its item is let go, it lands refused rather than leaving whoever waits waiting for ever.
struct Lander(Rc<Landing>);

impl Drop for Lander {
    fn drop(&mut self) {
        if self.0.landed.borrow().is_none() {
            self.0.land(Err(ApiError::Unexpected));
        }
    }
}

/// Whether an intent landed, once the server answered it, or what it was refused for.
pub struct Landed(Rc<Landing>);

impl Landed {
    fn refused(failure: ApiError) -> Self {
        let landing = Landing::default();
        landing.land(Err(failure));

        Self(Rc::new(landing))
    }
}

impl Future for Landed {
    type Output = Result<(), ApiError>;

    fn poll(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Self::Output> {
        let landed = self.0.landed.borrow().clone();
        match landed {
            Some(landed) => Poll::Ready(landed),
            None => {
                *self.0.waker.borrow_mut() = Some(context.waker().clone());
                Poll::Pending
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Count {
        version: u64,
        counted: u64,
    }

    impl Versioned for Count {
        fn version(&self) -> u64 {
            self.version
        }
    }

    fn a_count_at(version: u64) -> Count {
        Count {
            version,
            counted: 10,
        }
    }

    fn told(version: u64, event: Value) -> FrameDTO {
        FrameDTO {
            of: None,
            version,
            at: None,
            event,
        }
    }

    fn added(held: Count, by: u64) -> Option<Count> {
        Some(Count {
            version: held.version + 1,
            counted: held.counted + by,
        })
    }

    #[test]
    fn the_next_version_told_is_taken_in_on_what_is_held() {
        // when: the version after the one held is told
        let next = crate::working_copy::told_next(a_count_at(3), told(4, json!(5)), added);

        // then: it is what the change makes of what is held
        assert_eq!(
            next,
            Some(Count {
                version: 4,
                counted: 15
            })
        );
    }

    #[test]
    fn a_version_skipped_or_a_change_not_understood_is_not_taken_in() {
        // expect: a change two versions on is not taken in
        assert_eq!(
            crate::working_copy::told_next(a_count_at(3), told(5, json!(5)), added),
            None
        );
        // and: nor is one that does not read as a change
        assert_eq!(
            crate::working_copy::told_next(a_count_at(3), told(4, json!("five")), added),
            None
        );
    }
}
