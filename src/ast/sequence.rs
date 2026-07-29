/// A parsed sequence diagram.
#[derive(Debug, Clone)]
pub struct SequenceDiagram {
    pub title: Option<String>,
    /// `hide footbox` — omit the bottom participant row.
    pub hide_footbox: bool,
    pub elements: Vec<SequenceElement>,
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
}

/// A participant (actor, boundary, etc.).
#[derive(Debug, Clone)]
pub struct Participant {
    pub name: String,
    pub label: Option<String>,
    pub kind: ParticipantKind,
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
}
