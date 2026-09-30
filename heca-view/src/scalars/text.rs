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
    /// Distinct from [`ViewAlign`], which is where a *widget* sits in its parent. The two read alike
    /// and mean different things, which is why both names say what they align.
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
