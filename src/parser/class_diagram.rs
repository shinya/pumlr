/// Parser for class diagrams.
use crate::ast::class_diagram::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<ClassDiagram, PlantUmlError> {
    let mut diagram = ClassDiagram {
        title: None,
        classes: Vec::new(),
        packages: Vec::new(),
        relations: Vec::new(),
    };
    // Stack of package indices we are currently inside (nested packages keep
    // their nesting via `PackageDef::parent`).
    let mut package_stack: Vec<usize> = Vec::new();
    // When inside a `class Foo { ... }` body, index of the class being filled.
    let mut open_class: Option<usize> = None;

    for (line_no, raw_line) in body.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }

        // Inside a class body: members until `}`.
        if let Some(class_idx) = open_class {
            if line == "}" {
                open_class = None;
                continue;
            }
            // Separator lines like `--` / `..` / `==` inside a body are ignored.
            if !line.is_empty() && line.chars().all(|c| c == '-' || c == '.' || c == '=') {
                continue;
            }
            let member = parse_member(line);
            let class = &mut diagram.classes[class_idx];
            if class.kind == ClassKind::Enum || !member.text.contains('(') {
                class.fields.push(member);
            } else {
                class.methods.push(member);
            }
            continue;
        }

        if line == "}" {
            package_stack.pop();
            continue;
        }

        if let Some(rest) = line.strip_prefix("title ") {
            diagram.title = Some(rest.trim().to_string());
            continue;
        }
        // Ignored directives.
        if line.starts_with("skinparam")
            || line.starts_with("hide ")
            || line.starts_with("show ")
            || line.starts_with("scale ")
            || line.starts_with("left to right")
            || line.starts_with("top to bottom")
        {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "package") {
            let (name, _rest) = parse_name(rest);
            diagram.packages.push(PackageDef {
                name,
                classes: Vec::new(),
                parent: package_stack.last().copied(),
            });
            if rest.trim_end().ends_with('{') {
                package_stack.push(diagram.packages.len() - 1);
            }
            continue;
        }

        // Class-like declarations.
        if let Some((kind, rest)) = parse_class_keyword(line) {
            let (idx, has_body) = declare_class(&mut diagram, kind, rest, line_no)?;
            let class_name = diagram.classes[idx].name.clone();
            if let Some(&pkg) = package_stack.last() {
                if !diagram.packages[pkg].classes.contains(&class_name) {
                    diagram.packages[pkg].classes.push(class_name);
                }
            }
            if has_body {
                open_class = Some(idx);
            }
            continue;
        }

        // Relation line?
        if let Some((rel, left_lollipop, right_lollipop)) = parse_relation(line) {
            ensure_class(&mut diagram, &rel.left, &package_stack);
            ensure_class(&mut diagram, &rel.right, &package_stack);
            // `Foo ()-- Bar` displays Foo as a lollipop circle.
            if left_lollipop {
                set_circle_kind(&mut diagram, &rel.left);
            }
            if right_lollipop {
                set_circle_kind(&mut diagram, &rel.right);
            }
            diagram.relations.push(rel);
            continue;
        }

        // `ClassName : member` adds a member to an existing/new class.
        if let Some((name, member_text)) = line.split_once(" : ") {
            let name = name.trim().to_string();
            let member = parse_member(member_text.trim());
            ensure_class(&mut diagram, &name, &package_stack);
            let class = diagram
                .classes
                .iter_mut()
                .find(|c| c.name == name)
                .expect("ensured above");
            if class.kind == ClassKind::Enum || !member.text.contains('(') {
                class.fields.push(member);
            } else {
                class.methods.push(member);
            }
            continue;
        }

        // Unknown lines are ignored (PlantUML is lenient).
    }

    Ok(diagram)
}

