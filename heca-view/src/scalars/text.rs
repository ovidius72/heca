//! The value sets that say how text sits.

use crate::PropValue;

value_set! {
    /// Which side a control's label sits on — mirrors grid-ui `LabelSide` (`Checkbox`).
    pub enum ViewLabelSide {
        Right => "right",
        Left => "left",
    }
}

value_set! {
    /// How text sits in its box — mirrors grid-ui `TextAlign` (`Label`).
    ///
    /// Distinct from [`ViewAlign`](crate::ViewAlign), which is where a *widget* sits in its parent.
    /// The two read alike and mean different things, which is why both names say what they align.
    pub enum ViewTextAlign {
        Start => "start",
        Center => "center",
        End => "end",
    }
}

value_set! {
    /// Which end of a label is cut when its text does not fit — mirrors grid-ui `Ellipsis` (`Label`).
    ///
    /// Two, because the two kinds of text read from opposite ends: a label is identified by its
    /// beginning, a path by its end.
    pub enum ViewEllipsis {
        /// Keep the head, cut the tail.
        End => "end",
        /// Keep the tail, cut the head — what a path needs.
        Start => "start",
    }
}

value_set! {
    /// How heavy a label's glyphs are — mirrors grid-ui `FontWeight` (`Label`), CSS `font-weight`.
    pub enum ViewFontWeight {
        /// The ordinary weight (CSS 400).
        Regular => "regular",
        /// A little heavier (CSS 500).
        Medium => "medium",
        /// Between medium and bold (CSS 600).
        SemiBold => "semi_bold",
        /// Bold (CSS 700).
        Bold => "bold",
    }
}

value_set! {
    /// The air after every letter of a label, as a step of its font size — mirrors grid-ui
    /// `LetterSpacing` (`Label`), CSS `letter-spacing`.
    pub enum ViewLetterSpacing {
        /// A little less than the font's own spacing.
        Tight => "tight",
        /// The font's own spacing.
        Normal => "normal",
        /// A little more — a small uppercase heading.
        Wide => "wide",
        /// More again.
        Wider => "wider",
    }
}

value_set! {
    /// How tall one line of a label is, as a step of its font size — mirrors grid-ui `LineHeight`
    /// (`Label`), CSS `line-height`.
    pub enum ViewLineHeight {
        /// Lines close together.
        Tight => "tight",
        /// The library's own line.
        Normal => "normal",
        /// More air between lines.
        Relaxed => "relaxed",
        /// Double spacing.
        Loose => "loose",
    }
}

value_set! {
    /// The case a label's text is shown in — mirrors grid-ui `TextTransform` (`Label`), CSS
    /// `text-transform`. Only what is drawn changes; the text keeps the casing it was written in.
    pub enum ViewTextTransform {
        /// As written.
        None => "none",
        /// Every letter in capitals.
        Uppercase => "uppercase",
        /// Every letter in small letters.
        Lowercase => "lowercase",
        /// The first letter of every word in capitals.
        Capitalize => "capitalize",
    }
}
