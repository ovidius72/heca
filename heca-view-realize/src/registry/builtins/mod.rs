//! Built-in registration definitions: one factory/metadata entry per supported widget.

use heca_grid_ui::{
    Action, Alert, Badge, BadgeButton, Button, Card, Checkbox, Choice, DockFrame, Flex, Gauge,
    Glyph, Icon, IconButton, Image, Input, Item, ItemGroup, KeyHintGroup, Label, LandingSlot, MarkerGroup,
    NfGlyph, NfIcon, Overlay, Panel, ProgressBar, RailCell, Row as GridRow, ScrollRegion, Select,
    Separator, Spinner, Splitter, StatusDot, Surface, Tabs, Tag, Toast, Toggle,
};
use heca_grid_ui::reactive::{SignalGet, SignalUpdate};
use heca_grid_ui::builders::IdentityExt;
use heca_view::{PropValue, ViewEvent, ViewNode, WidgetId};

use super::{RealizeCx, RealizeError, WidgetDefinition, WidgetRegistry};
use crate::intents::{change_intent, intent_carrier, press_intent, slot_of, warn_unknown_slot};
use crate::props::{glyph_prop, name_prop, option_values, text_of};
use crate::style::with_props;
use crate::vocab::{severity_prop, toast_position};
use crate::widgets::{
    button_of, option_change, realize_button, realize_card_grid, realize_choice, realize_flex,
    realize_grid, realize_options, toggle_change,
};


mod containers;
mod composition;
mod controls;
mod display;
mod overlays;
mod surfaces;
mod keyed;

