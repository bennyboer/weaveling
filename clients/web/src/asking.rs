use std::cell::{Cell, RefCell};
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;

use leptos::prelude::*;

pub const PAUSES: [Duration; 4] = [
    Duration::from_millis(100),
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

pub async fn until_it_holds<T, Asked, Paused>(
    ask: impl Fn() -> Asked,
    holds: impl Fn(&T) -> bool,
    pause: impl Fn(Duration) -> Paused,
) -> T
where
    Asked: Future<Output = T>,
    Paused: Future<Output = ()>,
{
    let mut answer = ask().await;
    for wait in PAUSES {
        if holds(&answer) {
            return answer;
        }
        pause(wait).await;
        answer = ask().await;
    }

    answer
}

pub fn paused(wait: Duration) -> Pause {
    Pause {
        wait,
        over: Rc::new(Cell::new(false)),
        waker: Rc::new(RefCell::new(None)),
        started: false,
    }
}

pub struct Pause {
    wait: Duration,
    over: Rc<Cell<bool>>,
    waker: Rc<RefCell<Option<Waker>>>,
    started: bool,
}

impl Future for Pause {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.over.get() {
            return Poll::Ready(());
        }
        *self.waker.borrow_mut() = Some(context.waker().clone());
        if !self.started {
            self.started = true;
            let (over, waker) = (self.over.clone(), self.waker.clone());
            _ = set_timeout(
                move || {
                    over.set(true);
                    if let Some(waker) = waker.borrow_mut().take() {
                        waker.wake();
                    }
                },
                self.wait,
            );
        }

        Poll::Pending
    }
}

/// Asking again for something that changes, where the newest asking is the one whose answer counts: each asking takes the next number, and an answer is taken in only while no newer asking was made, so one read before a change and answered after it never stands over what came later. Acting once on a press is an `Act` instead.
///
/// Like every signal it belongs to the reactive owner current when it is made: one made inside an `Effect` is disposed when that effect runs again, and from then on no answer counts as the latest, so make it where whatever asks lives.
#[derive(Clone, Copy)]
pub struct Asking(StoredValue<u64>);

impl Asking {
    pub fn new() -> Self {
        Self(StoredValue::new(0))
    }

    pub fn next(self) -> u64 {
        let asked = self.0.get_value() + 1;
        self.0.set_value(asked);

        asked
    }

    pub fn is_latest(self, asked: u64) -> bool {
        self.0.try_get_value() == Some(asked)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    use super::*;

    fn at_once<T>(asking: impl Future<Output = T>) -> T {
        let mut asking = pin!(asking);
        let mut context = Context::from_waker(Waker::noop());
        match asking.as_mut().poll(&mut context) {
            Poll::Ready(answer) => answer,
            Poll::Pending => panic!("nothing here waits on anything but itself"),
        }
    }

    struct Asked {
        answers: RefCell<Vec<u32>>,
        times: Cell<u32>,
        paused: RefCell<Vec<Duration>>,
    }

    impl Asked {
        fn answering(answers: &[u32]) -> Self {
            Self {
                answers: RefCell::new(answers.iter().rev().copied().collect()),
                times: Cell::new(0),
                paused: RefCell::new(Vec::new()),
            }
        }

        fn until(&self, holds: u32) -> u32 {
            at_once(until_it_holds(
                || {
                    self.times.set(self.times.get() + 1);
                    let next = self.answers.borrow_mut().pop().unwrap_or(0);
                    future::ready(next)
                },
                |answer| *answer >= holds,
                |wait| {
                    self.paused.borrow_mut().push(wait);
                    future::ready(())
                },
            ))
        }
    }

    #[test]
    fn a_first_answer_that_holds_the_change_ends_the_asking_at_once() {
        // given: a first answer that holds the change
        let asked = Asked::answering(&[3]);

        // then: it is given after one asking and no pause
        assert_eq!(asked.until(3), 3);
        assert_eq!(asked.times.get(), 1);
        assert!(
            asked.paused.borrow().is_empty(),
            "the fast path costs nothing"
        );
    }

    #[test]
    fn an_answer_behind_the_change_is_asked_for_again_after_growing_pauses_until_it_holds() {
        // given: answers that reach the change on the third asking
        let asked = Asked::answering(&[1, 2, 3, 9]);

        // then: the third answer is given after the first two pauses
        assert_eq!(asked.until(3), 3);
        assert_eq!(asked.times.get(), 3);
        assert_eq!(*asked.paused.borrow(), PAUSES[..2]);
    }

    #[test]
    fn after_the_last_pause_the_last_answer_is_given_up_with() {
        // given: answers that reach the change only after the last pause
        let asked = Asked::answering(&[1, 1, 1, 1, 2, 9]);

        // then: the answer after the last pause is given, every pause taken
        assert_eq!(asked.until(9), 2, "the last answer, whatever it holds");
        assert_eq!(asked.times.get(), PAUSES.len() as u32 + 1);
        assert_eq!(*asked.paused.borrow(), PAUSES);
    }
}
