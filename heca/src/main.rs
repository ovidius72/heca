//! The `heca` program: everything lives in the library, so another crate can build its own
//! program on the same app and add to it before calling [`heca::run`].

fn main() {
    heca::run();
}
