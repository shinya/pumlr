/// AST for state diagrams.

#[derive(Debug, Clone)]
pub struct StateDiagram {
    pub title: Option<String>,
    /// All states, flat. Nesting is expressed via `parent`.
    pub states: Vec<StateDef>,
    pub transitions: Vec<Transition>,
}

#[derive(Debug, Clone)]
pub struct StateDef {
    pub name: String,
    pub display_name: String,
    /// Description lines (`Idle : waiting for job`).
    pub descriptions: Vec<String>,
    /// Name of the enclosing composite state, if any.
    pub parent: Option<String>,
    pub color: Option<String>,
    /// True when declared with a `{ ... }` body (composite), even if empty.
    pub composite: bool,
}

/// Pseudo-state endpoint names used in `Transition::from/to`:
/// `[*]` inside scope `S` becomes `start$S` / `end$S` (top level: `start$` /
/// `end$`), depending on which side of the arrow it appears.
pub const START_PREFIX: &str = "start$";
pub const END_PREFIX: &str = "end$";

#[derive(Debug, Clone)]
pub struct Transition {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    /// Line length in dashes; 1 keeps both states on the same rank.
    pub rank_len: usize,
}

impl StateDiagram {
    pub fn state(&self, name: &str) -> Option<&StateDef> {
        self.states.iter().find(|s| s.name == name)
    }

    pub fn children_of(&self, parent: Option<&str>) -> Vec<&StateDef> {
        self.states
            .iter()
            .filter(|s| s.parent.as_deref() == parent)
            .collect()
    }
}
