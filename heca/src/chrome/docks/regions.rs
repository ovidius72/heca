//! **`regions("sidebar.left")` — a region is a place, reached by name, that holds a list.**
//!
//! ```ignore
//! regions("sidebar.left").append(|| FileTree::new("files"));
//! regions("sidebar.left").prepend(|| Outline::new("outline"));
//! regions("sidebar.right").remove("workspaces");     // by id, wherever it is
//! regions("sidebar.left").retain(|id| id != "docker");
//! regions("sidebar.left").gap("md");
//! ```
//!
//! That is the whole surface, and it is the line a plugin author writes: no host object, no
//! crate-private type, no registry id (Rule Zero). A region is not a widget you build — it is a
//! list the app and every plugin add to — so it is fetched by name rather than constructed.
//!
//! **Not a component — and why.** A component is something you build and place in a tree, in one
//! expression (THE SYNTAX). A region is the opposite: it **already exists** in the window — the two
//! sidebars and the two bars — and it is **shared**: heca adds its docks, every extension and plugin
//! adds its own, each from its own code and at its own time, and the user's config moves them. No
//! single place writes the sidebar's tree, so there is nothing to build: a region is reached by its
//! name and changed with operations, the way a page reaches `document.body` and calls
//! `appendChild` / `remove` on it. The docks put into it are what carries the UI.
//!
//! **How it works.**
//! 1. A call (`append`, `prepend`, `remove`, `retain`, `gap`) is queued — at once, from anywhere on
//!    the window's thread, before `heca::run` or while the app runs.
//! 2. The window's host ([`ChromeHost`](crate::chrome::ChromeHost)) applies the queue as the window
//!    starts and then on every frame, in the order the calls were made.
//! 3. Where each added dock goes, in what order and how big is the user's `[[docks]]` first; the
//!    region named in the call, and the dock's `.order(..)` / `.flex(..)`, count where the user said
//!    nothing.
//! 4. A dock handed over as a recipe (`|| Dock::new(..)`) can be made again by the host — when a
//!    reload brings it back from `"none"`; a dock handed over as itself can be moved, but once out
//!    it returns only with a restart.
//! 5. The host then starts each new dock (its `on_activate`) and draws it at its next frame.
//!
//! Limits today: the queue is per thread (`thread_local!`), so a call from another thread — a
//! background worker, an extension's server-side service — is never applied; and a dock already
//! added cannot be fetched to call its methods. Both are planned (F013).
//!
//! **What each piece owns:**
//!
//! | who | says |
//! |---|---|
//! | the region | which containers it holds, in what order they were added, and the air between them |
//! | each container | how big it is — `.flex(n)` on the body it builds — and, if it must, where it sits: `.order(..)` on that body |
//! | whoever adds it | the same two, on the dock as it is added — `.append(Docker::new("d").flex(3.0).order(-1))` — and they win over the body's |
//!
//! The region never holds a size list: it is a list anyone may append to, so sizes written there
//! break the moment a plugin adds one more (the `template_row` that existed was removed for exactly
//! that). No methods take an index either: a plugin inserting anything would shift everyone after
//! it with nothing failing. The ends are stable, and a container that needs a particular place says
//! so with `.order`.
//!
//! **Names.** `"sidebar.left"`, `"sidebar.right"`, `"bar.top"`, `"bar.bottom"` — dotted, the
//! spelling a context menu path already uses, so a plugin learns one way to say *where*. The older
//! `"left-sidebar"` / `"left"` still read, through the same one parser RPC and config use. A name
//! nobody knows is **said out loud** and the call does nothing: a plugin's typo must not take the
//! app down, and must not vanish either.
//!
//! **Any time.** Calls are queued and the host applies them — as it starts
//! ([`ChromeHost::mount_pending`](crate::chrome::ChromeHost::mount_pending)) and then on every
//! frame ([`ChromeHost::apply_pending`](crate::chrome::ChromeHost::apply_pending)), so a plugin may
//! add or take out a dock while the app runs.
//!
//! **Where a dock goes and in what order is the user's word first.** The config's `[[docks]]`
//! entries (by kind or by id) give a `location` (a region, or `"none"`) and an `order`; the region a
//! call names, and the dock's own `.order(..)`, count only where the user said nothing.
//!
//! **A dock or the recipe for one.** `.append(Rooms::new("rooms"))` hands over a dock;
//! `.append(|| Rooms::new("rooms"))` hands over how to make it, so the host can make it again when
//! the user's `[[docks]]` brings it back after a reload (`"none"` → a region). A dock given as itself
//! can still be moved by a reload, but once taken out it returns only with a restart. A dock's
//! cursor, scroll and folded state are kept by its id, so one made again finds them.

