/// AST for tree-shaped diagrams (mind map, WBS).

#[derive(Debug, Clone)]
pub struct TreeDiagram {
    pub title: Option<String>,
    /// Root nodes (usually one).
    pub roots: Vec<TreeNode>,
}

/// Which side of the root a mind-map node is drawn on.
///
/// Assigned by the parser: the `left side` / `right side` directives switch
/// the side for subsequent `*` nodes, while OrgMode `+` / `-` markers force
/// Right / Left respectively. Only the side of a root's direct children
/// matters for layout; deeper nodes follow their branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    Left,
    #[default]
    Right,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub text: String,
    pub color: Option<String>,
    /// Drawn without a surrounding box (`*_ text`).
    pub boxless: bool,
    /// Side of the root this node is drawn on (mind maps only).
    pub side: Side,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(text: String) -> Self {
        Self {
            text,
            color: None,
            boxless: false,
            side: Side::default(),
            children: Vec::new(),
        }
    }
}
