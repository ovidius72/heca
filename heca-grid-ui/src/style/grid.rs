//! **Grid tracks and cells** — what a [`Grid`](crate::widgets::Grid) is divided into and where a
//! child sits in it.

use super::*;

/// One column/row track size for a [`Grid`](crate::widgets::Grid).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Track {
    /// Fixed logical pixels.
    Px(f32),
    /// A fraction of the leftover free space (`1fr`, `2fr`, …).
    Fr(f32),
    /// Sized to fit content / grid rules.
    Auto,
    /// Shrink to the minimum the content allows.
    MinContent,
    /// Grow to the maximum the content wants.
    MaxContent,
}

/// **A track list, in whichever shape the caller holds it** — one stylesheet line (`"auto 1fr"`),
/// or a list of anything a [`Track`] reads (`["auto", "1fr"]`, `[Track::Auto, Track::Fr(1.0)]`,
/// `[200, 100]`).
///
/// One builder per axis takes this, so there is no `rows` / `template_rows` pair to choose between.
pub trait IntoTracks {
    /// The tracks, parsed.
    fn into_tracks(self) -> Vec<Track>;
}

impl IntoTracks for &str {
    /// Whitespace-separated, exactly as CSS writes `grid-template-rows`.
    fn into_tracks(self) -> Vec<Track> {
        Track::list(self)
    }
}

impl IntoTracks for &String {
    fn into_tracks(self) -> Vec<Track> {
        Track::list(self)
    }
}

impl<T: Into<Track>> IntoTracks for Vec<T> {
    fn into_tracks(self) -> Vec<Track> {
        self.into_iter().map(Into::into).collect()
    }
}

impl<T: Into<Track>, const N: usize> IntoTracks for [T; N] {
    fn into_tracks(self) -> Vec<Track> {
        self.into_iter().map(Into::into).collect()
    }
}

/// **One template row or a list of them** — what [`Grid::template_area`](crate::widgets::Grid::template_area)
/// takes, so a single-row template needs no brackets.
pub trait IntoRows {
    /// The rows, as strings.
    fn into_rows(self) -> Vec<String>;
}

impl IntoRows for &str {
    fn into_rows(self) -> Vec<String> {
        vec![self.to_string()]
    }
}

impl IntoRows for &String {
    fn into_rows(self) -> Vec<String> {
        vec![self.to_string()]
    }
}

impl<T: AsRef<str>> IntoRows for Vec<T> {
    fn into_rows(self) -> Vec<String> {
        self.iter().map(|r| r.as_ref().to_string()).collect()
    }
}

impl<T: AsRef<str>, const N: usize> IntoRows for [T; N] {
    fn into_rows(self) -> Vec<String> {
        self.iter().map(|r| r.as_ref().to_string()).collect()
    }
}

impl std::str::FromStr for Track {
    type Err = TrackParseError;

    /// **The one parser**, and the vocabulary CSS already has: `"auto"`, `"1fr"`, `"22px"`, `"22"`,
    /// `"min-content"` / `"min"`, `"max-content"` / `"max"`.
    ///
    /// [`Deserialize`] calls this, so a described grid and a native one can never come to disagree
    /// about what `"1fr"` means, and Rust code can write `"1fr"` just as a plugin's JSON does.
    fn from_str(t: &str) -> Result<Self, Self::Err> {
        let t = t.trim();
        match t.to_ascii_lowercase().as_str() {
            "auto" => return Ok(Track::Auto),
            // Hyphen and underscore both, because CSS writes one and serde's own names write the
            // other, and a reader should not have to know which side of the wire they are on.
            "min" | "min-content" | "min_content" => return Ok(Track::MinContent),
            "max" | "max-content" | "max_content" => return Ok(Track::MaxContent),
            _ => {}
        }
        let low = t.to_ascii_lowercase();
        if let Some(fr) = low.strip_suffix("fr") {
            return fr
                .trim()
                .parse()
                .map(Track::Fr)
                .map_err(|_| TrackParseError);
        }
        // `px` is optional and means the same as no suffix — the rule `Length` and `Space` follow.
        let px = low.strip_suffix("px").map_or(low.as_str(), str::trim_end);
        px.parse::<f32>()
            .map(Track::Px)
            .map_err(|_| TrackParseError)
    }
}

/// What [`Track`]'s `from_str` returns when a string is no track size at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackParseError;

impl From<&str> for Track {
    /// `.rows(["auto", "1fr"])`.
    ///
    /// ⚠️ Anything unreadable is [`Auto`](Track::Auto) rather than a panic — the rule [`Length`]
    /// and [`Space`] follow, because these arrive from config and from plugins, so a typo costs
    /// its author a differently-sized track and not the host. Use
    /// [`from_str`](std::str::FromStr::from_str) when you want to be told.
    fn from(t: &str) -> Self {
        t.parse().unwrap_or(Track::Auto)
    }
}

