//! The shell's state: region sizes and visibility, and each mount's scroll, cursor and focus.

use super::*;

/// A sidebar's size. Container-agnostic shell state. Not `Copy` (aliasing signal handle).
///
/// Whether the region is shown is NOT here — that is [`SharedChromeState::is_visible`], one value
/// per region for all four regions, so a sidebar has no second hidden/shown state to keep in step.
#[derive(Clone, Debug)]
pub(crate) struct RegionState {
    pub(crate) size: Signal<f32>,
}

impl RegionState {
    fn new(size: f32) -> Self {
        Self { size: signal(size) }
    }
}

/// Container-agnostic **shell** chrome state + the mounted containers' states.
/// One per app, held in `AppState`. See module docs (shell vs container split).
#[derive(Clone, Debug)]
pub struct SharedChromeState {
    events: ChromeEventBus,
    pub(crate) left: RegionState,
    pub(crate) right: RegionState,
    /// **Is each region shown** — the one visible/hidden state a region has. `[settings] show_*`
    /// sets where it starts, `set_region_visible` / `sidebar_left` / `sidebar_right` flip it, and a
    /// config reload sets it back ([`apply_visibility`](Self::apply_visibility)). A hidden region
    /// takes no space: a sidebar is zero wide, a bar zero tall.
    visible: RegionMap<Signal<bool>>,
    /// The (currently sole) mounted container's state. Becomes a container-id-keyed
    /// registry when a second container (Docker/agents/…) is added.
    pub workspaces: WorkspacesContainerState,
    /// Every pane's live facts, read by any component.
    pub panes: PaneRuntimeStore,
    /// Scroll offsets for containers' **own** nested scroll areas, keyed by the **mount id**.
    ///
    /// Per mount, not per container kind: the same container can be seated twice — two of them in
    /// one region, or one in each — and each mount scrolls its own content. Keying this by kind is
    /// what made two mounts scroll together, which is invisible until there are two.
    ///
    /// Created on first ask and kept here so an offset outlives the retained tree; a rebuild
    /// (triggered by something as small as a pane's git status changing) restores rather than
    /// resets. Shared, so a clone of this store aliases the same signals.
    container_scroll: std::rc::Rc<std::cell::RefCell<HashMap<String, Signal<f32>>>>,
    /// The container the last focus announcement named, kept **only** so a change is announced once
    /// ([`announce_focused_container`](Self::announce_focused_container)). Nothing reads it to decide
    /// where keys go: the window tree is the one record of where the keyboard is.
    announced_container: Signal<Option<String>>,
    /// A dock the host asked to give the keyboard to **before the tree had it** — its region was
    /// only just revealed. A request, not a record: the host lands it once the tree is rebuilt and
    /// clears it (`app::tree_focus`).
    dock_focus_request: Signal<Option<String>>,
    /// The mount that held the keyboard most recently — kept after it is released.
    ///
    /// It answers "which placement did the user mean?" for an action fired from somewhere with no
    /// target of its own: the command palette, an RPC call, a script. Without it, choosing a
    /// component's action from the palette while nothing is focused would have to guess between two
    /// placements — see `owning_mount`.
    last_focused_container: Signal<Option<String>>,
    /// **This placement's** selected row, as the row declared it (`key`).
    ///
    /// Per mount, exactly like the scroll offset and the keyboard target beside it: two placements
    /// of one container have two cursors. Created on first ask, and outliving the retained tree so
    /// a rebuild restores the cursor rather than dropping it.
    ///
    /// A row's `key` is what writes it — a row declares its identity once and the cursor, the
    /// right-click target and drag are readers of that one declaration.
    container_cursor: std::rc::Rc<std::cell::RefCell<HashMap<String, Signal<Option<String>>>>>,
}

impl SharedChromeState {
    /// This mount's own scroll offset signal, created on first ask.
    ///
    /// Keyed by **mount id**, so two mounts of the same container each get their own and scroll
    /// independently. The signal outlives the retained tree, so a rebuild restores the position
    /// instead of snapping to the top.
    pub fn container_scroll(&self, container: &str) -> Signal<f32> {
        if let Some(existing) = self.container_scroll.borrow().get(container) {
            return *existing;
        }
        let created = signal(0.0);
        self.container_scroll
            .borrow_mut()
            .insert(container.to_string(), created);
        created
    }

    /// Record a mount's scroll offset, emitting [`ChromeEvent::ContainerScrollChanged`] when it
    /// really moved. The epsilon guard is what stops a restore, or a wheel notch that changed
    /// nothing, from emitting.
    pub fn set_container_scroll(&self, container: &str, offset: f32) {
        let sig = self.container_scroll(container);
        if (sig.get_untracked() - offset).abs() <= f32::EPSILON {
            return;
        }
        sig.set(offset);
        self.events.emit(ChromeEvent::ContainerScrollChanged {
            container: container.to_string(),
            offset,
        });
    }

    /// **Say where the keyboard is**, once per change: `container` is the dock that holds it on the
    /// page (`None` for none). Emits [`ChromeEvent::ContainerFocusChanged`] only when it differs
    /// from the last announcement, and remembers a dock as the one that held the keyboard last.
    ///
    /// Written by the host's focus settle from the window tree, which is where keyboard focus lives.
    /// It is not a second place to put it: nothing asks this store which dock has the keyboard.
    pub fn announce_focused_container(&self, container: Option<String>) {
        if self.announced_container.get_untracked() == container {
            return;
        }
        self.announced_container.set(container.clone());
        // Remembered past the release, so an action fired from the palette or RPC has a placement
        // to mean when nothing currently holds the keyboard. Only a dock holding it updates this —
        // letting go does not, or the memory would be wiped by the very act it exists to survive.
        if container.is_some() {
            self.last_focused_container.set(container.clone());
        }
        self.events
            .emit(ChromeEvent::ContainerFocusChanged { container });
    }

