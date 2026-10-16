//! **The vocabulary every menu writes its entries with** — the host's pane menu and a component's
//! row menus alike, so it belongs to neither.

use heca_grid_ui::widgets::Menu;

use crate::chrome::{DropdownItem, Intent, PropValue};

/// An entry whose visual identity (`id` → icon/label) differs from what it runs. The `action` is
/// the name [`resolve_action`](crate::input::resolve_action) resolves, and `args` are its arguments —
/// the same pair a `config.toml` binding or an RPC command would supply.
pub(crate) fn item_running(
    id: &str,
    label: &str,
    action: &str,
    args: &[(&str, PropValue)],
) -> DropdownItem {
    let intent = Intent::new(action).args(args.iter().map(|(k, v)| (*k, v.clone())));
    DropdownItem::with_intent(id, label, intent)
}

/// **Turn a list of entries into a declared [`Menu`]** — the bridge from the `DropdownItem`
/// vocabulary to a menu a widget carries.
///
/// This is what a component calls where it is drawing the row, so the menu's rows capture the row's
/// own ids and nothing has to resolve "what did you right-click" from a position afterwards.
///
/// Three things cross here, and each is deliberate:
///
/// - **The icon comes from the action**, via the action catalog, because `ActionMeta.icon`
///   is the single source of an action's `Glyph`. A component naming its own glyph would let the
///   sidebar's "Close pane" drift from the palette's `close`.
/// - **Behaviour crosses as an [`Intent`]**, wrapped in `InteractionIntent::View`, so a menu entry
///   is dispatched by name through the **central gate** — the same policy and destructive-confirm a
///   keypress or an RPC call gets. A menu row is not a back door.
/// - **`name` names the menu**, so other components may contribute rows to it
///   ([`Menu::name`](heca_grid_ui::widgets::Menu::name)). Pass `""` for a menu that is closed.
pub(crate) fn menu_from_items(
    title: &str,
    description: &str,
    name: &str,
    items: Vec<DropdownItem>,
    catalog: &crate::actions::ActionCatalog,
    emit: &crate::chrome::ChromeIntentEmitter,
) -> Menu {
    // **One conversion, two authoring paths.** The body moved to
    // `heca_view_realize::menu_from_items` so a *described* node can declare a menu and get the
    // identical one — this is the app's half, supplying the two things only it knows: an entry's
    // icon, from the action catalog, and how a choice is dispatched.
    heca_view_realize::menu_from_items(
        title,
        description,
        name,
        items,
        &|id| catalog.icon(id),
        &|intent| {
            let emit = emit.clone();
            Box::new(move || {
                emit.fire(crate::app::interaction::InteractionIntent::View(
                    intent.clone(),
                ))
            })
        },
    )
}
