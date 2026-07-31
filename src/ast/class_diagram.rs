/// AST for class diagrams.

#[derive(Debug, Clone)]
pub struct ClassDiagram {
    pub title: Option<String>,
    pub classes: Vec<ClassDef>,
    pub packages: Vec<PackageDef>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassKind {
    Class,
    AbstractClass,
    Interface,
    Enum,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    /// Identifier used in relations (alias if `as` was used).
    pub name: String,
    /// Text shown in the header (quoted display name if given).
    pub display_name: String,
    pub kind: ClassKind,
    pub stereotype: Option<String>,
    pub fields: Vec<Member>,
    pub methods: Vec<Member>,
    /// Explicit fill color (`class Foo #LightBlue`).
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Member {
    pub text: String,
    pub visibility: Option<Visibility>,
    pub is_abstract: bool,
    pub is_static: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Private,
    Protected,
    PackagePrivate,
}

/// A `package Name { ... }` grouping; contains the names of member classes.
#[derive(Debug, Clone)]
pub struct PackageDef {
    pub name: String,
    pub classes: Vec<String>,
}

/// Marker drawn at one end of a relation line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndMarker {
    None,
    /// Hollow triangle (extension / realization), e.g. `<|`.
    Triangle,
    /// Hollow diamond (aggregation), `o`.
    Diamond,
    /// Filled diamond (composition), `*`.
    FilledDiamond,
    /// Concave arrowhead (association direction / dependency), `<` / `>`.
    ArrowHead,
}

#[derive(Debug, Clone)]
pub struct Relation {
    /// Entity written on the left of the arrow. Laid out above `right`
    /// (PlantUML/dot rank semantics for `--`-length lines).
    pub left: String,
    pub right: String,
    pub left_marker: EndMarker,
    pub right_marker: EndMarker,
    /// Dotted line (`..`): realization / dependency.
    pub dashed: bool,
    /// Length of the line body (`->` = 1 keeps both on the same rank).
    pub rank_len: usize,
    pub label: Option<String>,
    /// Cardinality written next to the left / right entity (`"1"`, `"0..*"`).
    pub left_card: Option<String>,
    pub right_card: Option<String>,
}
