/// AST for component diagrams.

#[derive(Debug, Clone)]
pub struct ComponentDiagram {
    pub title: Option<String>,
    pub components: Vec<CompDef>,
    pub packages: Vec<CompPackage>,
    pub relations: Vec<CompRelation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompKind {
    /// Rectangular component with the component icon.
    Component,
    /// Lollipop interface: small circle with a label below.
    Interface,
}

#[derive(Debug, Clone)]
pub struct CompDef {
    pub name: String,
    pub display_name: String,
    pub kind: CompKind,
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CompPackage {
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CompRelation {
    pub from: String,
    pub to: String,
    pub arrow: bool,
    pub back_arrow: bool,
    pub dashed: bool,
    pub rank_len: usize,
    pub label: Option<String>,
}

impl ComponentDiagram {
    pub fn component(&self, name: &str) -> Option<&CompDef> {
        self.components.iter().find(|c| c.name == name)
    }
}
