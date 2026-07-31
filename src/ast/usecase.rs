/// AST for use case diagrams.

#[derive(Debug, Clone)]
pub struct UseCaseDiagram {
    pub title: Option<String>,
    pub elements: Vec<Element>,
    pub containers: Vec<ContainerDef>,
    pub relations: Vec<UcRelation>,
    pub left_to_right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElementKind {
    Actor,
    UseCase,
}

#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    pub display_name: String,
    pub kind: ElementKind,
    pub color: Option<String>,
}

/// `rectangle Name { ... }` / `package Name { ... }` grouping.
#[derive(Debug, Clone)]
pub struct ContainerDef {
    pub name: String,
    pub members: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UcRelation {
    pub from: String,
    pub to: String,
    /// Arrowhead at the `to` end.
    pub arrow: bool,
    /// Arrowhead at the `from` end.
    pub back_arrow: bool,
    pub dashed: bool,
    pub rank_len: usize,
    pub label: Option<String>,
}

impl UseCaseDiagram {
    pub fn element(&self, name: &str) -> Option<&Element> {
        self.elements.iter().find(|e| e.name == name)
    }
}