// `prepend`, `remove`, `retain` and `gap` are for plugins; the app itself only appends today, so in
// the binary they read as unused. They are exercised by `host::region_list_tests`.
#![allow(dead_code)]

use crate::chrome::RegionId;
use crate::providers::Provider;

/// **The region called `name`** — see the module docs.
///
/// Never fails: an unknown name gives a handle whose every call reports the name and the ones that
/// exist, and does nothing else.
pub fn regions(name: &str) -> RegionHandle {
    RegionHandle {
        region: name.parse().ok(),
        name: name.to_string(),
    }
}

/// A region, fetched by name. Every call returns the handle, so calls chain.
pub struct RegionHandle {
    region: Option<RegionId>,
    name: String,
}

impl RegionHandle {
    /// **Add at the end** — one container or several, in the order given. Say how big it is or
    /// where it sits on the dock itself: `.append(Docker::new("docker").flex(2.0).order(-1))`.
    /// A recipe — `.append(|| Docker::new("docker"))` — lets the host make it again on a reload.
    pub fn append<M>(self, containers: impl IntoDocks<M>) -> Self {
        let ops = containers
            .into_docks()
            .into_iter()
            .map(RegionOp::Append)
            .collect();
        self.queue(ops)
    }

    /// **Add at the start** — one container or several; several keep the order given, all of them
    /// ahead of what was there.
    pub fn prepend<M>(self, containers: impl IntoDocks<M>) -> Self {
        // Prepending each in reverse puts the group at the front in the order the caller wrote.
        let ops = containers
            .into_docks()
            .into_iter()
            .rev()
            .map(RegionOp::Prepend)
            .collect();
        self.queue(ops)
    }

    /// **Take out the dock whose id is `id`**, wherever it is — the user may have placed it in
    /// another region through `[[docks]]` or by hand. An id no dock has is said out loud.
    ///
    /// Removing a built-in is allowed — that is the point — so nothing in the app may assume a
    /// particular container is there. A dock taken out this way is not made again by a reload.
    pub fn remove(self, id: &str) -> Self {
        self.queue(vec![RegionOp::Remove(id.to_string())])
    }

    /// **Keep only the containers `keep` says yes to**, by name.
    ///
    /// ```ignore
    /// regions("sidebar.left").retain(|id| id != "docker");   // everything but docker stays
    /// ```
    ///
    /// **Why `retain` and not `filter`:** in Rust, `filter` builds a *new* sequence and leaves the
    /// old one alone; `retain` changes the list it is called on, which is what this does — the
    /// containers it says no to are taken out of the region. Same idea, the name Rust already gives
    /// it (`Vec::retain`), so it reads as what it does.
    pub fn retain(self, keep: impl Fn(&str) -> bool + 'static) -> Self {
        self.queue(vec![RegionOp::Retain(Box::new(keep))])
    }

    /// **Air between the containers** — pixels, a step of the theme's rhythm (`"sm"`), or either as
    /// a string, exactly as everywhere else. Unset, the theme's `"sm"` step.
    pub fn gap(self, gap: impl Into<heca_grid_ui::style::Space>) -> Self {
        self.queue(vec![RegionOp::Gap(gap.into())])
    }

    fn queue(self, ops: Vec<RegionOp>) -> Self {
        let Some(region) = self.region else {
            crate::chrome::input::identity::warn_author(format!(
                "[heca] there is no region called '{}' — the regions are 'sidebar.left', \
                 'sidebar.right', 'bar.top' and 'bar.bottom'. Nothing was changed.",
                self.name,
            ));
            return self;
        };
        QUEUE.with(|q| q.borrow_mut().extend(ops.into_iter().map(|op| (region, op))));
        self
    }
}

/// One queued change to a region, applied in the order it was made.
pub(crate) enum RegionOp {
    Append(Placed),
    Prepend(Placed),
    Remove(String),
    Retain(Box<dyn Fn(&str) -> bool>),
    Gap(heca_grid_ui::style::Space),
}

/// **What a region says about arranging its own contents** — only the air between them. Sizes
/// belong to each container (`.flex(n)` on its body), never to the region.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RegionLayout {
    /// Air between the containers. `None` ⇒ the theme's `"sm"` step.
    pub gap: Option<heca_grid_ui::style::Space>,
}

