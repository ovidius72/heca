use crate::types::Rect;

// ── Disposition ────────────────────────────────────────────────

/// Where a pane currently lives in the workspace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Disposition {
    Embedded,         // in the BSP tree
    Floating,         // absolute position (stored in PaneTree.floats)
    Scratchpad,       // hidden until toggled
    Hidden,           // alive but not displayed
}

// ── Pane ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct Pane {
    pub id: u64,
    pub title: String,
    pub disposition: Disposition,
    pub background: [f32; 4],
}

impl Pane {
    pub fn new(id: u64, title: &str, background: [f32; 4]) -> Self {
        Self {
            id,
            title: title.to_string(),
            disposition: Disposition::Embedded,
            background,
        }
    }
}

// ── Split Direction ────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

// ── Layout Tree ────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum LayoutNode {
    Split {
        dir: SplitDirection,
        ratio: f32, // 0.0–1.0, fraction for the left/top child
        left: Box<LayoutNode>,
        right: Box<LayoutNode>,
    },
    Leaf {
        pane_id: u64,
    },
    Empty,
}

impl LayoutNode {
    /// Count leaf panes (ignore Empty).
    fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Split { left, right, .. } => left.leaf_count() + right.leaf_count(),
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Empty => 0,
        }
    }

    /// Find a pane ID in the tree (depth-first).
    fn contains(&self, pane_id: u64) -> bool {
        match self {
            LayoutNode::Split { left, right, .. } => left.contains(pane_id) || right.contains(pane_id),
            LayoutNode::Leaf { pane_id: pid } => *pid == pane_id,
            LayoutNode::Empty => false,
        }
    }

    /// Replace a leaf with a new node, or a no-op if not found.
    fn replace_leaf(&mut self, pane_id: u64, replacement: LayoutNode) -> bool {
        match self {
            LayoutNode::Split { left, right, .. } => {
                left.replace_leaf(pane_id, replacement.clone())
                    || right.replace_leaf(pane_id, replacement)
            }
            LayoutNode::Leaf { pane_id: pid } if *pid == pane_id => {
                *self = replacement;
                true
            }
            _ => false,
        }
    }
}

// ── PaneTree ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct PaneTree {
    pub panes: Vec<Pane>,
    pub root: LayoutNode,
    pub floats: Vec<(u64, Rect)>,
    pub next_id: u64,
}

impl PaneTree {
    pub fn new() -> Self {
        Self {
            panes: Vec::new(),
            root: LayoutNode::Empty,
            floats: Vec::new(),
            next_id: 1,
        }
    }

    /// Create a new pane and add it to the tree.
    /// If the tree is empty, it becomes the root leaf.
    /// If the tree has an Empty node, replaces it.
    /// Otherwise, splits the current root to accommodate the new pane.
    pub fn add_pane(&mut self, title: &str, background: [f32; 4]) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let pane = Pane::new(id, title, background);
        self.panes.push(pane);

