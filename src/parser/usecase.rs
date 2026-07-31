/// Parser for use case diagrams.
use crate::ast::usecase::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<UseCaseDiagram, PlantUmlError> {
    let mut diagram = UseCaseDiagram {
        title: None,
        elements: Vec::new(),
        containers: Vec::new(),
        relations: Vec::new(),
        left_to_right: false,
    };
    let mut container_stack: Vec<usize> = Vec::new();

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }
        if line == "}" {
            container_stack.pop();
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            diagram.title = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("left to right") {
            diagram.left_to_right = true;
            continue;
        }
        if line.starts_with("top to bottom") {
            diagram.left_to_right = false;
            continue;
        }
        if line.starts_with("skinparam") || line.starts_with("hide ") || line.starts_with("scale ")
        {
            continue;
        }

        if let Some(rest) = strip_keyword(line, "actor") {
            declare_from_decl(&mut diagram, &container_stack, rest, ElementKind::Actor);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "usecase") {
            declare_from_decl(&mut diagram, &container_stack, rest, ElementKind::UseCase);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "rectangle").or_else(|| strip_keyword(line, "package"))
        {
            let (name, _r) = parse_token(rest);
            diagram.containers.push(ContainerDef {
                name,
                members: Vec::new(),
            });
            if rest.trim_end().ends_with('{') {
                container_stack.push(diagram.containers.len() - 1);
            }
            continue;
        }

        if let Some(rel) = parse_relation(line) {
            for endpoint in [&rel.0, &rel.1] {
                ensure_element(&mut diagram, &container_stack, endpoint);
            }
            let (from_ref, to_ref) = (canonical_name(&rel.0), canonical_name(&rel.1));
            diagram.relations.push(UcRelation {
                from: from_ref,
                to: to_ref,
                arrow: rel.2,
                back_arrow: rel.3,
                dashed: rel.4,
                rank_len: rel.5,
                label: rel.6,
            });
            continue;
        }
    }

    Ok(diagram)
}

fn strip_keyword<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(keyword)?;
    if rest.starts_with(' ') || rest.starts_with('\t') {
        Some(rest.trim_start())
    } else {
        None
    }
}

/// Parse a declaration remainder: `Name`, `"Long name" as N`, `(Text) as N`,
/// `:Name:`, optionally followed by ` #Color`.
fn declare_from_decl(
    diagram: &mut UseCaseDiagram,
    container_stack: &[usize],
    rest: &str,
    kind: ElementKind,
) {
    let (display, r) = parse_token(rest);
    let mut rest = r.trim_start();
    let mut name = display.clone();
    if let Some(after_as) = strip_keyword(rest, "as") {
        let (alias, r2) = parse_token(after_as);
        name = alias;
        rest = r2.trim_start();
    }
    let color = rest
        .split_whitespace()
        .find(|t| t.starts_with('#'))
        .map(|t| t.to_string());
    declare(diagram, container_stack, &name, &display, kind, color);
}

/// Strip decorations from an endpoint reference to its canonical name.
fn canonical_name(token: &str) -> String {
    let t = token.trim();
    if let Some(inner) = t.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
        return inner.trim().to_string();
    }
    if let Some(inner) = t.strip_prefix(':').and_then(|s| s.strip_suffix(':')) {
        return inner.trim().to_string();
    }
    t.trim_matches('"').to_string()
}

/// Ensure an endpoint element exists; `(X)` creates a use case, `:X:` an
/// actor, a bare name defaults to a use case only if unknown & parenthesised
/// — otherwise an actor-or-usecase already declared wins, and bare unknown
/// names become use cases (PlantUML default in use case diagrams).
fn ensure_element(diagram: &mut UseCaseDiagram, container_stack: &[usize], token: &str) {
    let name = canonical_name(token);
    if diagram.elements.iter().any(|e| e.name == name) {
        return;
    }
    let t = token.trim();
    let kind = if t.starts_with(':') && t.ends_with(':') {
        ElementKind::Actor
    } else {
        ElementKind::UseCase
    };
    declare(diagram, container_stack, &name, &name, kind, None);
}

fn declare(
    diagram: &mut UseCaseDiagram,
    container_stack: &[usize],
    name: &str,
    display: &str,
    kind: ElementKind,
    color: Option<String>,
) {
    if let Some(e) = diagram.elements.iter_mut().find(|e| e.name == name) {
        if color.is_some() {
            e.color = color;
        }
        return;
    }
    diagram.elements.push(Element {
        name: name.to_string(),
        display_name: display.to_string(),
        kind,
        color,
    });
    if let Some(&ci) = container_stack.last() {
        diagram.containers[ci].members.push(name.to_string());
    }
}