thread_local! {
    /// Changes made and not yet applied by the host.
    static QUEUE: std::cell::RefCell<Vec<(RegionId, RegionOp)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Take every queued change, in the order made. The host calls this; nothing else should.
pub(crate) fn take_pending() -> Vec<(RegionId, RegionOp)> {
    QUEUE.with(|q| std::mem::take(&mut *q.borrow_mut()))
}

/// **How to make a dock again** — what `.append(|| Dock::new(..))` hands over.
pub(crate) type Recipe = std::rc::Rc<dyn Fn() -> Box<dyn Provider>>;

/// **One value per region**, reached by the region's own name — `shown[RegionId::TopBar]`.
///
/// The one place "four of something, one per region" is written down. It replaced a
/// four-slot array indexed by hand in the host, and four separate fields plus a private copy of the
/// region list in the show/hide handler — two lists of the same four regions that could only drift.
/// A region added to [`RegionId`] reaches every map at once, because [`RegionMap::from_fn`] asks
/// [`RegionId::ALL`] rather than a count written here.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RegionMap<T>([T; 4]);

impl<T> RegionMap<T> {
    /// Build one value per region from the region itself.
    pub fn from_fn(mut value: impl FnMut(RegionId) -> T) -> Self {
        RegionMap(RegionId::ALL.map(&mut value))
    }

    /// Every value, in [`RegionId::ALL`] order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.0.iter()
    }

    /// Every value, mutably, in [`RegionId::ALL`] order.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.0.iter_mut()
    }
}

impl<T> std::ops::Index<RegionId> for RegionMap<T> {
    type Output = T;
    fn index(&self, region: RegionId) -> &T {
        &self.0[region.index()]
    }
}

impl<T> std::ops::IndexMut<RegionId> for RegionMap<T> {
    fn index_mut(&mut self, region: RegionId) -> &mut T {
        &mut self.0[region.index()]
    }
}

/// **Which regions `[settings]` says to show** — the one place the four `show_*` config keys become
/// a [`RegionMap`], read at startup and again on reload.
pub fn shown_from_settings(settings: &heca_config::settings::SettingsConfig) -> RegionMap<bool> {
    RegionMap::from_fn(|region| match region {
        RegionId::LeftSidebar => settings.show_left_sidebar,
        RegionId::RightSidebar => settings.show_right_sidebar,
        RegionId::TopBar => settings.show_top_bar,
        RegionId::BottomBar => settings.show_bottom_bar,
    })
}

/// **What whoever adds a dock says about it** — how big it is and where it sits.
///
/// The dock's own author says this on the body it builds (`.flex(n)` / `.order(..)`), which is the
/// default. Whoever *places* a dock they did not write says it here, and **the placer wins** — the
/// same way a style written where an element is used beats the one its component shipped with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DockPlacement {
    /// CSS `flex: <n>` — `n` parts of the region. `None` ⇒ whatever the dock's body says, else 1.
    pub flex: Option<f32>,
    /// CSS `order` — where it sits among the region's docks. `None` ⇒ the body's, else as added.
    pub order: Option<heca_grid_ui::order::Order>,
}

/// **A dock on its way into a region**, carrying what its placer said about it.
///
/// Made by calling [`.flex`](PlaceDock::flex) or [`.order`](PlaceDock::order) on any dock:
///
/// ```ignore
/// regions("sidebar.left").append(Workspaces::new("files").flex(3.0));
/// regions("sidebar.left").append(Docker::new("docker").order(-1));   // first, whoever wrote it
/// regions("sidebar.left").append(Notes::new("notes").flex(0.0).order([0, 5]));
/// ```
pub struct Placed {
    pub(crate) provider: Box<dyn Provider>,
    pub(crate) placement: DockPlacement,
    /// How to make the dock again, when it was handed over as a recipe.
    pub(crate) recipe: Option<Recipe>,
}

impl Placed {
    /// CSS `flex: <n>` — see [`PlaceDock::flex`].
    pub fn flex(mut self, parts: f32) -> Self {
        self.placement.flex = Some(parts);
        self
    }

    /// CSS `order` — see [`PlaceDock::order`].
    pub fn order(mut self, order: impl Into<heca_grid_ui::order::Order>) -> Self {
        self.placement.order = Some(order.into());
        self
    }
}

impl From<Box<dyn Provider>> for Placed {
    fn from(provider: Box<dyn Provider>) -> Self {
        Placed {
            provider,
            placement: DockPlacement::default(),
            recipe: None,
        }
    }
}

/// **`.flex` and `.order` on any dock you add** — built once here, for every dock type there is or
/// will be, so no dock author has to write them and no placer has to know which docks have them.
pub trait PlaceDock: Sized {
    /// Box it with nothing said yet — what the two builders start from.
    fn into_placed(self) -> Placed;

    /// **CSS `flex: <n>` — `n` parts of the region**, whatever the dock holds. `0.0` is as big as
    /// its content. Overrides what the dock's own body asked for. Same meaning as
    /// `LayoutExt::flex` on a widget — see `docs/layout.md` → Shares.
    fn flex(self, parts: f32) -> Placed {
        self.into_placed().flex(parts)
    }

