// nom is available for future complex parsing needs but current implementation
// uses line-by-line parsing for simplicity and PlantUML compatibility.

use crate::ast::sequence::*;
use crate::error::PlantUmlError;

/// Parse a preprocessed sequence diagram body into an AST.
pub fn parse(input: &str) -> Result<SequenceDiagram, PlantUmlError> {
    let mut elements = Vec::new();
    let mut title = None;
    let mut line_num = 0;

    let mut lines_iter = input.lines().peekable();
    while let Some(line) = lines_iter.next() {
        line_num += 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Title
        if let Some(t) = trimmed.strip_prefix("title ").or_else(|| trimmed.strip_prefix("title\t"))
        {
            title = Some(t.trim().to_string());
            continue;
        }

        // Multi-line note
        if let Some(note) = try_parse_multiline_note(trimmed, &mut lines_iter) {
            elements.push(SequenceElement::Note(note));
            continue;
        }

        // Group start
        if let Some(group) = try_parse_group_start(trimmed) {
            let group = parse_group_body(group, &mut lines_iter)?;
            elements.push(SequenceElement::Group(group));
            continue;
        }

        if let Some(element) = parse_line(trimmed, line_num)? {
            elements.push(element);
        }
    }

    Ok(SequenceDiagram { title, elements })
}

fn parse_line(line: &str, _line_num: usize) -> Result<Option<SequenceElement>, PlantUmlError> {
    // autonumber
    if line.eq_ignore_ascii_case("autonumber") {
        return Ok(Some(SequenceElement::AutoNumber(AutoNumberConfig {
            start: None,
        })));
    }
    if let Some(rest) = strip_prefix_ci(line, "autonumber ") {
        let n = rest.trim().parse::<u32>().ok();
        return Ok(Some(SequenceElement::AutoNumber(AutoNumberConfig {
            start: n,
        })));
    }

    // Separator: == text ==
    if line.starts_with("==") && line.ends_with("==") && line.len() > 4 {
        let label = line[2..line.len() - 2].trim().to_string();
        return Ok(Some(SequenceElement::Separator(Separator { label })));
    }

    // Delay: ...text... or ...
    if line.starts_with("...") {
        let label = if line.ends_with("...") && line.len() > 3 {
            let inner = line[3..line.len() - 3].trim();
            if inner.is_empty() {
                None
            } else {
                Some(inner.to_string())
            }
        } else {
            None
        };
        return Ok(Some(SequenceElement::Delay(label)));
    }

    // Space: |||  or ||45||
    if line.starts_with("||") {
        if line == "|||" {
            return Ok(Some(SequenceElement::Space(None)));
        }
        if line.starts_with("||") && line.ends_with("||") && line.len() > 4 {
            let inner = &line[2..line.len() - 2];
            let n = inner.trim().parse::<u32>().ok();
            return Ok(Some(SequenceElement::Space(n)));
        }
    }

    // Activate / Deactivate
    if let Some(name) = strip_prefix_ci(line, "activate ") {
        return Ok(Some(SequenceElement::Activate(name.trim().to_string())));
    }
    if let Some(name) = strip_prefix_ci(line, "deactivate ") {
        return Ok(Some(SequenceElement::Deactivate(name.trim().to_string())));
    }

    // Single-line note
    if let Some(note) = try_parse_single_line_note(line) {
        return Ok(Some(SequenceElement::Note(note)));
    }

    // Message — tried BEFORE participant declarations because keywords like
    // "database" can appear at the start of a message line (e.g. "Database --> Server").
    if let Some(msg) = try_parse_message(line) {
        return Ok(Some(SequenceElement::Message(msg)));
    }

    // Participant declarations
    if let Some(p) = try_parse_participant_decl(line) {
        return Ok(Some(SequenceElement::ParticipantDecl(p)));
    }

    // Unknown line — skip silently for forward compatibility
    Ok(None)
}

// --- Participant parsing ---

