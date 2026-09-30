use super::*;
use crate::{Intent, PropValue, ViewEvent};

// Which kinds take which event. A kind is listed only for an event `realize` wires for it: a `press`
// on a `Separator` would be a setter that silently does nothing. The hint is not here — it is on
// every kind, as `Style::on_hint`.
//
// `ScrollBar` is absent from this table and from the SDK entirely: host-only, its state is live
// host signals, and `realize` refuses it outright rather than render a dead control.
with_event!(
    Row { on_press => Press }
    Button { on_press => Press }
    IconButton { on_press => Press }
    BadgeButton { on_press => Press }
    Item { on_press => Press }
    RailCell { on_press => Press }
    Choice { on_press => Press }
    Input { on_change => Change }
    Toggle { on_change => Change }
    Checkbox { on_change => Change }
    Select { on_change => Change }
    Tabs { on_change => Change }
    ItemGroup { on_toggle => Toggle }
    DockFrame { on_toggle => Toggle }
    Toast { on_action => Action, on_dismiss => Dismiss }
    Overlay { on_dismiss => Dismiss }
    CardGrid { on_activate => Activate, on_move => Move, on_dismiss => Dismiss }
);

with_text!(
    Button, Panel, Input, Choice, Item, Toast, Alert, ItemGroup, DockFrame,
);