pub(super) fn register(registry: &mut WidgetRegistry) -> Result<(), RealizeError> {
    for definition in [
        WidgetDefinition::new("keyed", keyed::build).names("Keyed", "Keyed").inputs::<keyed::Inputs>()?.children().update(keyed::update),
        WidgetDefinition::new("dialog", surfaces::dialog).names("Dialog", "Dialog")
            .properties::<heca_grid_ui::Dialog>().state::<surfaces::OpenInputs>()?.children().events(&[ViewEvent::Dismiss]),
        WidgetDefinition::new("command_palette", surfaces::command_palette).names("CommandPalette", "CommandPalette")
            .properties::<heca_grid_ui::CommandPalette>().inputs::<surfaces::PaletteInputs>()?
            .inputs::<surfaces::PaletteState>()?.state_node(surfaces::palette_state).children_of(&[WidgetId::new("command")]).events(&[ViewEvent::Dismiss]),
        WidgetDefinition::new("command", surfaces::command).names("Command", "Command")
            .properties::<heca_grid_ui::Command>().inputs::<surfaces::CommandInputs>()?.events(&[ViewEvent::Press]).declaration(),
        WidgetDefinition::new("toast_stack", surfaces::toast_stack).names("ToastStack", "ToastStack")
            .properties::<heca_grid_ui::ToastStack>().inputs::<surfaces::StackInputs>()?
            .state_node(surfaces::queue).children_of(&[WidgetId::Toast]).events(&[ViewEvent::Dismiss, ViewEvent::Action]),
        WidgetDefinition::new("pane", composition::pane).names("Pane", "Pane").properties::<heca_grid_ui::Pane>().children(),
        WidgetDefinition::new("focus_scope", composition::focus_scope).names("FocusScope", "FocusScope").properties::<heca_grid_ui::FocusScope>().children(),
        WidgetDefinition::new("chrome_region", composition::chrome_region).names("ChromeRegion", "ChromeRegion").properties::<heca_grid_ui::ChromeRegion>().inputs::<composition::RegionInputs>()?.children(),
        WidgetDefinition::new(WidgetId::VStack, containers::v_stack).names("VStack", "Flex").children(),
        WidgetDefinition::new(WidgetId::HStack, containers::h_stack).names("HStack", "Flex").children(),
        WidgetDefinition::new(WidgetId::Row, containers::row).names("Row", "Row").properties::<GridRow>().children().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::Card, containers::card).names("Card", "Card").children(),
        WidgetDefinition::new(WidgetId::Surface, containers::surface).names("Surface", "Surface").children(),
        WidgetDefinition::new(WidgetId::Panel, containers::panel).names("Panel", "Panel").properties::<Panel>().children(),
        WidgetDefinition::new(WidgetId::Overlay, overlays::overlay).names("Overlay", "Overlay").properties::<Overlay>().children().events(&[ViewEvent::Dismiss]),
        WidgetDefinition::new(WidgetId::KeyHintGroup, overlays::key_hint_group).names("KeyHintGroup", "KeyHintGroup").properties::<KeyHintGroup>().children(),
        WidgetDefinition::new(WidgetId::Scroll, containers::scroll).names("Scroll", "ScrollRegion").properties::<ScrollRegion>().children(),
        WidgetDefinition::new(WidgetId::Line, display::line).names("Line", "Line").properties::<heca_grid_ui::Line>(),
        WidgetDefinition::new(WidgetId::Resizable, containers::resizable).names("Resizable", "Resizable").properties::<heca_grid_ui::Resizable>().children(),
        WidgetDefinition::new(WidgetId::TreeLayout, containers::tree_layout).names("TreeLayout", "TreeLayout").children(),
        WidgetDefinition::new(WidgetId::MessageList, containers::message_list).names("MessageList", "MessageList").properties::<heca_grid_ui::MessageList>().children(),
        WidgetDefinition::new(WidgetId::Label, display::label).names("Label", "Label").properties::<Label>(),
        WidgetDefinition::new(WidgetId::Button, controls::button).names("Button", "Button").properties::<Button>().children().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::Badge, display::badge).names("Badge", "Badge").properties::<Badge>(),
        WidgetDefinition::new(WidgetId::Tag, display::tag).names("Tag", "Tag").properties::<Tag>().children(),
        WidgetDefinition::new(WidgetId::Alert, display::alert).names("Alert", "Alert").properties::<Alert>(),
        WidgetDefinition::new(WidgetId::StatusDot, display::status_dot).names("StatusDot", "StatusDot").properties::<StatusDot>().state::<display::DotInputs>()?,
        WidgetDefinition::new(WidgetId::Separator, display::separator).names("Separator", "Separator").properties::<Separator>(),
        WidgetDefinition::new(WidgetId::LandingSlot, controls::landing_slot).names("LandingSlot", "LandingSlot").properties::<LandingSlot>(),
        WidgetDefinition::new(WidgetId::Splitter, controls::splitter).names("Splitter", "Splitter").properties::<Splitter>().events(&[ViewEvent::Resize]),
        WidgetDefinition::new(WidgetId::Gauge, display::gauge).names("Gauge", "Gauge").properties::<Gauge>(),
        WidgetDefinition::new(WidgetId::Spinner, display::spinner).names("Spinner", "Spinner"),
        WidgetDefinition::new(WidgetId::Progress, display::progress).names("Progress", "ProgressBar").properties::<ProgressBar>(),
        WidgetDefinition::new(WidgetId::Tile, display::tile).names("Tile", "Tile").children(),
        WidgetDefinition::new(WidgetId::ButtonGroup, controls::button_group).names("ButtonGroup", "ButtonGroup").properties::<heca_grid_ui::widgets::ButtonGroup>().children_of(&[WidgetId::Button]),
        WidgetDefinition::new(WidgetId::NfIcon, display::nf_icon).names("NfIcon", "NfIcon").properties::<NfIcon>().validate(|node, _| display::nf_glyph(node).map(|_| ())),
        WidgetDefinition::new(WidgetId::Icon, display::icon).names("Icon", "Icon").properties::<Icon>().validate(|node, _| display::icon_glyph(node).map(|_| ())),
        WidgetDefinition::new(WidgetId::Image, display::image).names("Image", "Image").properties::<Image>().children(),
        WidgetDefinition::new(WidgetId::Input, controls::input).names("Input", "Input").properties::<Input>().events(&[ViewEvent::Change, ViewEvent::Submit]),
        WidgetDefinition::new(WidgetId::Toggle, controls::toggle).names("Toggle", "Toggle").properties::<Toggle>().events(&[ViewEvent::Change]),
        WidgetDefinition::new(WidgetId::Checkbox, controls::checkbox).names("Checkbox", "Checkbox").properties::<Checkbox>().events(&[ViewEvent::Change]),
        WidgetDefinition::new(WidgetId::BadgeButton, controls::badge_button).names("BadgeButton", "BadgeButton").properties::<BadgeButton>().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::IconButton, controls::icon_button).names("IconButton", "IconButton").properties::<IconButton>().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::Item, controls::item).names("Item", "Item").properties::<Item>().children().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::RailCell, controls::rail_cell).names("RailCell", "RailCell").properties::<RailCell>().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::Choice, controls::choice).names("Choice", "Choice").properties::<Choice>().children().events(&[ViewEvent::Press]),
        WidgetDefinition::new(WidgetId::Select, controls::select).names("Select", "Select").properties::<Select>().children_of(&[WidgetId::Choice]).events(&[ViewEvent::Change]),
        WidgetDefinition::new(WidgetId::Tabs, controls::tabs).names("Tabs", "Tabs").properties::<Tabs>().children_of(&[WidgetId::Choice]).events(&[ViewEvent::Change]),
        WidgetDefinition::new(WidgetId::ItemGroup, containers::item_group).names("ItemGroup", "ItemGroup").properties::<ItemGroup>().children().events(&[ViewEvent::Toggle]),
        WidgetDefinition::new(WidgetId::MarkerGroup, containers::marker_group).names("MarkerGroup", "MarkerGroup").properties::<MarkerGroup>().children(),
        WidgetDefinition::new(WidgetId::Grid, containers::grid).names("Grid", "Grid").children(),
        WidgetDefinition::new(WidgetId::CardGrid, containers::card_grid).names("CardGrid", "CardGrid").properties::<heca_grid_ui::widgets::CardGrid>().children().events(&[ViewEvent::Activate, ViewEvent::Move, ViewEvent::Dismiss]),
        WidgetDefinition::new(WidgetId::DockFrame, containers::dock_frame).names("DockFrame", "DockFrame").properties::<DockFrame>().children().events(&[ViewEvent::Toggle]),
        WidgetDefinition::new(WidgetId::Toast, overlays::toast).names("Toast", "Toast").properties::<Toast>().inputs::<surfaces::ToastInputs>()?.children().events(&[ViewEvent::Dismiss, ViewEvent::Action]),
        WidgetDefinition::new(WidgetId::ScrollBar, display::scroll_bar).names("ScrollBar", "ScrollBar").state::<display::ScrollBarState>()?.events(&[ViewEvent::Change]),
    ] {
        registry.register(definition)?;
    }
    Ok(())
}

fn attach_children(mut container: heca_grid_ui::WidgetModel, node: &ViewNode, cx: &mut RealizeCx<'_>) -> Result<heca_grid_ui::WidgetModel, RealizeError> {
    container.base_mut().children.extend(cx.children(node)?);
    Ok(container)
}