fn try_parse_participant_decl(line: &str) -> Option<Participant> {
    let kinds = [
        ("participant ", ParticipantKind::Participant),
        ("actor ", ParticipantKind::Actor),
        ("boundary ", ParticipantKind::Boundary),
        ("control ", ParticipantKind::Control),
        ("entity ", ParticipantKind::Entity),
        ("database ", ParticipantKind::Database),
        ("collections ", ParticipantKind::Collections),
        ("queue ", ParticipantKind::Queue),
    ];

    for (prefix, kind) in &kinds {
        if let Some(rest) = strip_prefix_ci(line, prefix) {
            let rest = rest.trim();
            return Some(parse_participant_name_label(rest, *kind));
        }
    }
    None
}

fn parse_participant_name_label(input: &str, kind: ParticipantKind) -> Participant {
    // Pattern: "Long Name" as alias
    if let Some(stripped) = input.strip_prefix('"') {
        if let Some(end_quote) = stripped.find('"') {
            let label = stripped[..end_quote].to_string();
            let after = stripped[end_quote + 1..].trim();
            if let Some(alias) = strip_prefix_ci(after, "as ") {
                return Participant {
                    name: alias.trim().to_string(),
                    label: Some(label),
                    kind,
                };
            }
            return Participant {
                name: label.clone(),
                label: Some(label),
                kind,
            };
        }
    }

    // Pattern: Name as "Label" or Name as Label
    if let Some(as_pos) = find_ci(input, " as ") {
        let name = input[..as_pos].trim().to_string();
        let label = input[as_pos + 4..].trim().trim_matches('"').to_string();
        return Participant {
            name,
            label: Some(label),
            kind,
        };
    }

    // Simple name
    let name = input.split_whitespace().next().unwrap_or(input);
    Participant {
        name: name.to_string(),
        label: None,
        kind,
    }
}

// --- Message parsing ---

fn try_parse_message(line: &str) -> Option<Message> {
    // Find arrow pattern in line
    let arrow_patterns: &[(&str, ArrowStyle, bool)] = &[
        // solid filled (left)
        (
            "<-",
            ArrowStyle {
                line: LineStyle::Solid,
                head: ArrowHead::Filled,
            },
            true,
        ),
        // dashed filled (left)
        (
            "<--",
            ArrowStyle {
                line: LineStyle::Dashed,
                head: ArrowHead::Filled,
            },
            true,
        ),
        // dashed open (right) — must check before ->
        (
            "-->>",
            ArrowStyle {
                line: LineStyle::Dashed,
                head: ArrowHead::Open,
            },
            false,
        ),
        // dashed filled (right) — must check before ->
        (
            "-->",
            ArrowStyle {
                line: LineStyle::Dashed,
                head: ArrowHead::Filled,
            },
            false,
        ),
        // solid open (left)
        (
            "<<-",
            ArrowStyle {
                line: LineStyle::Solid,
                head: ArrowHead::Open,
            },
            true,
        ),
        // dashed open (left)
        (
            "<<--",
            ArrowStyle {
                line: LineStyle::Dashed,
                head: ArrowHead::Open,
            },
            true,
        ),
        // solid open (right)
        (
            "->>",
            ArrowStyle {
                line: LineStyle::Solid,
                head: ArrowHead::Open,
            },
            false,
        ),
        // solid filled (right)
        (
            "->",
            ArrowStyle {
                line: LineStyle::Solid,
                head: ArrowHead::Filled,
            },
            false,
        ),
    ];

    // Try longest patterns first to avoid partial matches
    // Sort by arrow pattern length descending
    let mut sorted: Vec<_> = arrow_patterns.iter().collect();
    sorted.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    for (pattern, style, is_left) in sorted {
        if let Some(pos) = line.find(pattern) {
            let (from_part, to_part, label) = if *is_left {
                // <-- means to is on the left, from is on the right
                let left = line[..pos].trim();
                let right_and_label = line[pos + pattern.len()..].trim();
                let (right, label) = split_label(right_and_label);
                (right, left, label)
            } else {
                let left = line[..pos].trim();
                let right_and_label = line[pos + pattern.len()..].trim();
                let (right, label) = split_label(right_and_label);
                (left, right, label)
            };

            let from = clean_participant_name(from_part);
            let to = clean_participant_name(to_part);

            if from.is_empty() || to.is_empty() {
                continue;
            }

            let is_self = from == to;

            return Some(Message {
                from,
                to,
                label: label.unwrap_or_default(),
                arrow: *style,
                is_self_referencing: is_self,
            });
        }
    }

    None
}

