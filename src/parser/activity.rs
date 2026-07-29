use crate::ast::activity::*;
use crate::error::PlantUmlError;

/// Parse a preprocessed activity diagram body into an AST.
pub fn parse(input: &str) -> Result<ActivityDiagram, PlantUmlError> {
    let mut lines_iter = input.lines().peekable();
    let mut title = None;

    // Check for title
    if let Some(first_line) = lines_iter.peek() {
        let trimmed = first_line.trim();
        if let Some(t) = strip_prefix_ci(trimmed, "title ") {
            title = Some(t.trim().to_string());
            lines_iter.next();
        }
    }

    let elements = parse_elements(&mut lines_iter, &[])?;

    Ok(ActivityDiagram { title, elements })
}

fn parse_elements<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    stop_keywords: &[&str],
) -> Result<Vec<ActivityElement>, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let mut elements = Vec::new();

    while let Some(line) = lines.peek() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            lines.next();
            continue;
        }

        // Check stop keywords
        let lower = trimmed.to_lowercase();
        if stop_keywords.iter().any(|kw| lower.starts_with(kw)) {
            break;
        }

        lines.next();

        if let Some(el) = parse_single_element(trimmed, lines)? {
            elements.push(el);
        }
    }

    Ok(elements)
}

fn parse_single_element<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<Option<ActivityElement>, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let lower = line.to_lowercase();

    // start / stop / end / detach
    if lower == "start" {
        return Ok(Some(ActivityElement::Start));
    }
    if lower == "stop" {
        return Ok(Some(ActivityElement::Stop));
    }
    if lower == "end" {
        return Ok(Some(ActivityElement::End));
    }
    if lower == "detach" {
        return Ok(Some(ActivityElement::Detach));
    }

    // Action: :text;
    if line.starts_with(':') {
        return Ok(Some(parse_action(line, lines)));
    }

    // if
    if lower.starts_with("if ") || lower.starts_with("if(") {
        return Ok(Some(parse_if(line, lines)?));
    }

    // while
    if lower.starts_with("while ") || lower.starts_with("while(") {
        return Ok(Some(parse_while(line, lines)?));
    }

    // repeat (loop start) — "repeat while" is the loop end, handled inside
    if lower == "repeat" {
        return Ok(Some(parse_repeat(lines)?));
    }

    // fork
    if lower == "fork" {
        return Ok(Some(parse_fork(lines)?));
    }

    // switch
    if lower.starts_with("switch ") || lower.starts_with("switch(") {
        return Ok(Some(parse_switch(line, lines)?));
    }

    // partition
    if lower.starts_with("partition ") {
        return Ok(Some(parse_partition(line, lines)?));
    }

    // note
    if lower.starts_with("note left") || lower.starts_with("note right") {
        return Ok(Some(parse_note(line, lines)));
    }

    // Arrow label: -> text;
    if let Some(rest) = line.strip_prefix("->") {
        let label = rest.trim().trim_end_matches(';').trim().to_string();
        return Ok(Some(ActivityElement::Arrow(ArrowLabel { label })));
    }

    // Unknown — skip
    Ok(None)
}

fn parse_action<'a, I>(line: &str, lines: &mut std::iter::Peekable<I>) -> ActivityElement
where
    I: Iterator<Item = &'a str>,
{
    // :text; on single line
    if let Some(label) = extract_action_label(line) {
        return ActivityElement::Action(Action {
            label,
            shape: ActionShape::Action,
        });
    }

    // Multi-line action: starts with : but no ; on same line
    let mut text = line[1..].to_string();
    for next_line in lines.by_ref() {
        let trimmed = next_line.trim();
        if trimmed.ends_with(';') {
            let part = trimmed.trim_end_matches(';');
            if !text.is_empty() && !part.is_empty() {
                text.push('\n');
            }
            text.push_str(part);
            break;
        }
        text.push('\n');
        text.push_str(trimmed);
    }

    ActivityElement::Action(Action {
        label: text.trim().to_string(),
        shape: ActionShape::Action,
    })
}

fn extract_action_label(line: &str) -> Option<String> {
    if line.starts_with(':') && line.ends_with(';') {
        Some(line[1..line.len() - 1].trim().to_string())
    } else {
        None
    }
}