impl From<&String> for Track {
    fn from(t: &String) -> Self {
        Track::from(t.as_str())
    }
}

impl From<f32> for Track {
    /// A bare number is pixels, the same forgiving reading every other size takes.
    fn from(v: f32) -> Self {
        Track::Px(v)
    }
}

impl From<i32> for Track {
    fn from(v: i32) -> Self {
        Track::Px(v as f32)
    }
}

impl Serialize for Track {
    /// Every track travels as the string a stylesheet would write, except a plain pixel count,
    /// which travels as the number it is.
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match *self {
            Track::Px(v) => s.serialize_f32(v),
            Track::Fr(v) => s.serialize_str(&format!("{v}fr")),
            Track::Auto => s.serialize_str("auto"),
            Track::MinContent => s.serialize_str("min-content"),
            Track::MaxContent => s.serialize_str("max-content"),
        }
    }
}

impl<'de> Deserialize<'de> for Track {
    /// A string through [`from_str`](std::str::FromStr::from_str), or a bare number as pixels.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Track;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a track size: \"auto\", \"1fr\", \"22px\", a number, \"min-content\" or \"max-content\"")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Track, E> {
                v.parse()
                    .map_err(|_| E::custom(format!("not a track size: {v}")))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Track, E> {
                Ok(Track::Px(v as f32))
            }
        }
        d.deserialize_any(V)
    }
}

impl Track {
    /// **Read a whole track list the way a stylesheet writes one**: `"auto 1fr"`,
    /// `"200px 1fr 2fr"`, `"repeat(3, 1fr)"`, `"auto repeat(2, 1fr) auto"`.
    ///
    /// Whitespace-separated, each item through the one parser, and `repeat(n, tracks)` expanded as
    /// CSS expands it — the tracks inside repeated `n` times. Nesting a `repeat` inside a `repeat`
    /// is not a thing in CSS and is not one here.
    pub fn list(template: &str) -> Vec<Track> {
        let mut out = Vec::new();
        let mut rest = template;
        while let Some(at) = rest.to_ascii_lowercase().find("repeat(") {
            out.extend(rest[..at].split_whitespace().map(Track::from));
            let after = &rest[at + "repeat(".len()..];
            let Some(close) = after.find(')') else {
                // Unclosed: read what is there as ordinary tracks rather than dropping it.
                out.extend(after.split_whitespace().map(Track::from));
                return out;
            };
            let (inside, tail) = after.split_at(close);
            if let Some((count, tracks)) = inside.split_once(',') {
                let times = count.trim().parse::<usize>().unwrap_or(1).min(512);
                let unit: Vec<Track> = tracks.split_whitespace().map(Track::from).collect();
                for _ in 0..times {
                    out.extend(unit.iter().copied());
                }
            }
            rest = &tail[1..];
        }
        out.extend(rest.split_whitespace().map(Track::from));
        out
    }

    pub(super) fn to_taffy(self) -> taffy::style::TrackSizingFunction {
        use taffy::prelude::*;
        match self {
            Track::Px(v) => length(v),
            // **A fraction is `minmax(0, <n>fr)`, which is what an author means by it.**
            //
            // A bare `fr` track carries an automatic minimum of its own CONTENT, so a track holding
            // something tall grows past its share and pushes its neighbours out of the box — two
            // docks in one sidebar, and the second one off the bottom. The floor has to go for the
            // fraction to be a fraction, and it goes here so no grid, and no caller, meets it.
            Track::Fr(v) => minmax(length(0.0), fr(v)),
            Track::Auto => auto(),
            Track::MinContent => min_content(),
            Track::MaxContent => max_content(),
        }
    }
}

/// Placement of a child within a [`Grid`](crate::widgets::Grid): a 1-based start
/// column/row plus a span. `Copy`, so it lives on [`Style`] without breaking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GridCell {
    /// 1-based start column.
    pub col: u16,
    /// 1-based start row.
    pub row: u16,
    /// Number of columns spanned (≥ 1), or [`GridCell::ALL`] for every column there are.
    pub col_span: u16,
    /// Number of rows spanned (≥ 1), or [`GridCell::ALL`] for every row there are.
    pub row_span: u16,
}

/// **A grid's template**, borrowed from the widget that owns it — what a child's placement is
/// resolved against during layout.
///
/// See [`Component::grid_template`](crate::component::Component::grid_template).
#[derive(Debug, Clone, Copy)]
pub struct GridTemplate<'a> {
    /// The column tracks.
    pub columns: &'a [Track],
    /// The row tracks.
    pub rows: &'a [Track],
    /// The named areas, one string per row, exactly as written.
    pub areas: &'a [String],
}