fn split_label(input: &str) -> (&str, Option<String>) {
    if let Some(colon_pos) = input.find(':') {
        let name = input[..colon_pos].trim();
        let label = input[colon_pos + 1..].trim().to_string();
        (name, Some(label))
    } else {
        (input, None)
    }
}

fn clean_participant_name(name: &str) -> String {
    name.trim().trim_matches('"').to_string()
}

// --- Note parsing ---

fn try_parse_single_line_note(line: &str) -> Option<Note> {
    // note left of X : text
    // note right of X : text
    // note over X : text
    // note over X, Y : text

    if let Some(rest) = strip_prefix_ci(line, "note left of ") {
        if let Some(colon_pos) = rest.find(':') {
            let target = rest[..colon_pos].trim().to_string();
            let text = rest[colon_pos + 1..].trim().to_string();
            return Some(Note {
                position: NotePosition::LeftOf(target),
                text,
            });
        }
    }

    if let Some(rest) = strip_prefix_ci(line, "note right of ") {
        if let Some(colon_pos) = rest.find(':') {
            let target = rest[..colon_pos].trim().to_string();
            let text = rest[colon_pos + 1..].trim().to_string();
            return Some(Note {
                position: NotePosition::RightOf(target),
                text,
            });
        }
    }

    if let Some(rest) = strip_prefix_ci(line, "note over ") {
        if let Some(colon_pos) = rest.find(':') {
            let targets_str = rest[..colon_pos].trim();
            let targets: Vec<String> = targets_str
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let text = rest[colon_pos + 1..].trim().to_string();
            return Some(Note {
                position: NotePosition::Over(targets),
                text,
            });
        }
    }

    None
}

fn try_parse_multiline_note<'a, I>(first_line: &str, lines: &mut std::iter::Peekable<I>) -> Option<Note>
where
    I: Iterator<Item = &'a str>,
{
    let position = if let Some(rest) = strip_prefix_ci(first_line, "note left of ") {
        Some(NotePosition::LeftOf(rest.trim().to_string()))
    } else if let Some(rest) = strip_prefix_ci(first_line, "note right of ") {
        Some(NotePosition::RightOf(rest.trim().to_string()))
    } else if let Some(rest) = strip_prefix_ci(first_line, "note over ") {
        let targets: Vec<String> = rest.trim().split(',').map(|s| s.trim().to_string()).collect();
        Some(NotePosition::Over(targets))
    } else {
        None
    };

    let position = position?;

    // Check that there's no colon (single-line note handled elsewhere)
    if first_line.contains(':') {
        return None;
    }

    // Collect lines until "end note"
    let mut text_lines = Vec::new();
    for line in lines.by_ref() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("end note") || trimmed.eq_ignore_ascii_case("endnote") {
            break;
        }
        text_lines.push(trimmed);
    }

    Some(Note {
        position,
        text: text_lines.join("\n"),
    })
}

// --- Group parsing ---

