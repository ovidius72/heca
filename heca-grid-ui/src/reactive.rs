//! Reactive facade.
//!
//! Fine-grained reactivity (signals, memos, effects) is provided by
//! [`floem_reactive`], but **component code must never name that crate
//! directly** — it goes through this facade. The whole reactive engine can then
//! be swapped (e.g. for a custom runtime) without touching a single widget,
//! because the public surface here stays stable.
//!
//! ## Usage
//! ```ignore
//! use heca_grid_ui::reactive::{signal, SignalGet, SignalUpdate};
//!
//! let count = signal(0);
//! count.set(count.get() + 1);
//! ```
//!
//! Reading a signal (`.get()`) inside a component's `paint`/`layout` subscribes
//! that component; writing (`.set()` / `.update()`) marks subscribers dirty,
//! which the app coalesces into a single redraw.

pub use floem_reactive::{
    Memo, RwSignal, SignalGet, SignalWith, batch, create_effect, create_memo, untrack,
};

/// **Writing a signal** — and a write that changes nothing wakes nobody.
///
/// [`set`](Self::set) compares first: writing the value a signal already holds is not a change, so
/// nothing that listens to it runs. Without this, two parts that answer each other — a surface that
/// writes `open = false` when closed, and a host that closes the surface when `open` turns false —
/// pass the same message back and forth for ever; the compiler cannot see it, because every step is
/// correct. With it, the second write is a no-op and the exchange ends. It is the default of
/// SolidJS, Svelte and Leptos for the same reason.
///
/// [`update`](Self::update) changes the value in place and always tells the listeners — for a value
/// that cannot be compared, or a change made inside it.
pub trait SignalUpdate<T> {
    /// Write `value`; nobody is told unless it differs from what the signal holds.
    fn set(&self, value: T)
    where
        T: PartialEq + 'static;
    /// Change the value in place, and tell the listeners.
    fn update(&self, f: impl FnOnce(&mut T))
    where
        T: 'static;
    /// Change the value in place, tell the listeners, and return what `f` returned.
    fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
    where
        T: 'static;
}

impl<T> SignalUpdate<T> for RwSignal<T> {
    fn set(&self, value: T)
    where
        T: PartialEq + 'static,
    {
        if self.with_untracked(|now| *now != value) {
            floem_reactive::SignalUpdate::set(self, value);
        }
    }
    fn update(&self, f: impl FnOnce(&mut T))
    where
        T: 'static,
    {
        floem_reactive::SignalUpdate::update(self, f);
    }
    fn try_update<O>(&self, f: impl FnOnce(&mut T) -> O) -> Option<O>
    where
        T: 'static,
    {
        floem_reactive::SignalUpdate::try_update(self, f)
    }
}

/// A read-write reactive value. Cheap to copy and move into closures.
pub type Signal<T> = RwSignal<T>;

/// Create a new reactive signal holding `value`.
pub fn signal<T: 'static>(value: T) -> Signal<T> {
    RwSignal::new(value)
}
