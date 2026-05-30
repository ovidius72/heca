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

#[allow(dead_code)]
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
                let rect = default_rect.unwrap_or_else(|| {
                    // Cascade: offset by 40px per existing float to avoid overlap
                    let offset = self.floats.len() as f32 * 40.0;
                    Rect::new(100.0 + offset, 100.0 + offset, 400.0, 300.0)
                });
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

    /// Move a float to the end of the vec so it renders on top.
    pub fn bring_float_to_front(&mut self, pane_id: u64) {
        if let Some(pos) = self.floats.iter().position(|(id, _)| *id == pane_id) {
            let entry = self.floats.remove(pos);
            self.floats.push(entry);
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

    /// Check if a pane is the left/top (first) child of its immediate parent split.
    pub fn is_first_child(&self, pane_id: u64) -> Option<bool> {
        PaneTree::is_first_child_in_node(&self.root, pane_id)
    }

    fn is_first_child_in_node(node: &LayoutNode, pane_id: u64) -> Option<bool> {
        match node {
            LayoutNode::Split { left, right, .. } => {
                if matches!(left.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id) {
                    return Some(true);
                }
                if matches!(right.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id) {
                    return Some(false);
                }
                if let Some(result) = Self::is_first_child_in_node(left, pane_id) {
                    return Some(result);
                }
                if let Some(result) = Self::is_first_child_in_node(right, pane_id) {
                    return Some(result);
                }
                None
            }
            _ => None,
        }
    }

    /// Resize the immediate parent split of a pane (single border move).
    /// For keyboard: delta sign depends on action direction.
    pub fn resize(&mut self, pane_id: u64, delta: f32) {
        PaneTree::resize_in_node(&mut self.root, pane_id, delta);
    }

    /// Resize by directly adding delta to ratio (for mouse drag).
    /// Always adds delta — caller must pass correctly-signed delta.
    pub fn resize_delta(&mut self, pane_id: u64, delta: f32) {
        PaneTree::resize_delta_in_node(&mut self.root, pane_id, delta);
    }

    /// Only adjust the ratio at the split where pane_id is a DIRECT child.
    /// For keyboard shortcuts: inverts delta for right-side children.
    fn resize_in_node(node: &mut LayoutNode, pane_id: u64, delta: f32) -> bool {
        match node {
            LayoutNode::Split { left, right, ratio, .. } => {
                if matches!(left.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id) {
                    *ratio = (*ratio + delta).clamp(0.15, 0.85);
                    return true;
                }
                if matches!(right.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id) {
                    *ratio = (*ratio - delta).clamp(0.15, 0.85);
                    return true;
                }
                if Self::resize_in_node(left, pane_id, delta) {
                    return true;
                }
                if Self::resize_in_node(right, pane_id, delta) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// Always adds delta to ratio (no child-side inversion).
    fn resize_delta_in_node(node: &mut LayoutNode, pane_id: u64, delta: f32) -> bool {
        match node {
            LayoutNode::Split { left, right, ratio, .. } => {
                if matches!(left.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id)
                    || matches!(right.as_ref(), LayoutNode::Leaf { pane_id: pid } if *pid == pane_id) {
                    *ratio = (*ratio + delta).clamp(0.15, 0.85);
                    return true;
                }
                if Self::resize_delta_in_node(left, pane_id, delta) {
                    return true;
                }
                if Self::resize_delta_in_node(right, pane_id, delta) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// Check if a pane is visible (embedded or floating).
    pub fn is_visible(&self, pane_id: u64) -> bool {
        self.panes.iter().any(|p| {
            p.id == pane_id && (p.disposition == Disposition::Embedded || p.disposition == Disposition::Floating)
        })
    }

    /// Geometric neighbor detection: find the best pane in the given direction.
    /// Searches both embedded panes and floats. Uses rectangle overlap for scoring.
    pub fn find_neighbor_geo(&self, pane_id: u64, dir: SplitDirection, width: f32, height: f32, preferred: Option<u64>) -> Option<u64> {
        let (embedded, floats) = self.compute_rects(width, height);

        // Find current pane's rect (search embedded first, then floats)
        let current_rect = embedded.iter()
            .find(|(_, p)| p.id == pane_id)
            .map(|(r, _)| *r)
            .or_else(|| floats.iter().find(|(p, _)| p.id == pane_id).map(|(_, r)| *r))?;

        let mut best: Option<(u64, f32, f32)> = None; // (pane_id, overlap, center_dist)

        // Score all candidates (embedded + floats)
        let mut candidates: Vec<(u64, f32, f32)> = Vec::new(); // (pane_id, overlap, dist) for debug

        for (rect, pane) in &embedded {
            if pane.id == pane_id { continue; }
            if let Some(scored) = Self::score_neighbor(dir, &current_rect, rect, pane.id, preferred, &mut best) {
                candidates.push((pane.id, scored.1, scored.2));
            }
        }
        for (pane, rect) in &floats {
            if pane.id == pane_id { continue; }
            if let Some(scored) = Self::score_neighbor(dir, &current_rect, rect, pane.id, preferred, &mut best) {
                candidates.push((pane.id, scored.1, scored.2));
            }
        }

        let result = best.map(|(id, _, _)| id);

        // Debug: print candidates and result
        let curr_title = self.panes.iter().find(|p| p.id == pane_id).map(|p| p.title.as_str()).unwrap_or("?");
        let dir_str = match dir { SplitDirection::Horizontal => "R", SplitDirection::Vertical => "D" };
        eprint!("[geo] {} ->{} candidates=", curr_title, dir_str);
        for (pid, ov, dist) in candidates {
            let t = self.panes.iter().find(|p| p.id == pid).map(|p| p.title.as_str()).unwrap_or("?");
            eprint!(" {}:ov={:.0},dist={:.0}", t, ov, dist);
        }
        let res_title = result.and_then(|id| self.panes.iter().find(|p| p.id == id).map(|p| p.title.as_str()));
        eprintln!(" => {:?}", res_title);

        result
    }

    fn score_neighbor(
        dir: SplitDirection,
        current_rect: &Rect,
        rect: &Rect,
        pane_id: u64,
        preferred: Option<u64>,
        best: &mut Option<(u64, f32, f32)>,
    ) -> Option<(u64, f32, f32)> {
        let (in_dir, overlap) = match dir {
            SplitDirection::Horizontal => {
                let in_dir = rect.x + rect.w <= current_rect.x + 0.01
                    || rect.x >= current_rect.x + current_rect.w - 0.01;
                let y_ov = (rect.y + rect.h).min(current_rect.y + current_rect.h) - rect.y.max(current_rect.y);
                (in_dir, y_ov.max(0.0))
            }
            SplitDirection::Vertical => {
                let in_dir = rect.y + rect.h <= current_rect.y + 0.01
                    || rect.y >= current_rect.y + current_rect.h - 0.01;
                let x_ov = (rect.x + rect.w).min(current_rect.x + current_rect.w) - rect.x.max(current_rect.x);
                (in_dir, x_ov.max(0.0))
            }
        };

        if !in_dir || overlap <= 0.0 { return None; }

        let center_dist = match dir {
            SplitDirection::Horizontal => {
                let curr_cy = current_rect.y + current_rect.h / 2.0;
                let other_cy = rect.y + rect.h / 2.0;
                (curr_cy - other_cy).abs()
            }
            SplitDirection::Vertical => {
                let curr_cx = current_rect.x + current_rect.w / 2.0;
                let other_cx = rect.x + rect.w / 2.0;
                (curr_cx - other_cx).abs()
            }
        };

        let is_preferred = preferred == Some(pane_id);
        let best_is_preferred = best.map_or(false, |(id, _, _)| preferred == Some(id));

        let better = match *best {
            None => true,
            Some((_, best_ov, best_dist)) => {
                if is_preferred && !best_is_preferred { true }
                else if !is_preferred && best_is_preferred { false }
                else if overlap > best_ov + 0.1 { true }
                else if (overlap - best_ov).abs() < 0.1 && center_dist < best_dist { true }
                else { false }
            }
        };

        if better {
            let result = (pane_id, overlap, center_dist);
            *best = Some(result);
            Some(result)
        } else {
            None
        }
    }

    /// Move a pane by swapping the subtrees at the lowest common ancestor
    /// with a matching direction. Falls back to ID-swap if no structural match.
    pub fn move_pane(&mut self, pane_id: u64, dir: SplitDirection) -> bool {
        if let Some(neighbor) = self.find_neighbor(pane_id, dir) {
            // Try structural subtree swap at LCA with matching direction
            if Self::swap_subtrees_at_lca(&mut self.root, pane_id, neighbor, dir) {
                return true;
            }
            // Fall back to ID swap
            self.swap_panes(pane_id, neighbor);
            true
        } else {
            false
        }
    }

    fn swap_subtrees_at_lca(node: &mut LayoutNode, a: u64, b: u64, dir: SplitDirection) -> bool {
        match node {
            LayoutNode::Split { left, right, dir: split_dir, .. } => {
                let a_in_left = left.contains(a);
                let b_in_left = left.contains(b);
                let a_in_right = right.contains(a);
                let b_in_right = right.contains(b);

                // If a and b are in different children AND the split direction matches,
                // swap the entire child subtrees.
                if *split_dir == dir && ((a_in_left && b_in_right) || (b_in_left && a_in_right)) {
                    std::mem::swap(left, right);
                    return true;
                }

                // Otherwise recurse into the child that contains either a or b
                if (a_in_left || b_in_left) && Self::swap_subtrees_at_lca(left, a, b, dir) {
                    return true;
                }
                if (a_in_right || b_in_right) && Self::swap_subtrees_at_lca(right, a, b, dir) {
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    /// Collapse empty Split nodes. Handles three cases:
    /// - Both children Empty → this node becomes Empty
    /// - One child Empty, other non-Empty → hoist the non-Empty child
    /// - Both non-Empty → nothing to do
    fn collapse_tree_node(node: &mut LayoutNode) {
        match node {
            LayoutNode::Split { left, right, .. } => {
                PaneTree::collapse_tree_node(left);
                PaneTree::collapse_tree_node(right);

                let left_is_empty = matches!(left.as_ref(), LayoutNode::Empty);
                let right_is_empty = matches!(right.as_ref(), LayoutNode::Empty);

                if left_is_empty && right_is_empty {
                    *node = LayoutNode::Empty;
                } else if right_is_empty {
                    // Hoist the non-empty left child up
                    *node = (**left).clone();
                } else if left_is_empty {
                    // Hoist the non-empty right child up
                    *node = (**right).clone();
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

    /// Cycle focus through all visible panes (embedded then floats, in order).
    /// delta = 1 for next, -1 for previous. Wraps around.
    pub fn cycle_focus(&self, current: Option<u64>, delta: i32) -> Option<u64> {
        let mut order: Vec<u64> = Vec::new();

        // Embedded panes in tree order (left-to-right, top-to-bottom)
        self.collect_leaf_ids(&self.root, &mut order);

        // Floating panes in z-order (back-to-front)
        for (id, _) in &self.floats {
            if self.panes.iter().any(|p| p.id == *id && p.disposition == Disposition::Floating) {
                order.push(*id);
            }
        }

        if order.is_empty() {
            return None;
        }

        let current_idx = current
            .and_then(|id| order.iter().position(|&pid| pid == id))
            .unwrap_or(0);

        let len = order.len() as i32;
        let next_idx = ((current_idx as i32 + delta).rem_euclid(len)) as usize;
        Some(order[next_idx])
    }

    fn collect_leaf_ids(&self, node: &LayoutNode, out: &mut Vec<u64>) {
        match node {
            LayoutNode::Split { left, right, .. } => {
                self.collect_leaf_ids(left, out);
                self.collect_leaf_ids(right, out);
            }
            LayoutNode::Leaf { pane_id } => {
                if let Some(pane) = self.panes.iter().find(|p| p.id == *pane_id) {
                    if pane.disposition == Disposition::Embedded {
                        out.push(*pane_id);
                    }
                }
            }
            LayoutNode::Empty => {}
        }
    }
}