fn try_parse_group_start(line: &str) -> Option<Group> {
    let group_kinds = [
        ("alt ", GroupKind::Alt),
        ("loop ", GroupKind::Loop),
        ("opt ", GroupKind::Opt),
        ("break ", GroupKind::Break),
        ("par ", GroupKind::Par),
        ("critical ", GroupKind::Critical),
        ("group ", GroupKind::Group),
    ];

    for (prefix, kind) in &group_kinds {
        if let Some(rest) = strip_prefix_ci(line, prefix) {
            return Some(Group {
                kind: *kind,
                label: rest.trim().to_string(),
                elements: Vec::new(),
                else_blocks: Vec::new(),
            });
        }
    }

    // alt/loop/opt/etc without label
    let no_label_kinds = [
        ("alt", GroupKind::Alt),
        ("loop", GroupKind::Loop),
        ("opt", GroupKind::Opt),
        ("break", GroupKind::Break),
        ("par", GroupKind::Par),
        ("critical", GroupKind::Critical),
        ("group", GroupKind::Group),
    ];

    for (keyword, kind) in &no_label_kinds {
        if line.eq_ignore_ascii_case(keyword) {
            return Some(Group {
                kind: *kind,
                label: String::new(),
                elements: Vec::new(),
                else_blocks: Vec::new(),
            });
        }
    }

    None
}

fn parse_group_body<'a, I>(
    mut group: Group,
    lines: &mut std::iter::Peekable<I>,
) -> Result<Group, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let mut current_elements: Vec<SequenceElement> = Vec::new();
    let mut in_else = false;
    let mut else_label = String::new();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();

        if trimmed.eq_ignore_ascii_case("end") {
            break;
        }

        // else block
        if trimmed.eq_ignore_ascii_case("else") || strip_prefix_ci(trimmed, "else ").is_some() {
            if in_else {
                group.else_blocks.push(ElseBlock {
                    label: else_label.clone(),
                    elements: std::mem::take(&mut current_elements),
                });
            } else {
                group.elements = std::mem::take(&mut current_elements);
                in_else = true;
            }
            else_label = strip_prefix_ci(trimmed, "else ")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            continue;
        }

        // Nested group
        if let Some(nested) = try_parse_group_start(trimmed) {
            let nested = parse_group_body(nested, lines)?;
            current_elements.push(SequenceElement::Group(nested));
            continue;
        }

        // Multi-line note
        // TODO: handle multiline notes inside groups
        if let Some(element) = parse_line(trimmed, 0)? {
            current_elements.push(element);
        }
    }

    if in_else {
        group.else_blocks.push(ElseBlock {
            label: else_label,
            elements: current_elements,
        });
    } else {
        group.elements = current_elements;
    }

    Ok(group)
}