        // Find the first Empty node and replace it, or set as root
        if matches!(self.root, LayoutNode::Empty) {
            self.root = LayoutNode::Leaf { pane_id: id };
        } else if self.replace_first_empty(id) {
            // success
        } else {
            // No empty node — split the root vertically to make room
            let old_root = std::mem::replace(&mut self.root, LayoutNode::Empty);
            self.root = LayoutNode::Split {
                dir: SplitDirection::Horizontal,
                ratio: 0.5,
                left: Box::new(old_root),
                right: Box::new(LayoutNode::Leaf { pane_id: id }),
            };
        }
        id
    }

    fn replace_first_empty(&mut self, pane_id: u64) -> bool {
        PaneTree::replace_first_empty_node(&mut self.root, pane_id)
    }

    fn replace_first_empty_node(node: &mut LayoutNode, pane_id: u64) -> bool {
        match node {
            LayoutNode::Split { left, right, .. } => {
                Self::replace_first_empty_node(left, pane_id)
                    || Self::replace_first_empty_node(right, pane_id)
            }
            LayoutNode::Empty => {
                *node = LayoutNode::Leaf { pane_id };
                true
            }
            LayoutNode::Leaf { .. } => false,
        }
    }

    /// Split a pane: replace the leaf with a Split containing the original
    /// pane on the left and a new empty pane on the right.
    pub fn split(&mut self, pane_id: u64, dir: SplitDirection) -> Option<u64> {
        let new_id = self.next_id;
        self.next_id += 1;

        let replacement = LayoutNode::Split {
            dir,
            ratio: 0.5,
            left: Box::new(LayoutNode::Leaf { pane_id }),
            right: Box::new(LayoutNode::Leaf { pane_id: new_id }),
        };

        if self.root.replace_leaf(pane_id, replacement) {
            let pane = Pane::new(
                new_id,
                &format!("Pane {}", new_id),
                [0.15, 0.15, 0.20, 1.0], // slight darker tint
            );
            self.panes.push(pane);
            Some(new_id)
        } else {
            None
        }
    }

    /// Remove a pane from the tree (mark as Empty).
    /// If a Split ends up with empty children, collapse it.
    pub fn remove(&mut self, pane_id: u64) {
        // Set disposition to Hidden
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
            pane.disposition = Disposition::Hidden;
        }
        // Replace leaf with Empty
        self.root.replace_leaf(pane_id, LayoutNode::Empty);
        // Remove from floats if present
        self.floats.retain(|(id, _)| *id != pane_id);
    }

    /// Toggle a pane between Embedded and Floating.
    pub fn toggle_float(&mut self, pane_id: u64, default_rect: Option<Rect>) {
        let pane_idx = self.panes.iter().position(|p| p.id == pane_id);
        if let Some(idx) = pane_idx {
            let is_embedded = matches!(self.panes[idx].disposition, Disposition::Embedded);
            let is_floating = matches!(self.panes[idx].disposition, Disposition::Floating);

            if is_embedded {
                self.panes[idx].disposition = Disposition::Floating;
                let rect = default_rect.unwrap_or(Rect::new(100.0, 100.0, 400.0, 300.0));
                self.floats.push((pane_id, rect));
                self.root.replace_leaf(pane_id, LayoutNode::Empty);
                PaneTree::collapse_tree_node(&mut self.root);
            } else if is_floating {
                self.panes[idx].disposition = Disposition::Embedded;
                self.floats.retain(|(id, _)| *id != pane_id);
                if !self.replace_first_empty(pane_id) {
                    let old_root = std::mem::replace(&mut self.root, LayoutNode::Empty);
                    self.root = LayoutNode::Split {
                        dir: SplitDirection::Horizontal,
                        ratio: 0.5,
                        left: Box::new(old_root),
                        right: Box::new(LayoutNode::Leaf { pane_id }),
                    };
                }
            }
        }
    }

    /// Toggle a pane's scratchpad state: if visible (floating), hide it;
    /// if hidden/scratchpad, show it as floating.
    pub fn toggle_scratchpad(&mut self, pane_id: u64) {
        let pane_idx = self.panes.iter().position(|p| p.id == pane_id);
        if let Some(idx) = pane_idx {
            let is_hidden = matches!(self.panes[idx].disposition, Disposition::Scratchpad | Disposition::Hidden);

            if !is_hidden {
                self.panes[idx].disposition = Disposition::Scratchpad;
                self.floats.retain(|(id, _)| *id != pane_id);
                self.root.replace_leaf(pane_id, LayoutNode::Empty);
                PaneTree::collapse_tree_node(&mut self.root);
            } else {
                self.panes[idx].disposition = Disposition::Floating;
                let rect = Rect::new(200.0, 150.0, 400.0, 300.0);
                self.floats.push((pane_id, rect));
            }
        }
    }

    /// Hide a pane without removing it.
    pub fn hide(&mut self, pane_id: u64) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
            pane.disposition = Disposition::Hidden;
            self.floats.retain(|(id, _)| *id != pane_id);
            self.root.replace_leaf(pane_id, LayoutNode::Empty);
            PaneTree::collapse_tree_node(&mut self.root);
        }
    }

    /// Show a hidden pane (back to embedded).
    pub fn show(&mut self, pane_id: u64) {
        if let Some(pane) = self.panes.iter_mut().find(|p| p.id == pane_id) {
            if pane.disposition == Disposition::Hidden {
                pane.disposition = Disposition::Embedded;
                if !self.replace_first_empty(pane_id) {
                    let old_root = std::mem::replace(&mut self.root, LayoutNode::Empty);
                    self.root = LayoutNode::Split {
                        dir: SplitDirection::Horizontal,
                        ratio: 0.5,
                        left: Box::new(old_root),
                        right: Box::new(LayoutNode::Leaf { pane_id }),
                    };
                }
            }
        }
    }

    /// Find the neighbor of a pane in a given direction.
    /// Returns the pane_id of the neighbor, if found.
    pub fn find_neighbor(&self, pane_id: u64, dir: SplitDirection) -> Option<u64> {
        PaneTree::find_neighbor_inner(&self.root, pane_id, dir, None)
    }

    fn find_neighbor_inner(
        node: &LayoutNode,
        pane_id: u64,
        dir: SplitDirection,
        fallback: Option<u64>,
    ) -> Option<u64> {
        match node {
            LayoutNode::Split {
                dir: split_dir,
                left,
                right,
                ..
            } => {
                if left.contains(pane_id) {
                    if *split_dir == dir {
                        Some(PaneTree::leftmost_leaf(right))
                    } else {
                        PaneTree::find_neighbor_inner(left, pane_id, dir, fallback)
                    }
                } else if right.contains(pane_id) {
                    if *split_dir == dir {
                        Some(PaneTree::rightmost_leaf(left))
                    } else {
                        PaneTree::find_neighbor_inner(right, pane_id, dir, fallback)
                    }
                } else {
                    fallback
                }
            }
            LayoutNode::Leaf { .. } | LayoutNode::Empty => fallback,
        }
    }

    fn leftmost_leaf(node: &LayoutNode) -> u64 {
        match node {
            LayoutNode::Split { left, .. } => PaneTree::leftmost_leaf(left),
            LayoutNode::Leaf { pane_id } => *pane_id,
            LayoutNode::Empty => 0,
        }
    }

    fn rightmost_leaf(node: &LayoutNode) -> u64 {
        match node {
            LayoutNode::Split { right, .. } => PaneTree::rightmost_leaf(right),
            LayoutNode::Leaf { pane_id } => *pane_id,
            LayoutNode::Empty => 0,
        }
    }

    /// Swap two pane IDs in the tree leaves.
    pub fn swap_panes(&mut self, a_id: u64, b_id: u64) {
        PaneTree::swap_in_node(&mut self.root, a_id, b_id);
    }

    fn swap_in_node(node: &mut LayoutNode, a_id: u64, b_id: u64) {
        match node {
            LayoutNode::Split { left, right, .. } => {
                PaneTree::swap_in_node(left, a_id, b_id);
                PaneTree::swap_in_node(right, a_id, b_id);
            }
            LayoutNode::Leaf { pane_id } if *pane_id == a_id => {
                *pane_id = b_id;
            }
            LayoutNode::Leaf { pane_id } if *pane_id == b_id => {
                *pane_id = a_id;
            }
            _ => {}
        }
    }

    /// Resize a split: adjust the ratio of the split containing the given pane.
    /// delta is a signed amount to add to the ratio (e.g., 0.05).
    pub fn resize(&mut self, pane_id: u64, delta: f32) {
        PaneTree::resize_in_node(&mut self.root, pane_id, delta);
    }

    fn resize_in_node(node: &mut LayoutNode, pane_id: u64, delta: f32) {
        match node {
            LayoutNode::Split {
                left,
                right,
                ratio,
                ..
            } if left.contains(pane_id) || right.contains(pane_id) => {
                if left.contains(pane_id) {
                    *ratio = (*ratio + delta).clamp(0.15, 0.85);
                } else {
                    *ratio = (*ratio - delta).clamp(0.15, 0.85);
                }
                PaneTree::resize_in_node(left, pane_id, delta);
                PaneTree::resize_in_node(right, pane_id, delta);
            }
            LayoutNode::Split { left, right, .. } => {
                PaneTree::resize_in_node(left, pane_id, delta);
                PaneTree::resize_in_node(right, pane_id, delta);
            }
            _ => {}
        }
    }

    /// Collapse empty Split nodes (a Split with both children Empty becomes Empty).
    fn collapse_tree_node(node: &mut LayoutNode) {
        match node {
            LayoutNode::Split { left, right, .. } => {
                PaneTree::collapse_tree_node(left);
                PaneTree::collapse_tree_node(right);
                if matches!(left.as_ref(), LayoutNode::Empty) && matches!(right.as_ref(), LayoutNode::Empty) {
                    *node = LayoutNode::Empty;
                }
            }
            _ => {}
        }
    }

    // ── Layout Computation ─────────────────────────────────────

    /// Compute pixel rectangles for all visible panes.
    /// Returns (embedded_panes, floating_panes).
    pub fn compute_rects(&self, width: f32, height: f32) -> (Vec<(Rect, &Pane)>, Vec<(&Pane, Rect)>) {
        let mut embedded = Vec::new();
        self.collect_leaf_rects(&self.root, Rect::new(0.0, 0.0, width, height), &mut embedded);

        let mut scratchpad_visible: Vec<u64> = Vec::new();
        let floats: Vec<_> = self
            .floats
            .iter()
            .filter_map(|(id, rect)| {
                self.panes
                    .iter()
                    .find(|p| p.id == *id && (p.disposition == Disposition::Floating))
                    .map(|p| (p, *rect))
            })
            .collect();

        // Check for scratchpad panes that should show as floating
        for pane in &self.panes {
            if pane.disposition == Disposition::Floating && !self.floats.iter().any(|(id, _)| *id == pane.id) {
                scratchpad_visible.push(pane.id);
            }
        }

        (embedded, floats)
    }

    fn collect_leaf_rects<'a>(
        &'a self,
        node: &LayoutNode,
        rect: Rect,
        out: &mut Vec<(Rect, &'a Pane)>,
    ) {
        match node {
            LayoutNode::Split {
                dir,
                ratio,
                left,
                right,
            } => {
                let (left_rect, right_rect) = match dir {
                    SplitDirection::Horizontal => {
                        let split_x = rect.x + rect.w * ratio;
                        (
                            Rect::new(rect.x, rect.y, split_x - rect.x, rect.h),
                            Rect::new(split_x, rect.y, rect.x + rect.w - split_x, rect.h),
                        )
                    }
                    SplitDirection::Vertical => {
                        let split_y = rect.y + rect.h * ratio;
                        (
                            Rect::new(rect.x, rect.y, rect.w, split_y - rect.y),
                            Rect::new(rect.x, split_y, rect.w, rect.y + rect.h - split_y),
                        )
                    }
                };
                self.collect_leaf_rects(left, left_rect, out);
                self.collect_leaf_rects(right, right_rect, out);
            }
            LayoutNode::Leaf { pane_id } => {
                if let Some(pane) = self.panes.iter().find(|p| p.id == *pane_id) {
                    if pane.disposition == Disposition::Embedded {
                        out.push((rect, pane));
                    }
                }
            }
            LayoutNode::Empty => {}
        }
    }

    /// Get the pane count for the status bar.
    pub fn visible_pane_count(&self) -> usize {
        self.panes
            .iter()
            .filter(|p| p.disposition == Disposition::Embedded || p.disposition == Disposition::Floating)
            .count()
    }
}