fn parse_class_keyword(line: &str) -> Option<(ClassKind, &str)> {
    if let Some(rest) = strip_keyword(line, "abstract class") {
        return Some((ClassKind::AbstractClass, rest));
    }
    if let Some(rest) = strip_keyword(line, "abstract") {
        return Some((ClassKind::AbstractClass, rest));
    }
    if let Some(rest) = strip_keyword(line, "class") {
        return Some((ClassKind::Class, rest));
    }
    if let Some(rest) = strip_keyword(line, "interface") {
        return Some((ClassKind::Interface, rest));
    }
    if let Some(rest) = strip_keyword(line, "enum") {
        return Some((ClassKind::Enum, rest));
    }
    if let Some(rest) = strip_keyword(line, "circle") {
        return Some((ClassKind::Circle, rest));
    }
    // `() "Name" as N` — lollipop interface shorthand.
    if let Some(rest) = line.strip_prefix("()") {
        if rest.starts_with(' ') || rest.starts_with('\t') {
            return Some((ClassKind::Circle, rest.trim_start()));
        }
    }
    None
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.starts_with(' ') || rest.starts_with('\t') {
        Some(rest.trim_start())
    } else {
        None
    }
}

/// Parse a (possibly quoted) name; returns (name, remainder).
fn parse_name(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), &rest[end + 1..]);
        }
    }
    let end = input
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(input.len());
    (input[..end].to_string(), &input[end..])
}

fn declare_class(
    diagram: &mut ClassDiagram,
    kind: ClassKind,
    rest: &str,
    _line_no: usize,
) -> Result<(usize, bool), PlantUmlError> {
    let (display_name, mut rest) = parse_name(rest);
    let mut name = display_name.clone();

    rest = rest.trim_start();
    if let Some(after_as) = strip_keyword(rest, "as") {
        let (alias, r) = parse_name(after_as);
        name = alias;
        rest = r.trim_start();
    }

    let mut stereotype = None;
    if let Some(start) = rest.find("<<") {
        if let Some(end) = rest.find(">>") {
            stereotype = Some(rest[start + 2..end].trim().to_string());
        }
    }

    let color = rest
        .split_whitespace()
        .find(|t| t.starts_with('#'))
        .map(|t| t.to_string());

    let has_body = rest.trim_end().ends_with('{');

    // Re-declaration merges (e.g. members added after the fact).
    if let Some(idx) = diagram.classes.iter().position(|c| c.name == name) {
        let class = &mut diagram.classes[idx];
        if class.kind == ClassKind::Class {
            class.kind = kind;
        }
        if stereotype.is_some() {
            class.stereotype = stereotype;
        }
        if color.is_some() {
            class.color = color;
        }
        return Ok((idx, has_body));
    }

    diagram.classes.push(ClassDef {
        name,
        display_name,
        kind,
        stereotype,
        fields: Vec::new(),
        methods: Vec::new(),
        color,
    });
    Ok((diagram.classes.len() - 1, has_body))
}

/// Mark an existing class as a lollipop circle (used by `()`-decorated arrows).
fn set_circle_kind(diagram: &mut ClassDiagram, name: &str) {
    if let Some(class) = diagram.classes.iter_mut().find(|c| c.name == name) {
        class.kind = ClassKind::Circle;
    }
}

fn ensure_class(diagram: &mut ClassDiagram, name: &str, package_stack: &[usize]) {
    if diagram.classes.iter().any(|c| c.name == name) {
        return;
    }
    diagram.classes.push(ClassDef {
        name: name.to_string(),
        display_name: name.to_string(),
        kind: ClassKind::Class,
        stereotype: None,
        fields: Vec::new(),
        methods: Vec::new(),
        color: None,
    });
    if let Some(&pkg) = package_stack.last() {
        diagram.packages[pkg].classes.push(name.to_string());
    }
}

fn parse_member(text: &str) -> Member {
    let mut text = text.trim();
    let mut is_abstract = false;
    let mut is_static = false;
    loop {
        if let Some(rest) = text.strip_prefix("{abstract}") {
            is_abstract = true;
            text = rest.trim_start();
        } else if let Some(rest) = text.strip_prefix("{static}") {
            is_static = true;
            text = rest.trim_start();
        } else {
            break;
        }
    }
    let (visibility, rest) = match text.chars().next() {
        Some('+') => (Some(Visibility::Public), &text[1..]),
        Some('-') => (Some(Visibility::Private), &text[1..]),
        Some('#') => (Some(Visibility::Protected), &text[1..]),
        Some('~') => (Some(Visibility::PackagePrivate), &text[1..]),
        _ => (None, text),
    };
    Member {
        text: rest.trim_start().to_string(),
        visibility,
        is_abstract,
        is_static,
    }
}