// --- Helper functions ---

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len()
        && s[..prefix.len()].eq_ignore_ascii_case(prefix)
    {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    let h = haystack.to_lowercase();
    let n = needle.to_lowercase();
    h.find(&n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_message() {
        let input = "Alice -> Bob : Hello";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.elements.len(), 1);
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.from, "Alice");
                assert_eq!(msg.to, "Bob");
                assert_eq!(msg.label, "Hello");
                assert_eq!(msg.arrow.line, LineStyle::Solid);
                assert_eq!(msg.arrow.head, ArrowHead::Filled);
                assert!(!msg.is_self_referencing);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_dashed_arrow() {
        let input = "Alice --> Bob : Response";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.arrow.line, LineStyle::Dashed);
                assert_eq!(msg.arrow.head, ArrowHead::Filled);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_open_arrow() {
        let input = "Alice ->> Bob : Async";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.arrow.line, LineStyle::Solid);
                assert_eq!(msg.arrow.head, ArrowHead::Open);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_left_arrow() {
        let input = "Alice <-- Bob : Response";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.from, "Bob");
                assert_eq!(msg.to, "Alice");
                assert_eq!(msg.arrow.line, LineStyle::Dashed);
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_participant_declaration() {
        let input = "participant Alice";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::ParticipantDecl(p) => {
                assert_eq!(p.name, "Alice");
                assert_eq!(p.kind, ParticipantKind::Participant);
                assert!(p.label.is_none());
            }
            _ => panic!("expected ParticipantDecl"),
        }
    }

    #[test]
    fn test_parse_participant_with_alias() {
        let input = r#"participant "Alice Johnson" as alice"#;
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::ParticipantDecl(p) => {
                assert_eq!(p.name, "alice");
                assert_eq!(p.label.as_deref(), Some("Alice Johnson"));
            }
            _ => panic!("expected ParticipantDecl"),
        }
    }

    #[test]
    fn test_parse_actor() {
        let input = "actor Bob";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::ParticipantDecl(p) => {
                assert_eq!(p.name, "Bob");
                assert_eq!(p.kind, ParticipantKind::Actor);
            }
            _ => panic!("expected ParticipantDecl"),
        }
    }

    #[test]
    fn test_parse_title() {
        let input = "title My Sequence Diagram\nAlice -> Bob : Hi";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("My Sequence Diagram"));
    }

    #[test]
    fn test_parse_note_right() {
        let input = "note right of Alice : This is a note";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Note(note) => {
                assert_eq!(note.text, "This is a note");
                match &note.position {
                    NotePosition::RightOf(name) => assert_eq!(name, "Alice"),
                    _ => panic!("expected RightOf"),
                }
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_parse_note_over() {
        let input = "note over Alice, Bob : Shared note";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Note(note) => {
                assert_eq!(note.text, "Shared note");
                match &note.position {
                    NotePosition::Over(names) => {
                        assert_eq!(names, &["Alice", "Bob"]);
                    }
                    _ => panic!("expected Over"),
                }
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_parse_alt_group() {
        let input = "alt success\nAlice -> Bob : OK\nelse failure\nAlice -> Bob : Error\nend";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Group(group) => {
                assert_eq!(group.kind, GroupKind::Alt);
                assert_eq!(group.label, "success");
                assert_eq!(group.elements.len(), 1);
                assert_eq!(group.else_blocks.len(), 1);
                assert_eq!(group.else_blocks[0].label, "failure");
                assert_eq!(group.else_blocks[0].elements.len(), 1);
            }
            _ => panic!("expected Group"),
        }
    }

    #[test]
    fn test_parse_loop_group() {
        let input = "loop every 5s\nAlice -> Bob : ping\nend";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Group(group) => {
                assert_eq!(group.kind, GroupKind::Loop);
                assert_eq!(group.label, "every 5s");
                assert_eq!(group.elements.len(), 1);
            }
            _ => panic!("expected Group"),
        }
    }

    #[test]
    fn test_parse_separator() {
        let input = "== Initialization ==";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Separator(sep) => {
                assert_eq!(sep.label, "Initialization");
            }
            _ => panic!("expected Separator"),
        }
    }

    #[test]
    fn test_parse_activate_deactivate() {
        let input = "activate Bob\ndeactivate Bob";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.elements.len(), 2);
        match &diagram.elements[0] {
            SequenceElement::Activate(name) => assert_eq!(name, "Bob"),
            _ => panic!("expected Activate"),
        }
        match &diagram.elements[1] {
            SequenceElement::Deactivate(name) => assert_eq!(name, "Bob"),
            _ => panic!("expected Deactivate"),
        }
    }

    #[test]
    fn test_parse_autonumber() {
        let input = "autonumber";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::AutoNumber(config) => {
                assert!(config.start.is_none());
            }
            _ => panic!("expected AutoNumber"),
        }
    }

    #[test]
    fn test_parse_autonumber_with_start() {
        let input = "autonumber 10";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::AutoNumber(config) => {
                assert_eq!(config.start, Some(10));
            }
            _ => panic!("expected AutoNumber"),
        }
    }

    #[test]
    fn test_parse_delay() {
        let input = "...waiting...";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Delay(label) => {
                assert_eq!(label.as_deref(), Some("waiting"));
            }
            _ => panic!("expected Delay"),
        }
    }

    #[test]
    fn test_parse_space() {
        let input = "|||";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Space(n) => assert!(n.is_none()),
            _ => panic!("expected Space"),
        }
    }

    #[test]
    fn test_parse_space_with_value() {
        let input = "||45||";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Space(n) => assert_eq!(*n, Some(45)),
            _ => panic!("expected Space"),
        }
    }

    #[test]
    fn test_parse_self_referencing_message() {
        let input = "Alice -> Alice : Think";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert!(msg.is_self_referencing);
                assert_eq!(msg.from, "Alice");
                assert_eq!(msg.to, "Alice");
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_multiline_note() {
        let input = "note right of Alice\nLine 1\nLine 2\nend note";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Note(note) => {
                assert_eq!(note.text, "Line 1\nLine 2");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_parse_full_diagram() {
        let input = r#"title Authentication Flow
participant Client
participant Server
participant Database

Client -> Server : POST /login
activate Server
Server -> Database : SELECT user
activate Database
Database --> Server : user data
deactivate Database
alt valid credentials
    Server --> Client : 200 OK
else invalid
    Server --> Client : 401 Unauthorized
end
deactivate Server"#;

        let diagram = parse(input).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("Authentication Flow"));
        // 3 participants + 2 messages + activate + message + activate + message + deactivate + group + deactivate
        assert!(diagram.elements.len() >= 10);
    }

    #[test]
    fn test_parse_message_no_label() {
        let input = "Alice -> Bob";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.label, "");
            }
            _ => panic!("expected Message"),
        }
    }

    #[test]
    fn test_parse_database_participant() {
        let input = "database MySQL";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            SequenceElement::ParticipantDecl(p) => {
                assert_eq!(p.kind, ParticipantKind::Database);
                assert_eq!(p.name, "MySQL");
            }
            _ => panic!("expected ParticipantDecl"),
        }
    }

    // Regression: "Database --> Server" was parsed as participant decl for "database" keyword,
    // creating a phantom participant named "-->".
    #[test]
    fn test_database_message_not_parsed_as_participant() {
        let input = "Database --> Server : user data";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.elements.len(), 1);
        match &diagram.elements[0] {
            SequenceElement::Message(msg) => {
                assert_eq!(msg.from, "Database");
                assert_eq!(msg.to, "Server");
                assert_eq!(msg.label, "user data");
                assert_eq!(msg.arrow.line, LineStyle::Dashed);
            }
            other => panic!("expected Message, got {:?}", other),
        }
    }

    // Regression: ensure all participant keyword names work correctly as message senders
    #[test]
    fn test_participant_keyword_names_in_messages() {
        let keywords = [
            "Participant", "Actor", "Boundary", "Control",
            "Entity", "Database", "Collections", "Queue",
        ];
        for keyword in &keywords {
            let input = format!("{} -> Server : test", keyword);
            let diagram = parse(&input).unwrap();
            match &diagram.elements[0] {
                SequenceElement::Message(msg) => {
                    assert_eq!(msg.from, *keyword, "failed for keyword: {}", keyword);
                    assert_eq!(msg.to, "Server");
                }
                other => panic!("expected Message for {}, got {:?}", keyword, other),
            }
        }
    }

    // Regression: full diagram with database participant should have exactly 3 participants
    #[test]
    fn test_no_phantom_participant_in_full_diagram() {
        let input = r#"participant Client
participant Server
database Database

Client -> Server : POST /login
Database --> Server : user data
Server --> Client : 200 OK"#;

        let diagram = parse(input).unwrap();

        let participants: Vec<_> = diagram.elements.iter().filter_map(|e| {
            if let SequenceElement::ParticipantDecl(p) = e { Some(p.name.clone()) } else { None }
        }).collect();
        assert_eq!(participants, vec!["Client", "Server", "Database"]);

        let messages: Vec<_> = diagram.elements.iter().filter_map(|e| {
            if let SequenceElement::Message(m) = e { Some(m.label.clone()) } else { None }
        }).collect();
        assert_eq!(messages, vec!["POST /login", "user data", "200 OK"]);
    }
}