fn parse_if<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let (condition, then_label) = parse_if_header(line);

    let then_elements = parse_elements(lines, &["else", "elseif", "endif"])?;

    let mut else_label = String::new();
    let mut else_elements = Vec::new();
    let mut elseif_blocks = Vec::new();

    while let Some(next) = lines.peek() {
        let trimmed = next.trim().to_string();
        let lower = trimmed.to_lowercase();

        if lower.starts_with("elseif") || lower.starts_with("else if") {
            lines.next();
            let (cond, label) = parse_elseif_header(&trimmed);
            let elems = parse_elements(lines, &["else", "elseif", "endif"])?;
            elseif_blocks.push(ElseIfBlock {
                condition: cond,
                then_label: label,
                elements: elems,
            });
        } else if lower.starts_with("else") {
            lines.next();
            else_label = extract_paren_content(&trimmed).unwrap_or_default();
            else_elements = parse_elements(lines, &["endif"])?;
        } else if lower.starts_with("endif") {
            lines.next();
            break;
        } else {
            break;
        }
    }

    Ok(ActivityElement::If(IfBlock {
        condition,
        then_label,
        then_elements,
        else_label,
        else_elements,
        elseif_blocks,
    }))
}

fn parse_if_header(line: &str) -> (String, String) {
    // if (condition) then (label)
    let condition = extract_paren_after(line, "if").unwrap_or_default();
    let then_label = if let Some(pos) = line.to_lowercase().find("then") {
        let after_then = &line[pos + 4..];
        extract_paren_content(after_then).unwrap_or_default()
    } else {
        String::new()
    };
    (condition, then_label)
}

fn parse_elseif_header(line: &str) -> (String, String) {
    let condition = extract_first_paren(line).unwrap_or_default();
    let then_label = if let Some(pos) = line.to_lowercase().find("then") {
        let after_then = &line[pos + 4..];
        extract_paren_content(after_then).unwrap_or_default()
    } else {
        String::new()
    };
    (condition, then_label)
}

fn parse_while<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let condition = extract_paren_after(line, "while").unwrap_or_default();
    let is_label = if let Some(pos) = line.to_lowercase().find("is") {
        let after_is = &line[pos + 2..];
        extract_paren_content(after_is).unwrap_or_default()
    } else {
        String::new()
    };

    let elements = parse_elements(lines, &["endwhile"])?;

    let end_label = if let Some(end_line) = lines.next() {
        extract_paren_content(end_line.trim()).unwrap_or_default()
    } else {
        String::new()
    };

    Ok(ActivityElement::While(WhileBlock {
        condition,
        is_label,
        elements,
        end_label,
    }))
}

fn parse_repeat<'a, I>(lines: &mut std::iter::Peekable<I>) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let mut elements = parse_elements(lines, &["repeat while", "backward"])?;

    // Optional `backward :action;` just before the loop end
    let mut backward = None;
    if let Some(next) = lines.peek() {
        let trimmed = next.trim();
        if trimmed.to_lowercase().starts_with("backward") {
            backward = extract_action_label(trimmed["backward".len()..].trim());
            lines.next();
            elements.extend(parse_elements(lines, &["repeat while"])?);
        }
    }

    // `repeat while (cond) is (label)`
    let (condition, is_label) = if let Some(end_line) = lines.next() {
        let condition = extract_paren_after(end_line, "while").unwrap_or_default();
        let is_label = if let Some(pos) = end_line.to_lowercase().rfind("is") {
            extract_paren_content(&end_line[pos + 2..]).unwrap_or_default()
        } else {
            String::new()
        };
        (condition, is_label)
    } else {
        (String::new(), String::new())
    };

    Ok(ActivityElement::Repeat(RepeatBlock {
        elements,
        backward,
        condition,
        is_label,
    }))
}

fn parse_fork<'a, I>(lines: &mut std::iter::Peekable<I>) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let mut branches = Vec::new();

    let first_branch = parse_elements(lines, &["fork again", "end fork", "endfork"])?;
    branches.push(first_branch);

    while let Some(next) = lines.peek() {
        let lower = next.trim().to_lowercase();
        if lower == "fork again" {
            lines.next();
            let branch = parse_elements(lines, &["fork again", "end fork", "endfork"])?;
            branches.push(branch);
        } else if lower == "end fork" || lower == "endfork" {
            lines.next();
            break;
        } else {
            break;
        }
    }

    Ok(ActivityElement::Fork(ForkBlock { branches }))
}