/// Try to parse a relation line: `A "card" ARROW "card" B : label`.
/// Returns the relation plus whether the left/right entity is displayed as a
/// lollipop circle (`()`-decorated arrow, e.g. `Foo ()-- Bar`).
fn parse_relation(line: &str) -> Option<(Relation, bool, bool)> {
    // Split off the label first.
    let (line_part, label) = split_label(line);

    let tokens = tokenize_relation(line_part)?;
    let (left, left_card, mut arrow, right_card, right) = tokens;

    // `()` glued to the arrow marks that end's entity as a lollipop.
    let mut left_lollipop = false;
    let mut right_lollipop = false;
    if let Some(rest) = arrow.strip_prefix("()") {
        left_lollipop = true;
        arrow = rest.to_string();
    }
    if let Some(rest) = arrow.strip_suffix("()") {
        right_lollipop = true;
        arrow = rest.to_string();
    }

    let (left_marker, right_marker, dashed, rank_len) = parse_arrow(&arrow)?;

    Some((
        Relation {
            left,
            right,
            left_marker,
            right_marker,
            dashed,
            rank_len,
            label,
            left_card,
            right_card,
        },
        left_lollipop,
        right_lollipop,
    ))
}

/// Split `... : label` (label part is optional).
fn split_label(line: &str) -> (&str, Option<String>) {
    // Find a ':' that is not inside quotes.
    let mut in_quotes = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ':' if !in_quotes => {
                let label = line[i + 1..].trim();
                let label = label
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .trim()
                    .to_string();
                return (line[..i].trim_end(), Some(label).filter(|s| !s.is_empty()));
            }
            _ => {}
        }
    }
    (line, None)
}

type RelationTokens = (String, Option<String>, String, Option<String>, String);

/// Tokenize `Name ["card"] arrow ["card"] Name`.
fn tokenize_relation(line: &str) -> Option<RelationTokens> {
    let mut parts: Vec<String> = Vec::new();
    let mut rest = line.trim();
    while !rest.is_empty() {
        if rest.starts_with('"') {
            let end = rest[1..].find('"')? + 1;
            parts.push(rest[..=end].to_string());
            rest = rest[end + 1..].trim_start();
        } else {
            let end = rest
                .find(char::is_whitespace)
                .unwrap_or(rest.len());
            parts.push(rest[..end].to_string());
            rest = rest[end..].trim_start();
        }
    }
    // Find the arrow token: contains '-' or '.' and is not a name.
    let arrow_idx = parts.iter().position(|p| is_arrow_token(p))?;
    if arrow_idx == 0 || arrow_idx == parts.len() - 1 {
        return None;
    }

    let mut left = parts[0].clone();
    let mut left_card = None;
    if arrow_idx == 2 {
        left_card = Some(parts[1].trim_matches('"').to_string());
    } else if arrow_idx != 1 {
        return None;
    }
    left = left.trim_matches('"').to_string();

    let after = &parts[arrow_idx + 1..];
    let (right_card, right) = match after.len() {
        1 => (None, after[0].trim_matches('"').to_string()),
        2 => (
            Some(after[0].trim_matches('"').to_string()),
            after[1].trim_matches('"').to_string(),
        ),
        _ => return None,
    };

    Some((left, left_card, parts[arrow_idx].clone(), right_card, right))
}

fn is_arrow_token(token: &str) -> bool {
    if !token.contains('-') && !token.contains('.') {
        return false;
    }
    token.chars().all(|c| {
        matches!(
            c,
            '-' | '.' | '<' | '>' | '|' | '*' | 'o' | '#' | 'x' | '+' | '^' | '(' | ')'
        )
    }) || is_arrow_with_direction(token)
}

fn is_arrow_with_direction(token: &str) -> bool {
    strip_direction(token) != token && !strip_direction(token).is_empty()
}

