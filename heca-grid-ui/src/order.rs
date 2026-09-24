//! [`Order`] — **CSS `order`**: where a child sits among its siblings, said by the child.
//!
//! ```ignore
//! Flex::column()
//!     .child(notes)                  // unset — order 0, in the order it was added
//!     .child(docker.order(-1))       // before everything that said nothing
//!     .child(git.order([0, 5]))      // after every plain 0, before any 1
//! ```
//!
//! **Lower comes first. Ties keep the order the children were added in**, so a parent where nobody
//! says anything lays out exactly as it did before this existed.
//!
//! **A number or a list, through one parser.** A list is compared a place at a time, the way a
//! version number is: `[0, 5]` is after `0` and before `1`. That is what lets a plugin slot between
//! two children it does not own — built-ins at `0` and `1` leave no whole number between them, and
//! renumbering someone else's children is not an option. A missing place counts as `0`, so `1` and
//! `[1, 0]` are the same order.
//!
//! It changes **where a child is laid out**, not where it sits in the tree. Paint order, the Tab
//! order and the order the letter picker hands out letters stay the order the children were added
//! in — exactly CSS's rule, where `order` is visual only.
//!
//! In a [`Grid`](crate::widgets::Grid) it orders the children the grid places **itself**; a child
//! that said where it goes (`.row`, `.column`, `.area`) is already placed, and `order` does not move
//! it. Also CSS.

use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How many places a list may have. Four is far more than any real case needs — one level
/// separates built-ins, a second slots a plugin between two of them — and a fixed size keeps
/// [`Layout`](crate::style::Layout) `Copy`.
pub const MAX_PLACES: usize = 4;

/// **CSS `order`** — see the [module docs](self). `Order::default()` is `0`, which is what a child
/// that says nothing has.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Order([i64; MAX_PLACES]);

impl Order {
    /// The places, trailing zeros dropped — `[0, 5]` for `[0, 5]`, `[3]` for `3`, `[0]` for `0`.
    pub fn places(&self) -> &[i64] {
        let used = self.0.iter().rposition(|&p| p != 0).map_or(1, |i| i + 1);
        &self.0[..used]
    }
}

impl Ord for Order {
    fn cmp(&self, other: &Self) -> Ordering {
        // Missing places are zeros already, so the arrays compare place by place.
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Order {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Why a spelling is not an order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderError {
    /// A place that is not a whole number.
    NotANumber(String),
    /// More than [`MAX_PLACES`] places.
    TooManyPlaces(usize),
    /// Nothing at all.
    Empty,
}

impl fmt::Display for OrderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderError::NotANumber(p) => write!(f, "'{p}' is not a whole number"),
            OrderError::TooManyPlaces(n) => {
                write!(f, "an order has at most {MAX_PLACES} places, this has {n}")
            }
            OrderError::Empty => f.write_str("an order needs at least one number"),
        }
    }
}

impl std::error::Error for OrderError {}

impl Order {
    /// From a list of places. The one place a list becomes an order — every `From` and the parser
    /// come through here.
    pub fn from_places(places: &[i64]) -> Result<Self, OrderError> {
        if places.is_empty() {
            return Err(OrderError::Empty);
        }
        if places.len() > MAX_PLACES {
            return Err(OrderError::TooManyPlaces(places.len()));
        }
        let mut out = [0; MAX_PLACES];
        out[..places.len()].copy_from_slice(places);
        Ok(Order(out))
    }
}

