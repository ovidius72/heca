//! **Things declared before the app starts** — held until the host takes them, refused after.
//!
//! A program built on heca declares what it adds — an action, a layer, a pane button, a dock's
//! placement — in `main`, before `heca::run()`. The host takes each kind once, as it starts. What is
//! declared after that can no longer reach it, so it is **refused out loud** instead of being
//! accepted and silently doing nothing.
//!
//! That rule — queue, take once, refuse afterwards, start fresh for a new host — was written six
//! times, once per kind. It is written here once; a kind supplies the thing queued and the words of
//! its own refusal.
//!
//! ```ignore
//! thread_local! { static QUEUE: StartupQueue<Declared> = const { StartupQueue::new() }; }
//!
//! if let Err(late) = QUEUE.with(|q| q.add(declared)) {
//!     warn_author(format!("'{}' was added after the app started", late.name));
//! }
//! let everything = QUEUE.with(StartupQueue::take); // the host, once
//! ```
//!
//! Thread-local at the use site, not a global with a lock: the app is built and driven on the UI
//! thread only.

use std::cell::{Cell, RefCell};

/// What was declared before the host started, and whether it has.
pub(crate) struct StartupQueue<T> {
    items: RefCell<Vec<T>>,
    taken: Cell<bool>,
}

impl<T> StartupQueue<T> {
    /// An open, empty queue.
    pub(crate) const fn new() -> Self {
        Self {
            items: RefCell::new(Vec::new()),
            taken: Cell::new(false),
        }
    }

    /// Queue `item`. Once the host has taken the queue the item is handed **back**, so the caller
    /// can say what was refused in its own words.
    pub(crate) fn add(&self, item: T) -> Result<(), T> {
        if self.taken.get() {
            return Err(item);
        }
        self.items.borrow_mut().push(item);
        Ok(())
    }

    /// Everything queued, in the order it was declared — and the queue is closed. The host calls
    /// this once, as it starts.
    pub(crate) fn take(&self) -> Vec<T> {
        self.taken.set(true);
        std::mem::take(&mut *self.items.borrow_mut())
    }

    /// A new host is starting: accept again. What was declared before it started is kept — that is
    /// what the new host is about to take.
    pub(crate) fn reopen(&self) {
        self.taken.set(false);
    }

    /// Open and empty — for a test that wants the queue as a fresh process would have it.
    #[cfg(test)]
    pub(crate) fn reset(&self) {
        self.taken.set(false);
        self.items.borrow_mut().clear();
    }

    /// What is queued right now, without taking it.
    #[cfg(test)]
    pub(crate) fn peek<R>(&self, read: impl FnOnce(&[T]) -> R) -> R {
        read(&self.items.borrow())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Queued until taken, in order; refused — and handed back — after.**
    #[test]
    fn a_declaration_is_kept_until_the_host_takes_it_and_refused_after() {
        let queue = StartupQueue::new();
        assert_eq!(queue.add("first"), Ok(()));
        assert_eq!(queue.add("second"), Ok(()));
        assert_eq!(queue.take(), ["first", "second"]);
        assert_eq!(
            queue.add("late"),
            Err("late"),
            "the refused item comes back"
        );
        assert!(queue.peek(<[_]>::is_empty), "and was not kept");
    }

    /// A new host accepts again and keeps what was declared before it started.
    #[test]
    fn a_new_host_accepts_again_and_keeps_what_was_declared_before_it() {
        let queue = StartupQueue::new();
        let _ = queue.add(1);
        queue.reopen();
        assert_eq!(queue.add(2), Ok(()));
        assert_eq!(queue.take(), [1, 2]);
        queue.reset();
        assert_eq!(queue.add(3), Ok(()), "a reset queue is open");
        assert_eq!(queue.peek(|items| items.to_vec()), [3]);
    }
}