impl GridTemplate<'_> {
    /// **Where a named area sits** — the bounding box of every cell carrying that name. `.` and `_`
    /// mark an empty cell and name nothing.
    pub fn area(&self, name: &str) -> Option<GridCell> {
        let mut found: Option<(u16, u16, u16, u16)> = None; // min_c, max_c, min_r, max_r
        for (r, line) in self.areas.iter().enumerate() {
            for (c, token) in line.split_whitespace().enumerate() {
                if token != name {
                    continue;
                }
                let (col, row) = (c as u16 + 1, r as u16 + 1);
                found = Some(match found {
                    None => (col, col, row, row),
                    Some((c0, c1, r0, r1)) => (c0.min(col), c1.max(col), r0.min(row), r1.max(row)),
                });
            }
        }
        found.map(|(c0, c1, r0, r1)| GridCell {
            col: c0,
            row: r0,
            col_span: c1 - c0 + 1,
            row_span: r1 - r0 + 1,
        })
    }

    /// How many columns and rows there are — what [`GridCell::ALL`] resolves to.
    pub fn counts(&self) -> (u16, u16) {
        (
            self.columns.len().max(1) as u16,
            self.rows.len().max(1) as u16,
        )
    }
}

/// **How many tracks a grid item covers** — a count, or every one there is.
///
/// `All` is CSS's `1 / -1`: "to the end of the grid", whatever the template turns out to say. It
/// stays true when a column is added later, which a hard-coded count does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    /// Exactly this many tracks.
    Of(u16),
    /// Every track on that axis.
    All,
}

impl From<u16> for Span {
    fn from(n: u16) -> Self {
        Span::Of(n)
    }
}

impl From<i32> for Span {
    fn from(n: i32) -> Self {
        Span::Of(n.max(1) as u16)
    }
}

impl From<&str> for Span {
    /// `"all"` (or CSS's `"-1"`) for every track; a number for that many.
    fn from(t: &str) -> Self {
        match t.trim().to_ascii_lowercase().as_str() {
            "all" | "-1" => Span::All,
            n => n.parse::<u16>().map_or(Span::All, Span::Of),
        }
    }
}

impl Span {
    /// The stored span: a count, or the [`ALL`](GridCell::ALL) sentinel.
    pub(crate) fn stored(self) -> u16 {
        match self {
            Span::Of(n) => n.max(1),
            Span::All => GridCell::ALL,
        }
    }
}

/// **Where an item starts on one axis, and how far it reaches** — CSS `grid-column` /
/// `grid-row`, in the spellings a stylesheet writes them.
///
/// | spelling | means |
/// | --- | --- |
/// | `"3"` / `3` | start at line 3, one track |
/// | `"1 / -1"` | start at line 1, run to the end |
/// | `"1 / span 2"` | start at line 1, cover two tracks |
/// | `"2 / 4"` | start at line 2, end at line 4 |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridLine {
    /// 1-based start line.
    pub start: u16,
    /// How far it reaches from there.
    pub span: Span,
}

impl From<u16> for GridLine {
    fn from(start: u16) -> Self {
        GridLine {
            start: start.max(1),
            span: Span::Of(1),
        }
    }
}

impl From<i32> for GridLine {
    fn from(start: i32) -> Self {
        GridLine {
            start: start.max(1) as u16,
            span: Span::Of(1),
        }
    }
}

impl From<&str> for GridLine {
    /// The CSS spelling. Anything unreadable is line 1, one track — never a panic, because these
    /// arrive from config and from plugins.
    fn from(t: &str) -> Self {
        let t = t.trim();
        let Some((start, end)) = t.split_once('/') else {
            return GridLine {
                start: t.parse::<u16>().unwrap_or(1).max(1),
                span: Span::Of(1),
            };
        };
        let start_line = start.trim().parse::<u16>().unwrap_or(1).max(1);
        let end = end.trim();
        let span = match end.strip_prefix("span") {
            // `1 / span 2` — a count of tracks.
            Some(n) => Span::from(n.trim()),
            // `1 / -1` — to the end. `2 / 4` — up to that line, which is 4 - 2 tracks.
            None => match end.parse::<i32>() {
                Ok(-1) => Span::All,
                Ok(line) if line > start_line as i32 => Span::Of((line - start_line as i32) as u16),
                _ => Span::All,
            },
        };
        GridLine {
            start: start_line,
            span,
        }
    }
}

impl GridCell {
    /// **Span every track on that axis** — CSS's `1 / -1`.
    ///
    /// A sentinel rather than a field, so [`Layout`] stays `Copy`. How many tracks that turns out
    /// to be is the *parent's* answer, so it is resolved during layout, when the parent is known —
    /// which is also what lets a child be built before whatever places it.
    pub const ALL: u16 = u16::MAX;

    /// This cell with every `ALL` span replaced by the real track counts.
    pub(crate) fn resolved(self, columns: u16, rows: u16) -> Self {
        Self {
            col_span: if self.col_span == Self::ALL {
                columns.max(1)
            } else {
                self.col_span
            },
            row_span: if self.row_span == Self::ALL {
                rows.max(1)
            } else {
                self.row_span
            },
            ..self
        }
    }
}