fn parse_switch<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    let condition = extract_paren_after(line, "switch").unwrap_or_default();
    let mut cases = Vec::new();

    while let Some(next) = lines.peek() {
        let trimmed = next.trim().to_string();
        let lower = trimmed.to_lowercase();

        if lower.starts_with("case") {
            lines.next();
            let label = extract_paren_content(&trimmed).unwrap_or_default();
            let elements = parse_elements(lines, &["case", "endswitch"])?;
            cases.push(CaseBlock { label, elements });
        } else if lower.starts_with("endswitch") {
            lines.next();
            break;
        } else {
            lines.next();
        }
    }

    Ok(ActivityElement::Switch(SwitchBlock { condition, cases }))
}

fn parse_partition<'a, I>(
    line: &str,
    lines: &mut std::iter::Peekable<I>,
) -> Result<ActivityElement, PlantUmlError>
where
    I: Iterator<Item = &'a str>,
{
    // partition "name" {
    let rest = strip_prefix_ci(line, "partition ").unwrap_or("");
    let name = rest
        .trim()
        .trim_end_matches('{')
        .trim()
        .trim_matches('"')
        .to_string();

    let elements = parse_elements(lines, &["}"])?;

    // Consume closing }
    if let Some(next) = lines.peek() {
        if next.trim() == "}" {
            lines.next();
        }
    }

    Ok(ActivityElement::Partition(Partition { name, elements }))
}

fn parse_note<'a, I>(line: &str, lines: &mut std::iter::Peekable<I>) -> ActivityElement
where
    I: Iterator<Item = &'a str>,
{
    let lower = line.to_lowercase();
    let position = if lower.starts_with("note left") {
        ActivityNotePosition::Left
    } else {
        ActivityNotePosition::Right
    };

    // Single-line: note left : text
    if let Some(colon_pos) = line.find(':') {
        let text = line[colon_pos + 1..].trim().to_string();
        return ActivityElement::Note(ActivityNote { position, text });
    }

    // Multi-line: note left ... end note
    let mut text_lines = Vec::new();
    for next_line in lines.by_ref() {
        let trimmed = next_line.trim();
        if trimmed.eq_ignore_ascii_case("end note") || trimmed.eq_ignore_ascii_case("endnote") {
            break;
        }
        text_lines.push(trimmed.to_string());
    }

    ActivityElement::Note(ActivityNote {
        position,
        text: text_lines.join("\n"),
    })
}

// --- Helper functions ---

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

fn extract_paren_content(s: &str) -> Option<String> {
    let open = s.find('(')?;
    let close = s[open..].find(')')? + open;
    Some(s[open + 1..close].trim().to_string())
}

fn extract_paren_after(s: &str, keyword: &str) -> Option<String> {
    let lower = s.to_lowercase();
    let pos = lower.find(keyword)?;
    let after = &s[pos + keyword.len()..];
    extract_paren_content(after)
}

