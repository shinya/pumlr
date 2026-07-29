/// A parsed activity diagram (new/beta syntax).
#[derive(Debug, Clone)]
pub struct ActivityDiagram {
    pub title: Option<String>,
    /// `header text` — small gray text at the top right.
    pub header: Option<String>,
    /// `footer text` — small gray text at the bottom center.
    pub footer: Option<String>,
    /// `caption text` — text below the diagram, centered.
    pub caption: Option<String>,
    pub elements: Vec<ActivityElement>,
}

/// An element in an activity diagram.
#[derive(Debug, Clone)]
pub enum ActivityElement {
    Start,
    Stop,
    End,
    Action(Action),
    If(IfBlock),
    While(WhileBlock),
    Fork(ForkBlock),
    Switch(SwitchBlock),
    Partition(Partition),
    Note(ActivityNote),
    Arrow(ArrowLabel),
    Repeat(RepeatBlock),
    Detach,
    /// `|Lane|` — switch the current swimlane.
    LaneChange(String),
}

/// `repeat ... [backward :action;] repeat while (cond) is (label)`
#[derive(Debug, Clone)]
pub struct RepeatBlock {
    pub elements: Vec<ActivityElement>,
    /// `backward :label;` — action drawn on the loop-back rail.
    pub backward: Option<String>,
    pub condition: String,
    pub is_label: String,
}

/// An action node `:text;`
#[derive(Debug, Clone)]
pub struct Action {
    pub label: String,
    pub shape: ActionShape,
    /// Fill override from `:text; <<#Color>>` or the legacy `#Color:text;`.
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionShape {
    /// `:text;` — default rectangle with rounded corners
    Action,
    /// `:text|` — not implemented yet, but reserved
    Condition,
}

/// `if (cond) then (yes) ... else (no) ... endif`
#[derive(Debug, Clone)]
pub struct IfBlock {
    pub condition: String,
    pub then_label: String,
    pub then_elements: Vec<ActivityElement>,
    pub else_label: String,
    pub else_elements: Vec<ActivityElement>,
    pub elseif_blocks: Vec<ElseIfBlock>,
}

#[derive(Debug, Clone)]
pub struct ElseIfBlock {
    pub condition: String,
    pub then_label: String,
    pub elements: Vec<ActivityElement>,
}

/// `while (cond) ... endwhile`
#[derive(Debug, Clone)]
pub struct WhileBlock {
    pub condition: String,
    pub is_label: String,
    pub elements: Vec<ActivityElement>,
    pub end_label: String,
}

/// `fork ... fork again ... end fork`
#[derive(Debug, Clone)]
pub struct ForkBlock {
    pub branches: Vec<Vec<ActivityElement>>,
}

/// `switch (val) / case (a) / case (b) / endswitch`
#[derive(Debug, Clone)]
pub struct SwitchBlock {
    pub condition: String,
    pub cases: Vec<CaseBlock>,
}

#[derive(Debug, Clone)]
pub struct CaseBlock {
    pub label: String,
    pub elements: Vec<ActivityElement>,
}

/// `partition "name" { ... }`
#[derive(Debug, Clone)]
pub struct Partition {
    pub name: String,
    pub elements: Vec<ActivityElement>,
}

/// A note on an activity
#[derive(Debug, Clone)]
pub struct ActivityNote {
    pub position: ActivityNotePosition,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivityNotePosition {
    Left,
    Right,
}

/// Arrow label between elements: `-> text;` or `-->`
#[derive(Debug, Clone)]
pub struct ArrowLabel {
    pub label: String,
}
