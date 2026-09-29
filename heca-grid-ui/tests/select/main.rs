#[path = "../common/mod.rs"]
mod common;

use common::{click_at, give_keyboard};
use heca_grid_ui::prelude::*;
use heca_grid_ui::{DrawCommand, Event, LayoutEngine, Point, Size, Theme};

mod choice;
mod navigation;
mod opening;
mod tabs;