fn extract_first_paren(s: &str) -> Option<String> {
    extract_paren_content(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_start_stop() {
        let input = "start\nstop";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.elements.len(), 2);
        assert!(matches!(diagram.elements[0], ActivityElement::Start));
        assert!(matches!(diagram.elements[1], ActivityElement::Stop));
    }

    #[test]
    fn test_parse_action() {
        let input = ":Hello World;";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Action(a) => {
                assert_eq!(a.label, "Hello World");
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn test_parse_multiline_action() {
        let input = ":First line\nSecond line;";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Action(a) => {
                assert_eq!(a.label, "First line\nSecond line");
            }
            _ => panic!("expected Action"),
        }
    }

    #[test]
    fn test_parse_if_else() {
        let input = "if (condition) then (yes)\n:do A;\nelse (no)\n:do B;\nendif";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::If(block) => {
                assert_eq!(block.condition, "condition");
                assert_eq!(block.then_label, "yes");
                assert_eq!(block.then_elements.len(), 1);
                assert_eq!(block.else_label, "no");
                assert_eq!(block.else_elements.len(), 1);
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_parse_if_elseif() {
        let input = "if (a) then (1)\n:A;\nelseif (b) then (2)\n:B;\nelse (3)\n:C;\nendif";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::If(block) => {
                assert_eq!(block.condition, "a");
                assert_eq!(block.then_label, "1");
                assert_eq!(block.elseif_blocks.len(), 1);
                assert_eq!(block.elseif_blocks[0].condition, "b");
                assert_eq!(block.else_label, "3");
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_parse_while() {
        let input = "while (condition) is (true)\n:process;\nendwhile (done)";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::While(block) => {
                assert_eq!(block.condition, "condition");
                assert_eq!(block.is_label, "true");
                assert_eq!(block.elements.len(), 1);
                assert_eq!(block.end_label, "done");
            }
            _ => panic!("expected While"),
        }
    }

    #[test]
    fn test_parse_fork() {
        let input = "fork\n:task1;\nfork again\n:task2;\nend fork";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Fork(block) => {
                assert_eq!(block.branches.len(), 2);
                assert_eq!(block.branches[0].len(), 1);
                assert_eq!(block.branches[1].len(), 1);
            }
            _ => panic!("expected Fork"),
        }
    }

    #[test]
    fn test_parse_switch() {
        let input = "switch (test)\ncase (A)\n:do A;\ncase (B)\n:do B;\nendswitch";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Switch(block) => {
                assert_eq!(block.condition, "test");
                assert_eq!(block.cases.len(), 2);
                assert_eq!(block.cases[0].label, "A");
                assert_eq!(block.cases[1].label, "B");
            }
            _ => panic!("expected Switch"),
        }
    }

    #[test]
    fn test_parse_partition() {
        let input = "partition \"Init\" {\n:step1;\n:step2;\n}";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Partition(p) => {
                assert_eq!(p.name, "Init");
                assert_eq!(p.elements.len(), 2);
            }
            _ => panic!("expected Partition"),
        }
    }

    #[test]
    fn test_parse_note() {
        let input = "note right : This is a note";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::Note(n) => {
                assert_eq!(n.position, ActivityNotePosition::Right);
                assert_eq!(n.text, "This is a note");
            }
            _ => panic!("expected Note"),
        }
    }

    #[test]
    fn test_parse_title() {
        let input = "title My Activity\nstart\nstop";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.title.as_deref(), Some("My Activity"));
    }

    #[test]
    fn test_parse_full_activity() {
        let input = r#"start
:Initialize;
if (Valid?) then (yes)
  :Process;
  :Save;
else (no)
  :Show Error;
endif
:Cleanup;
stop"#;
        let diagram = parse(input).unwrap();
        // start + action + if + action + stop = 5
        assert_eq!(diagram.elements.len(), 5);
        assert!(matches!(diagram.elements[0], ActivityElement::Start));
        assert!(matches!(diagram.elements[4], ActivityElement::Stop));
    }

    #[test]
    fn test_parse_nested_if() {
        let input = "if (a) then (yes)\nif (b) then (y)\n:inner;\nendif\nelse (no)\n:outer;\nendif";
        let diagram = parse(input).unwrap();
        match &diagram.elements[0] {
            ActivityElement::If(block) => {
                assert_eq!(block.then_elements.len(), 1);
                assert!(matches!(block.then_elements[0], ActivityElement::If(_)));
                assert_eq!(block.else_elements.len(), 1);
            }
            _ => panic!("expected If"),
        }
    }

    #[test]
    fn test_parse_arrow_label() {
        let input = ":A;\n-> labeled;\n:B;";
        let diagram = parse(input).unwrap();
        assert_eq!(diagram.elements.len(), 3);
        match &diagram.elements[1] {
            ActivityElement::Arrow(a) => assert_eq!(a.label, "labeled"),
            _ => panic!("expected Arrow"),
        }
    }

    #[test]
    fn test_parse_detach() {
        let input = ":A;\ndetach";
        let diagram = parse(input).unwrap();
        assert!(matches!(diagram.elements[1], ActivityElement::Detach));
    }
}