/// **The one parser.** `"3"`, `"-1"`, `"0 5"`, `"0, 5"` and `"[0, 5]"` — so a call site, a plugin's
/// description and a config file can never disagree about what an order means.
impl FromStr for Order {
    type Err = OrderError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let inner = s.trim().trim_start_matches('[').trim_end_matches(']');
        let places = inner
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .map(|p| {
                p.parse::<i64>()
                    .map_err(|_| OrderError::NotANumber(p.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Order::from_places(&places)
    }
}

impl From<i32> for Order {
    fn from(v: i32) -> Self {
        Order::from(v as i64)
    }
}

impl From<i64> for Order {
    fn from(v: i64) -> Self {
        let mut out = [0; MAX_PLACES];
        out[0] = v;
        Order(out)
    }
}

/// `.order([0, 5])`. More than [`MAX_PLACES`] places is a programming error in code you wrote, so
/// it panics here; a description with too many is refused by the parser instead.
impl<const N: usize> From<[i32; N]> for Order {
    fn from(places: [i32; N]) -> Self {
        let places: Vec<i64> = places.iter().map(|&p| p as i64).collect();
        Order::from_places(&places).expect("an order has at most four places")
    }
}

impl<const N: usize> From<[i64; N]> for Order {
    fn from(places: [i64; N]) -> Self {
        Order::from_places(&places).expect("an order has at most four places")
    }
}

/// A spelling nobody can read is order `0` rather than a panic — the rule `Length` and `Space`
/// follow, because these arrive from plugins and config. Parse it yourself to be told instead.
impl From<&str> for Order {
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_default()
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let places = self.places();
        if let [one] = places {
            return write!(f, "{one}");
        }
        let parts: Vec<String> = places.iter().map(i64::to_string).collect();
        write!(f, "[{}]", parts.join(", "))
    }
}

/// On the wire: a number when it is one place (`3`), a list otherwise (`[0, 5]`).
impl Serialize for Order {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self.places() {
            [one] => s.serialize_i64(*one),
            places => places.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for Order {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Repr {
            One(i64),
            List(Vec<i64>),
            Text(String),
        }
        match Repr::deserialize(d)? {
            Repr::One(v) => Ok(Order::from(v)),
            Repr::List(places) => Order::from_places(&places).map_err(D::Error::custom),
            // One parser, not a second copy.
            Repr::Text(t) => t.parse().map_err(D::Error::custom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_comes_first_and_a_list_slots_between_whole_numbers() {
        let zero = Order::from(0);
        let between = Order::from([0, 5]);
        let one = Order::from(1);
        let before = Order::from(-1);
        assert!(before < zero);
        assert!(zero < between);
        assert!(between < one);
        assert_eq!(Order::from(1), Order::from([1, 0]), "a missing place is 0");
        assert_eq!(Order::default(), zero, "saying nothing is 0");
    }

    #[test]
    fn every_spelling_reads_through_one_parser() {
        assert_eq!("3".parse::<Order>(), Ok(Order::from(3)));
        assert_eq!("-1".parse::<Order>(), Ok(Order::from(-1)));
        for s in ["0 5", "0, 5", "[0, 5]", "[0,5]"] {
            assert_eq!(s.parse::<Order>(), Ok(Order::from([0, 5])), "{s}");
        }
        assert_eq!(
            "x".parse::<Order>(),
            Err(OrderError::NotANumber("x".into()))
        );
        assert_eq!("".parse::<Order>(), Err(OrderError::Empty));
        assert_eq!(
            "1 2 3 4 5".parse::<Order>(),
            Err(OrderError::TooManyPlaces(5))
        );
        assert_eq!(Order::from("nonsense"), Order::default(), "unreadable is 0");
    }

    #[test]
    fn the_wire_takes_a_number_a_list_or_a_string_through_the_same_parser() {
        use serde::de::IntoDeserializer;
        use serde::de::value::{Error, I64Deserializer, SeqDeserializer, StrDeserializer};

        let n: I64Deserializer<Error> = 3i64.into_deserializer();
        assert_eq!(Order::deserialize(n), Ok(Order::from(3)));

        let l: SeqDeserializer<_, Error> = vec![0i64, 5].into_deserializer();
        assert_eq!(Order::deserialize(l), Ok(Order::from([0, 5])));

        let t: StrDeserializer<Error> = "0 5".into_deserializer();
        assert_eq!(Order::deserialize(t), Ok(Order::from([0, 5])));

        let too_many: SeqDeserializer<_, Error> = vec![1i64, 2, 3, 4, 5].into_deserializer();
        assert!(Order::deserialize(too_many).is_err());

        assert_eq!(Order::from([0, 5]).to_string(), "[0, 5]");
        assert_eq!(Order::from(3).to_string(), "3");
    }
}