/// Remove embedded direction hints: `-down->` → `-->` etc.
fn strip_direction(token: &str) -> String {
    let mut t = token.to_string();
    for dir in ["left", "right", "up", "down", "le", "ri", "up", "do", "l", "r", "u", "d"] {
        let with_dashes = format!("-{}-", dir);
        if t.contains(&with_dashes) {
            t = t.replace(&with_dashes, "--");
            return t;
        }
        let with_dots = format!(".{}.", dir);
        if t.contains(&with_dots) {
            t = t.replace(&with_dots, "..");
            return t;
        }
    }
    t
}

fn parse_arrow(token: &str) -> Option<(EndMarker, EndMarker, bool, usize)> {
    let token = strip_direction(token);
    let mut rest = token.as_str();

    let left_marker;
    if let Some(r) = rest.strip_prefix("<|") {
        left_marker = EndMarker::Triangle;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('<') {
        left_marker = EndMarker::ArrowHead;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('*') {
        left_marker = EndMarker::FilledDiamond;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('o') {
        left_marker = EndMarker::Diamond;
        rest = r;
    } else if let Some(r) = rest.strip_prefix('#') {
        left_marker = EndMarker::FilledDiamond;
        rest = r;
    } else {
        left_marker = EndMarker::None;
    }

    let right_marker;
    if let Some(r) = rest.strip_suffix("|>") {
        right_marker = EndMarker::Triangle;
        rest = r;
    } else if let Some(r) = rest.strip_suffix('>') {
        right_marker = EndMarker::ArrowHead;
        rest = r;
    } else if let Some(r) = rest.strip_suffix('*') {
        right_marker = EndMarker::FilledDiamond;
        rest = r;
    } else if let Some(r) = rest.strip_suffix('o') {
        right_marker = EndMarker::Diamond;
        rest = r;
    } else if let Some(r) = rest.strip_suffix('#') {
        right_marker = EndMarker::FilledDiamond;
        rest = r;
    } else {
        right_marker = EndMarker::None;
    }

    if rest.is_empty() {
        return None;
    }
    let dashed = rest.contains('.');
    if !rest.chars().all(|c| c == '-' || c == '.') {
        return None;
    }
    Some((left_marker, right_marker, dashed, rest.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_class() {
        let d = parse("class Animal {\n  +name: String\n  +makeSound(): void\n}\nclass Dog\nAnimal <|-- Dog\n").unwrap();
        assert_eq!(d.classes.len(), 2);
        assert_eq!(d.classes[0].fields.len(), 1);
        assert_eq!(d.classes[0].methods.len(), 1);
        assert_eq!(d.relations.len(), 1);
        let r = &d.relations[0];
        assert_eq!(r.left, "Animal");
        assert_eq!(r.right, "Dog");
        assert_eq!(r.left_marker, EndMarker::Triangle);
        assert_eq!(r.right_marker, EndMarker::None);
        assert!(!r.dashed);
    }

    #[test]
    fn test_visibility() {
        let m = parse_member("+name: String");
        assert_eq!(m.visibility, Some(Visibility::Public));
        assert_eq!(m.text, "name: String");
        let m = parse_member("#age: int");
        assert_eq!(m.visibility, Some(Visibility::Protected));
        let m = parse_member("-id: long");
        assert_eq!(m.visibility, Some(Visibility::Private));
    }

    #[test]
    fn test_cardinality_and_label() {
        let d = parse("Customer \"1\" --> \"0..*\" Order : places\n").unwrap();
        assert_eq!(d.classes.len(), 2);
        let r = &d.relations[0];
        assert_eq!(r.left_card.as_deref(), Some("1"));
        assert_eq!(r.right_card.as_deref(), Some("0..*"));
        assert_eq!(r.label.as_deref(), Some("places"));
        assert_eq!(r.right_marker, EndMarker::ArrowHead);
    }

    #[test]
    fn test_composition_aggregation() {
        let d = parse("Order *-- LineItem\nLineItem o-- Product\n").unwrap();
        assert_eq!(d.relations[0].left_marker, EndMarker::FilledDiamond);
        assert_eq!(d.relations[1].left_marker, EndMarker::Diamond);
    }

    #[test]
    fn test_dashed_realization() {
        let d = parse("Repository <|.. UserRepository\n").unwrap();
        let r = &d.relations[0];
        assert!(r.dashed);
        assert_eq!(r.left_marker, EndMarker::Triangle);
    }

    #[test]
    fn test_package() {
        let d = parse("package domain {\n  interface Repository\n  enum Status {\n    ACTIVE\n  }\n}\nclass Foo\n").unwrap();
        assert_eq!(d.packages.len(), 1);
        assert_eq!(d.packages[0].classes, vec!["Repository", "Status"]);
        assert_eq!(d.classes.len(), 3);
        assert_eq!(d.classes[1].kind, ClassKind::Enum);
        assert_eq!(d.classes[1].fields.len(), 1);
        assert_eq!(d.classes[1].fields[0].text, "ACTIVE");
    }

    #[test]
    fn test_abstract_and_stereotype() {
        let d = parse("abstract class Entity <<root>>\ninterface Repo\n").unwrap();
        assert_eq!(d.classes[0].kind, ClassKind::AbstractClass);
        assert_eq!(d.classes[0].stereotype.as_deref(), Some("root"));
        assert_eq!(d.classes[1].kind, ClassKind::Interface);
    }

    #[test]
    fn test_member_via_colon() {
        let d = parse("class Animal\nAnimal : +name: String\nAnimal : +run(): void\n").unwrap();
        assert_eq!(d.classes[0].fields.len(), 1);
        assert_eq!(d.classes[0].methods.len(), 1);
    }

    #[test]
    fn test_static_member() {
        let m = parse_member("{static} count: int");
        assert!(m.is_static);
        assert!(!m.is_abstract);
        assert_eq!(m.text, "count: int");
        let m = parse_member("{static} +instance(): Counter");
        assert!(m.is_static);
        assert_eq!(m.visibility, Some(Visibility::Public));
        assert_eq!(m.text, "instance(): Counter");
    }

    #[test]
    fn test_nested_packages() {
        let d = parse(
            "package outer {\n  package inner {\n    class A\n  }\n  class B\n}\nclass C\n",
        )
        .unwrap();
        assert_eq!(d.packages.len(), 2);
        assert_eq!(d.packages[0].name, "outer");
        assert_eq!(d.packages[0].parent, None);
        assert_eq!(d.packages[1].name, "inner");
        assert_eq!(d.packages[1].parent, Some(0));
        assert_eq!(d.packages[0].classes, vec!["B"]);
        assert_eq!(d.packages[1].classes, vec!["A"]);
    }

    #[test]
    fn test_circle_declarations() {
        let d = parse("circle Direct\n() \"Runnable\" as R\nR - Counter\n").unwrap();
        assert_eq!(d.classes[0].kind, ClassKind::Circle);
        assert_eq!(d.classes[0].name, "Direct");
        assert_eq!(d.classes[1].kind, ClassKind::Circle);
        assert_eq!(d.classes[1].name, "R");
        assert_eq!(d.classes[1].display_name, "Runnable");
        let r = &d.relations[0];
        assert_eq!(r.left, "R");
        assert_eq!(r.right, "Counter");
        assert_eq!(r.rank_len, 1);
    }

    #[test]
    fn test_lollipop_arrow() {
        let d = parse("class Bar\nBaz ()-- Bar\n").unwrap();
        let baz = d.classes.iter().find(|c| c.name == "Baz").unwrap();
        assert_eq!(baz.kind, ClassKind::Circle);
        assert_eq!(d.relations.len(), 1);
        assert_eq!(d.relations[0].left, "Baz");
        // Right-side variant.
        let d = parse("class Bar\nBar --() Qux\n").unwrap();
        let qux = d.classes.iter().find(|c| c.name == "Qux").unwrap();
        assert_eq!(qux.kind, ClassKind::Circle);
    }

    #[test]
    fn test_direction_hint() {
        let d = parse("A -down-> B\n").unwrap();
        assert_eq!(d.relations[0].right_marker, EndMarker::ArrowHead);
        assert_eq!(d.relations[0].rank_len, 2);
    }
}
