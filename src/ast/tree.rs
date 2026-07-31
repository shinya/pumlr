/// AST for tree-shaped diagrams (mind map, WBS).

#[derive(Debug, Clone)]
pub struct TreeDiagram {
    pub title: Option<String>,
    /// Root nodes (usually one).
    pub roots: Vec<TreeNode>,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub text: String,
    pub color: Option<String>,
    /// Drawn without a surrounding box (`*_ text`).
    pub boxless: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    pub fn new(text: String) -> Self {
        Self {
            text,
            color: None,
            boxless: false,
            children: Vec::new(),
        }
    }
}
