/// Supported diagram types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramType {
    Sequence,
    Class,
    Activity,
    Component,
    State,
    UseCase,
    MindMap,
    Wbs,
    Gantt,
    Json,
    Yaml,
}
