/// Parser for component diagrams.
use crate::ast::component::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<ComponentDiagram, PlantUmlError> {
    let mut diagram = ComponentDiagram {
        title: None,
        components: Vec::new(),
        packages: Vec::new(),
        relations: Vec::new(),
    };
    let mut package_stack: Vec<usize> = Vec::new();

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('\'') {
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
        if line.starts_with("skinparam") || line.starts_with("hide ") || line.starts_with("scale ")
        {
            continue;
        }

        // Container keywords (all rendered as a package frame).
        if let Some(rest) = ["package", "node", "folder", "cloud", "database", "frame"]
            .iter()
            .find_map(|kw| strip_keyword(line, kw))
        {
            let (name, _r) = parse_token(rest);
            diagram.packages.push(CompPackage {
                name,
                members: Vec::new(),
            });
            if rest.trim_end().ends_with('{') {
                package_stack.push(diagram.packages.len() - 1);
            }
            continue;
        }

        if let Some(rest) = strip_keyword(line, "component") {
            declare_decl(&mut diagram, &package_stack, rest, CompKind::Component);
            continue;
        }
        if let Some(rest) = strip_keyword(line, "interface") {
            declare_decl(&mut diagram, &package_stack, rest, CompKind::Interface);
            continue;
        }
        if let Some(rest) = line.strip_prefix("() ") {
            declare_decl(&mut diagram, &package_stack, rest.trim(), CompKind::Interface);
            continue;
        }

        // Bare `[Name]` declaration.
        if line.starts_with('[') && line.ends_with(']') && !line.contains("->") {
            let name = line[1..line.len() - 1].trim().to_string();
            declare(
                &mut diagram,
                &package_stack,
                &name,
                &name,
                CompKind::Component,
                None,
            );
            continue;
        }

        if let Some(rel) = parse_relation(line) {
            for raw in [&rel.0, &rel.1] {
                ensure(&mut diagram, &package_stack, raw);
            }
            diagram.relations.push(CompRelation {
                from: canonical(&rel.0),
                to: canonical(&rel.1),
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

fn parse_token(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (rest[..end].to_string(), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return (rest[..end].trim().to_string(), &rest[end + 1..]);
        }
    }
    let end = input
        .find(|c: char| c.is_whitespace() || c == '{')
        .unwrap_or(input.len());
    (input[..end].to_string(), &input[end..])
}

fn parse_raw_token(input: &str) -> (String, &str) {
    let input = input.trim_start();
    if let Some(rest) = input.strip_prefix('"') {
        if let Some(end) = rest.find('"') {
            return (format!("\"{}\"", &rest[..end]), &rest[end + 1..]);
        }
    }
    if let Some(rest) = input.strip_prefix('[') {
        if let Some(end) = rest.find(']') {
            return (format!("[{}]", rest[..end].trim()), &rest[end + 1..]);
        }
    }
    let end = input.find(char::is_whitespace).unwrap_or(input.len());
    (input[..end].to_string(), &input[end..])
}

fn canonical(token: &str) -> String {
    let t = token.trim();
    if let Some(inner) = t.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        return inner.trim().to_string();
    }
    t.trim_matches('"').to_string()
}

fn declare_decl(
    diagram: &mut ComponentDiagram,
    package_stack: &[usize],
    rest: &str,
    kind: CompKind,
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
    declare(diagram, package_stack, &name, &display, kind, color);
}

fn ensure(diagram: &mut ComponentDiagram, package_stack: &[usize], raw: &str) {
    let name = canonical(raw);
    if diagram.components.iter().any(|c| c.name == name) {
        return;
    }
    // `[X]` is a component; a bare name that was not declared is treated as a
    // component too (interfaces must be declared).
    declare(
        diagram,
        package_stack,
        &name,
        &name,
        CompKind::Component,
        None,
    );
}

fn declare(
    diagram: &mut ComponentDiagram,
    package_stack: &[usize],
    name: &str,
    display: &str,
    kind: CompKind,
    color: Option<String>,
) {
    if let Some(c) = diagram.components.iter_mut().find(|c| c.name == name) {
        if color.is_some() {
            c.color = color;
        }
        return;
    }
    diagram.components.push(CompDef {
        name: name.to_string(),
        display_name: display.to_string(),
        kind,
        color,
    });
    if let Some(&pi) = package_stack.last() {
        diagram.packages[pi].members.push(name.to_string());
    }
}

type ParsedRelation = (String, String, bool, bool, bool, usize, Option<String>);

fn parse_relation(line: &str) -> Option<ParsedRelation> {
    let (line_part, label) = match line.split_once(" : ") {
        Some((l, lab)) => (l.trim_end(), Some(lab.trim().to_string())),
        None => (line, None),
    };
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
        let d = parse("[Web UI] --> [API Server] : HTTPS\n").unwrap();
        assert_eq!(d.components.len(), 2);
        assert_eq!(d.components[0].name, "Web UI");
        assert!(d.relations[0].arrow);
        assert_eq!(d.relations[0].label.as_deref(), Some("HTTPS"));
    }

    #[test]
    fn test_interface_association() {
        let d = parse("interface Auth\nAuth - [API Server]\n").unwrap();
        assert_eq!(d.components[0].kind, CompKind::Interface);
        let r = &d.relations[0];
        assert!(!r.arrow && !r.back_arrow);
        assert_eq!(r.rank_len, 1);
    }

    #[test]
    fn test_package() {
        let d = parse("package \"Backend\" {\n  [API Server]\n  [Worker]\n}\n").unwrap();
        assert_eq!(d.packages.len(), 1);
        assert_eq!(d.packages[0].members, vec!["API Server", "Worker"]);
    }

    #[test]
    fn test_dashed() {
        let d = parse("[A] ..> [B] : uses\n").unwrap();
        assert!(d.relations[0].dashed);
    }
}