/// Parse one token: quoted string, `(...)`, `:...:`, or bare word.
/// Returns (content, remainder).
fn parse_token(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix('(') {
        if let Some(end) = rest.find(')') {
            return (rest[..end].trim().to_string(), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix(':') {
        if let Some(end) = rest.find(':') {
            return (rest[..end].to_string(), &rest[end + 1..]);
        }
    }
    let end = input
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(input.len());
    (input[..end].to_string(), &input[end..])
}

/// Like `parse_token` but keeps surrounding delimiters in the returned text.
fn parse_raw_token(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (format!("\"{}\"", &rest[..end]), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix('(') {
        if let Some(end) = rest.find(')') {
            return (format!("({})", rest[..end].trim()), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix(':') {
        if let Some(end) = rest.find(':') {
            return (format!(":{}:", &rest[..end]), &rest[end + 1..]);
        }
    }
    let end = input
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(input.len());
    (input[..end].to_string(), &input[end..])
}

type ParsedRelation = (String, String, bool, bool, bool, usize, Option<String>);

/// Parse `A --> B : label`; endpoints may be `Name`, `"Q"`, `(UC)`, `:Actor:`.
/// Returns (from_token, to_token, arrow, back_arrow, dashed, rank_len, label).
fn parse_relation(line: &str) -> Option<ParsedRelation> {
    let (line_part, label) = match line.split_once(" : ") {
        Some((l, lab)) => (l.trim_end(), Some(lab.trim().to_string())),
        None => (line, None),
    };

    // Tokenize into up to 3 parts: endpoint, arrow, endpoint. Endpoints keep
    // their decorations (`(..)`, `:..:`, quotes) so element kinds survive.
    let (left, rest) = parse_raw_token(line_part);
    let rest = rest.trim_start();
    let (arrow_tok, rest2) = match rest.find(char::is_whitespace) {
        Some(i) => (&rest[..i], rest[i..].trim_start()),
        None => return None,
    };
    let (right, tail) = parse_raw_token(rest2);
    if !tail.trim().is_empty() || left.is_empty() || right.is_empty() {
        return None;
    }

    let (arrow, back_arrow, dashed, rank_len) = parse_arrow(arrow_tok)?;
    Some((left, right, arrow, back_arrow, dashed, rank_len, label))
}

fn parse_arrow(token: &str) -> Option<(bool, bool, bool, usize)> {
    let mut t = token.to_string();
    for dir in ["left", "right", "up", "down", "le", "ri", "do", "l", "r", "u", "d"] {
        for (open, close) in [('-', '-'), ('.', '.')] {
            let pat = format!("{}{}{}", open, dir, close);
            if t.contains(&pat) {
                t = t.replace(&pat, &format!("{}{}", open, close));
            }
        }
    }
    let mut rest = t.as_str();
    let back_arrow = if let Some(r) = rest.strip_prefix('<') {
        rest = r;
        true
    } else {
        false
    };
    let arrow = if let Some(r) = rest.strip_suffix('>') {
        rest = r;
        true
    } else {
        false
    };
    if rest.is_empty() || !rest.chars().all(|c| c == '-' || c == '.') {
        return None;
    }
    Some((arrow, back_arrow, rest.contains('.'), rest.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let d = parse("actor Customer\nusecase (Browse) as UC1\nCustomer --> UC1\n").unwrap();
        assert_eq!(d.elements.len(), 2);
        assert_eq!(d.elements[0].kind, ElementKind::Actor);
        assert_eq!(d.elements[1].kind, ElementKind::UseCase);
        assert_eq!(d.elements[1].display_name, "Browse");
        assert_eq!(d.relations.len(), 1);
        assert!(d.relations[0].arrow);
    }

    #[test]
    fn test_inline_elements() {
        let d = parse(":Guest: --> (Sign up)\n").unwrap();
        assert_eq!(d.elements[0].kind, ElementKind::Actor);
        assert_eq!(d.elements[0].name, "Guest");
        assert_eq!(d.elements[1].kind, ElementKind::UseCase);
        assert_eq!(d.elements[1].name, "Sign up");
    }

    #[test]
    fn test_include() {
        let d = parse("usecase (A) as UC1\nusecase (B) as UC2\nUC2 ..> UC1 : <<include>>\n")
            .unwrap();
        let r = &d.relations[0];
        assert!(r.dashed);
        assert!(r.arrow);
        assert_eq!(r.label.as_deref(), Some("<<include>>"));
    }

    #[test]
    fn test_rectangle_container() {
        let d = parse("rectangle Checkout {\n  usecase \"Pay\" as UC1\n}\nGuest --> UC1\n")
            .unwrap();
        assert_eq!(d.containers.len(), 1);
        assert_eq!(d.containers[0].members, vec!["UC1"]);
    }

    #[test]
    fn test_left_to_right() {
        let d = parse("left to right direction\nactor A\n").unwrap();
        assert!(d.left_to_right);
    }
}
