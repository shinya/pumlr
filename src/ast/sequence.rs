/// A parsed sequence diagram.
#[derive(Debug, Clone)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    /// `hide footbox` — omit the bottom participant row.
    pub hide_footbox: bool,
    /// `box "Title" ... end box` participant groupings.
    pub boxes: Vec<ParticipantBox>,
    /// `header text` — small gray text at the top right.
    pub header: Option<String>,
    /// `footer text` — small gray text at the bottom center.
    pub footer: Option<String>,
    /// `caption text` — text below the diagram, centered.
    pub caption: Option<String>,
    pub elements: Vec<SequenceElement>,
}

/// A `box "Title" #Color ... end box` grouping: a full-height background
/// panel behind its participants.
#[derive(Debug, Clone)]
pub struct ParticipantBox {
    pub title: String,
    pub color: Option<String>,
    /// Names of the participants declared inside the box.
    pub participants: Vec<String>,
}

/// An element in a sequence diagram.
#[derive(Debug, Clone)]
pub enum SequenceElement {
    ParticipantDecl(Participant),
    Message(Message),
    Note(Note),
    Group(Group),
    Separator(Separator),
    Activate(String),
    Deactivate(String),
    AutoNumber(AutoNumberConfig),
    Delay(Option<String>),
    Space(Option<u32>),
    /// `return <label>` — reply to whoever activated the current participant,
    /// closing its activation.
    Return(String),
    /// `ref over A, B : text` — a reference fragment spanning participants.
    RefOver(RefOver),
}

/// A `ref over` fragment.
#[derive(Debug, Clone)]
pub struct RefOver {
    pub participants: Vec<String>,
    pub text: String,
}

/// A participant (actor, boundary, etc.).
#[derive(Debug, Clone)]
pub struct Participant {
    pub name: String,
    pub label: Option<String>,
    pub kind: ParticipantKind,
    /// `participant Foo #Color` fill override.
    pub color: Option<String>,
    /// Declared with `create`: the head box appears at the first message
    /// received instead of in the top row.
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParticipantKind {
    Participant,
    Actor,
    Boundary,
    Control,
    Entity,
    Database,
    Collections,
    Queue,
}

/// A message between participants.
#[derive(Debug, Clone)]
pub struct Message {
    pub from: String,
    pub to: String,
    pub label: String,
    pub arrow: ArrowStyle,
    pub is_self_referencing: bool,
    /// `A -> B ++ : msg` — activate the target at this message.
    pub activate_target: bool,
    /// `A -> B -- : msg` — deactivate the source at this message.
    pub deactivate_source: bool,
    /// `A -[#red]> B` — line/arrowhead color override.
    pub color: Option<String>,
}

/// Arrow style for messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrowStyle {
    pub line: LineStyle,
    pub head: ArrowHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrowHead {
    Filled,
    Open,
}

/// A note attached to a participant or between participants.
#[derive(Debug, Clone)]
pub struct Note {
    pub position: NotePosition,
    pub text: String,
    /// `note right of X #Color : text` fill override.
    pub color: Option<String>,
}

#[derive(Debug, Clone)]
pub enum NotePosition {
    LeftOf(String),
    RightOf(String),
    Over(Vec<String>),
}

/// A group (alt, loop, opt, break, par, critical, group).
#[derive(Debug, Clone)]
pub struct Group {
    pub kind: GroupKind,
    pub label: String,
    pub elements: Vec<SequenceElement>,
    pub else_blocks: Vec<ElseBlock>,
}

#[derive(Debug, Clone)]
pub struct ElseBlock {
    pub label: String,
    pub elements: Vec<SequenceElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    Alt,
    Else,
    Loop,
    Opt,
    Break,
    Par,
    Critical,
    Group,
}

/// A separator line.
#[derive(Debug, Clone)]
pub struct Separator {
    pub label: String,
}

/// Auto-numbering configuration.
#[derive(Debug, Clone)]
pub struct AutoNumberConfig {
    pub start: Option<u32>,
    pub increment: Option<u32>,
    /// Format string like `"[000]"`: the run of `0`s is replaced by the
    /// zero-padded number, other characters are literal.
    pub format: Option<String>,
}