    /// **CSS `order` — where it sits among the region's docks.** Lower first, ties as added; a list
    /// (`[0, 5]`) lands between two docks at `0` and `1`. Overrides what the dock's own body asked
    /// for. Same meaning as `LayoutExt::order` — see `docs/layout.md` → Order.
    fn order(self, order: impl Into<heca_grid_ui::order::Order>) -> Placed {
        self.into_placed().order(order)
    }
}

impl<P: Provider + 'static> PlaceDock for P {
    fn into_placed(self) -> Placed {
        Placed::from(Box::new(self) as Box<dyn Provider>)
    }
}

impl PlaceDock for Box<dyn Provider> {
    fn into_placed(self) -> Placed {
        Placed::from(self)
    }
}

/// **One dock or several** — what [`RegionHandle::append`] and [`prepend`](RegionHandle::prepend)
/// take: a dock, a dock with `.flex` / `.order` said about it, a recipe that makes a dock
/// (`|| Rooms::new("rooms")`), or a list of any of them, so a caller hands over whichever shape
/// they hold and never goes looking for another spelling.
///
/// `M` only tells a dock from a recipe ([`AsDock`] / [`AsRecipe`]); nobody writes it.
pub trait IntoDocks<M> {
    /// The docks, each with what its placer said.
    fn into_docks(self) -> Vec<Placed>;
}

/// [`IntoDocks`]'s mark for a dock handed over as itself.
pub struct AsDock;
/// [`IntoDocks`]'s mark for a dock handed over as the recipe that makes it.
pub struct AsRecipe;
/// [`IntoDocks`]'s mark for a recipe that also says the dock's `.flex` / `.order`.
pub struct AsPlacedRecipe;

impl<F> IntoDocks<AsPlacedRecipe> for F
where
    F: Fn() -> Placed + 'static,
{
    fn into_docks(self) -> Vec<Placed> {
        let first = self();
        let recipe: Recipe = std::rc::Rc::new(move || self().provider);
        vec![Placed { recipe: Some(recipe), ..first }]
    }
}

impl<F, P> IntoDocks<AsRecipe> for F
where
    F: Fn() -> P + 'static,
    P: Provider + 'static,
{
    fn into_docks(self) -> Vec<Placed> {
        let recipe: Recipe = std::rc::Rc::new(move || Box::new(self()) as Box<dyn Provider>);
        let mut placed = Placed::from(recipe());
        placed.recipe = Some(recipe);
        vec![placed]
    }
}

impl<P: Provider + 'static> IntoDocks<AsDock> for P {
    fn into_docks(self) -> Vec<Placed> {
        vec![self.into_placed()]
    }
}

impl IntoDocks<AsDock> for Box<dyn Provider> {
    fn into_docks(self) -> Vec<Placed> {
        vec![self.into_placed()]
    }
}

impl IntoDocks<AsDock> for Placed {
    fn into_docks(self) -> Vec<Placed> {
        vec![self]
    }
}

impl<M, D: IntoDocks<M>> IntoDocks<M> for Vec<D> {
    fn into_docks(self) -> Vec<Placed> {
        self.into_iter().flat_map(IntoDocks::into_docks).collect()
    }
}

impl<M, D: IntoDocks<M>, const N: usize> IntoDocks<M> for [D; N] {
    fn into_docks(self) -> Vec<Placed> {
        self.into_iter().flat_map(IntoDocks::into_docks).collect()
    }
}

#[cfg(test)]
mod region_map_tests {
    use super::*;

    /// One value per region, reached by the region itself — no index written anywhere.
    #[test]
    fn a_region_map_is_read_and_written_by_region() {
        let mut shown = RegionMap::from_fn(|r| r == RegionId::LeftSidebar);
        assert!(shown[RegionId::LeftSidebar]);
        assert!(!shown[RegionId::TopBar]);
        shown[RegionId::TopBar] = true;
        assert!(shown[RegionId::TopBar]);
        assert_eq!(shown.iter().filter(|v| **v).count(), 2);
    }

    /// Each `[settings] show_*` key lands on its own region — the one place the two are paired.
    #[test]
    fn each_show_setting_lands_on_its_own_region() {
        for region in RegionId::ALL {
            let settings = heca_config::settings::SettingsConfig {
                show_left_sidebar: region == RegionId::LeftSidebar,
                show_right_sidebar: region == RegionId::RightSidebar,
                show_top_bar: region == RegionId::TopBar,
                show_bottom_bar: region == RegionId::BottomBar,
                ..Default::default()
            };
            let shown = shown_from_settings(&settings);
            for other in RegionId::ALL {
                assert_eq!(shown[other], other == region, "{region:?} vs {other:?}");
            }
        }
    }
}
