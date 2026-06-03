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
    create_effect, create_memo, Memo, RwSignal, SignalGet, SignalUpdate, SignalWith,
};

/// A read-write reactive value. Cheap to copy and move into closures.
pub type Signal<T> = RwSignal<T>;

/// Create a new reactive signal holding `value`.
pub fn signal<T: 'static>(value: T) -> Signal<T> {
    RwSignal::new(value)
}