    /// Ask for the keyboard to go to `mount` once the tree has it.
    pub fn request_dock_focus(&self, mount: String) {
        self.dock_focus_request.set(Some(mount));
    }

    /// The dock still waiting to be given the keyboard, if any.
    pub fn dock_focus_request(&self) -> Option<String> {
        self.dock_focus_request.get_untracked()
    }

    /// Drop the waiting request, landed or not.
    pub fn clear_dock_focus_request(&self) {
        if self.dock_focus_request.get_untracked().is_some() {
            self.dock_focus_request.set(None);
        }
    }

    /// The mount that holds the keyboard now, as last announced — `None` when no dock does.
    pub fn focused_container(&self) -> Option<String> {
        self.announced_container.get_untracked()
    }

    /// The mount that held the keyboard most recently, whether or not it still does.
    pub fn last_focused_container(&self) -> Option<String> {
        self.last_focused_container.get()
    }

    /// This placement's cursor signal, created on first ask.
    ///
    /// Keyed by **mount id** for the same reason the scroll offset is: place a container twice and
    /// each seating gets its own cursor over the same content.
    pub fn container_cursor(&self, container: &str) -> Signal<Option<String>> {
        if let Some(existing) = self.container_cursor.borrow().get(container) {
            return *existing;
        }
        let created = signal(None);
        self.container_cursor
            .borrow_mut()
            .insert(container.to_string(), created);
        created
    }

    /// Move a placement's cursor to the row that declared `key` (`None` clears it).
    ///
    /// Change-guarded, like every setter here, so republishing an unchanged cursor is free.
    pub fn set_container_cursor(&self, container: &str, key: Option<String>) {
        let sig = self.container_cursor(container);
        if sig.get_untracked() == key {
            return;
        }
        sig.set(key.clone());
        self.events.emit(ChromeEvent::ContainerCursorChanged {
            container: container.to_string(),
            key,
        });
    }

    /// Construct the store with the sidebars' widths. Every region starts shown; the app then
    /// applies `[settings] show_*` through [`apply_visibility`](Self::apply_visibility), the same
    /// call a config reload makes. Signals are created here — requires the reactive runtime,
    /// available on the UI thread at `AppState` construction.
    pub fn new(left_width: f32, right_width: f32) -> Self {
        let events = ChromeEventBus::default();
        Self {
            events: events.clone(),
            left: RegionState::new(left_width),
            right: RegionState::new(right_width),
            visible: RegionMap::from_fn(|_| signal(true)),
            workspaces: WorkspacesContainerState::new(events.clone()),
            panes: PaneRuntimeStore::new(events),
            container_scroll: std::rc::Rc::new(std::cell::RefCell::new(HashMap::new())),
            announced_container: signal(None),
            dock_focus_request: signal(None),
            last_focused_container: signal(None),
            container_cursor: std::rc::Rc::new(std::cell::RefCell::new(HashMap::new())),
        }
    }

    pub fn events(&self) -> ChromeEventBus {
        self.events.clone()
    }

    // ── Region (shell) reads/writes — bool/f32 are Copy → `.get()` is cheap ──

    /// Is `region` shown? The one answer, for the layout, the toggle buttons and plugins alike.
    pub fn is_visible(&self, region: RegionId) -> bool {
        self.visible[region].get()
    }

    /// Show or hide `region`. Returns whether anything changed — only a real change emits
    /// `chrome.region.visibility.changed`, and the caller re-flows the layout on `true`.
    pub fn set_visible(&self, region: RegionId, visible: bool) -> bool {
        if self.visible[region].get_untracked() == visible {
            return false;
        }
        self.visible[region].set(visible);
        self.events
            .emit(ChromeEvent::RegionVisibilityChanged { region, visible });
        true
    }

    /// Set every region to `shown` (what `[settings] show_*` says). Startup and config reload both
    /// come through here, so "the setting is the starting state" is one call. Returns whether any
    /// region changed.
    pub fn apply_visibility(&self, shown: &RegionMap<bool>) -> bool {
        let mut changed = false;
        for region in RegionId::ALL {
            changed |= self.set_visible(region, shown[region]);
        }
        changed
    }

    pub fn left_size(&self) -> f32 {
        self.left.size.get()
    }
    pub fn right_size(&self) -> f32 {
        self.right.size.get()
    }

    pub fn set_left_size(&self, size: f32) {
        self.resize(&self.left, RegionId::LeftSidebar, size);
    }
    pub fn set_right_size(&self, size: f32) {
        self.resize(&self.right, RegionId::RightSidebar, size);
    }

    /// Set a sidebar's size, announcing it only when it really changed.
    fn resize(&self, sidebar: &RegionState, region: RegionId, size: f32) {
        if (sidebar.size.get_untracked() - size).abs() <= f32::EPSILON {
            return;
        }
        sidebar.size.set(size);
        self.events
            .emit(ChromeEvent::RegionSizeChanged { region, size });
    }
}
