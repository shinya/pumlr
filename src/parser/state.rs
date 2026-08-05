/// Parser for state diagrams.
use crate::ast::state::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<StateDiagram, PlantUmlError> {
    let mut diagram = StateDiagram {
        title: None,
        states: Vec::new(),
        transitions: Vec::new(),
    };
    // Stack of enclosing composite states. `regions > 0` once a `--`
    // separator was seen; children then belong to `<name>$<regions-1>`.
    struct ScopeFrame {
        name: String,
        regions: usize,
    }
    impl ScopeFrame {
        /// Name that children/pseudo-states of this scope attach to.
        fn effective(&self) -> String {
            if self.regions > 0 {
                format!("{}${}", self.name, self.regions - 1)
            } else {
                self.name.clone()
            }
        }
    }
    let mut scope: Vec<ScopeFrame> = Vec::new();

    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }
        if line == "}" {
            scope.pop();
            continue;
        }

        // Concurrent region separator (`--` / `||`) inside a composite body.
        if !scope.is_empty()
            && line.len() >= 2
            && (line.chars().all(|c| c == '-') || line.chars().all(|c| c == '|'))
        {
            let frame = scope.last_mut().expect("checked non-empty");
            if frame.regions == 0 {
                // Retroactively move everything parsed so far into region 0.
                let region0 = format!("{}$0", frame.name);
                for st in diagram.states.iter_mut() {
                    if st.parent.as_deref() == Some(frame.name.as_str()) {
                        st.parent = Some(region0.clone());
                    }
                }
                let old_start = format!("{}{}", START_PREFIX, frame.name);
                let old_end = format!("{}{}", END_PREFIX, frame.name);
                for tr in diagram.transitions.iter_mut() {
                    for ep in [&mut tr.from, &mut tr.to] {
                        if *ep == old_start {
                            *ep = format!("{}{}", START_PREFIX, region0);
                        } else if *ep == old_end {
                            *ep = format!("{}{}", END_PREFIX, region0);
                        }
                    }
                }
                declare_region(&mut diagram, &region0, &frame.name);
                frame.regions = 1;
            }
            let next = format!("{}${}", frame.name, frame.regions);
            declare_region(&mut diagram, &next, &frame.name);
            frame.regions += 1;
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            diagram.title = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("skinparam")
            || line.starts_with("hide ")
            || line.starts_with("scale ")
        {
            continue;
        }

        if let Some(rest) = line.strip_prefix("state ") {
            let rest = rest.trim();
            let (display_name, rest) = parse_name(rest);
            let mut rest = rest.trim_start();
            let mut name = display_name.clone();
            if let Some(after_as) = rest.strip_prefix("as ") {
                let (alias, r) = parse_name(after_as);
                name = alias;
                rest = r.trim_start();
            }
            let color = rest
                .split_whitespace()
                .find(|t| t.starts_with('#'))
                .map(|t| t.to_string());
            let opens_body = rest.trim_end().ends_with('{');
            let parent = scope.last().map(|f| f.effective());
            declare_state(&mut diagram, &name, &display_name, parent, color, opens_body);
            if opens_body {
                scope.push(ScopeFrame { name, regions: 0 });
            }
            continue;
        }

        // Transition line?
        if let Some((from, to, label, rank_len)) = parse_transition(line) {
            let eff_scope = scope.last().map(|f| f.effective()).unwrap_or_default();
            let comp_scope = scope
                .last()
                .map(|f| f.name.clone())
                .unwrap_or_default();
            let from = resolve_endpoint(&from, &eff_scope, &comp_scope, true);
            let to = resolve_endpoint(&to, &eff_scope, &comp_scope, false);
            for endpoint in [&from, &to] {
                if !endpoint.starts_with(START_PREFIX)
                    && !endpoint.starts_with(END_PREFIX)
                    && !endpoint.starts_with(HIST_PREFIX)
                {
                    let parent = scope.last().map(|f| f.effective());
                    declare_state(&mut diagram, endpoint, endpoint, parent, None, false);
                }
            }
            diagram.transitions.push(Transition {
                from,
                to,
                label,
                rank_len,
            });
            continue;
        }

        // `State : description`
        if let Some((name, desc)) = line.split_once(':') {
            let name = name.trim();
            let desc = desc.trim();
            if !name.is_empty() && !name.contains(' ') && !desc.is_empty() {
                let parent = scope.last().map(|f| f.effective());
                declare_state(&mut diagram, name, name, parent, None, false);
                let state = diagram
                    .states
                    .iter_mut()
                    .find(|s| s.name == name)
                    .expect("declared above");
                state.descriptions.push(desc.to_string());
            }
            continue;
        }
    }

    Ok(diagram)
}

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

fn declare_state(
    diagram: &mut StateDiagram,
    name: &str,
    display_name: &str,
    parent: Option<String>,
    color: Option<String>,
    composite: bool,
) {
    if let Some(existing) = diagram.states.iter_mut().find(|s| s.name == name) {
        if color.is_some() {
            existing.color = color;
        }
        existing.composite |= composite;
        return;
    }
    diagram.states.push(StateDef {
        name: name.to_string(),
        display_name: display_name.to_string(),
        descriptions: Vec::new(),
        parent,
        color,
        composite,
        is_region: false,
    });
}

/// Declare a synthetic concurrent region `<composite>$<index>`.
fn declare_region(diagram: &mut StateDiagram, name: &str, parent: &str) {
    diagram.states.push(StateDef {
        name: name.to_string(),
        display_name: String::new(),
        descriptions: Vec::new(),
        parent: Some(parent.to_string()),
        color: None,
        composite: true,
        is_region: true,
    });
}

/// `[*]` on the left is the scope's start; on the right the scope's end.
/// `[H]` is the enclosing composite's shallow history; `X[H]` is state X's.
/// `eff_scope` is the region-aware scope name, `comp_scope` the composite
/// itself (history always belongs to the composite, not a region).
fn resolve_endpoint(name: &str, eff_scope: &str, comp_scope: &str, is_from: bool) -> String {
    if name == "[*]" {
        if is_from {
            format!("{}{}", START_PREFIX, eff_scope)
        } else {
            format!("{}{}", END_PREFIX, eff_scope)
        }
    } else if name == "[H]" {
        format!("{}{}", HIST_PREFIX, comp_scope)
    } else if let Some(base) = name.strip_suffix("[H]") {
        format!("{}{}", HIST_PREFIX, base)
    } else {
        name.to_string()
    }
}

/// Parse `A --> B : label`; returns (from, to, label, rank_len).
fn parse_transition(line: &str) -> Option<(String, String, Option<String>, usize)> {
    let (line_part, label) = match line.split_once(" : ") {
        Some((l, lab)) => (l.trim_end(), Some(lab.trim().to_string())),
        None => (line, None),
    };
    let tokens: Vec<&str> = line_part.split_whitespace().collect();
    if tokens.len() != 3 {
        return None;
    }
    let arrow = normalize_arrow(tokens[1])?;
    let from = tokens[0].trim_matches('"').to_string();
    let to = tokens[2].trim_matches('"').to_string();
    Some((from, to, label, arrow))
}

/// Returns the rank length of an arrow token like `-->`, `->`, `-d->`,
/// `-left->`. None if the token is not a state arrow.
fn normalize_arrow(token: &str) -> Option<usize> {
    let body = token.strip_suffix('>')?;
    let mut t = body.to_string();
    for dir in ["left", "right", "up", "down", "le", "ri", "do", "l", "r", "u", "d"] {
        let with_dashes = format!("-{}-", dir);
        if t.contains(&with_dashes) {
            t = t.replace(&with_dashes, "--");
            break;
        }
    }
    if t.is_empty() || !t.chars().all(|c| c == '-' || c == '.') {
        return None;
    }
    Some(t.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple() {
        let d = parse("[*] --> Idle\nIdle --> Running : start\nRunning --> [*]\n").unwrap();
        assert_eq!(d.states.len(), 2);
        assert_eq!(d.transitions.len(), 3);
        assert_eq!(d.transitions[0].from, "start$");
        assert_eq!(d.transitions[0].to, "Idle");
        assert_eq!(d.transitions[1].label.as_deref(), Some("start"));
        assert_eq!(d.transitions[2].to, "end$");
    }

    #[test]
    fn test_description() {
        let d = parse("Idle : waiting for job\n").unwrap();
        assert_eq!(d.states[0].descriptions, vec!["waiting for job"]);
    }

    #[test]
    fn test_composite() {
        let d = parse(
            "state NotShooting {\n  [*] --> Idle\n  Idle --> Configuring : EvConfig\n}\nNotShooting --> Shooting\n",
        )
        .unwrap();
        let ns = d.state("NotShooting").unwrap();
        assert!(ns.composite);
        assert_eq!(d.state("Idle").unwrap().parent.as_deref(), Some("NotShooting"));
        assert_eq!(d.transitions[0].from, "start$NotShooting");
        assert!(d.state("Shooting").unwrap().parent.is_none());
    }

    #[test]
    fn test_same_rank_arrow() {
        let d = parse("A -> B\n").unwrap();
        assert_eq!(d.transitions[0].rank_len, 1);
        let d = parse("A --> B\n").unwrap();
        assert_eq!(d.transitions[0].rank_len, 2);
    }

    #[test]
    fn test_concurrent_regions() {
        let d = parse(
            "state Active {\n  [*] --> A1\n  A1 --> A2\n  --\n  [*] --> B1\n  B1 --> B2\n}\n",
        )
        .unwrap();
        let r0 = d.state("Active$0").unwrap();
        let r1 = d.state("Active$1").unwrap();
        assert!(r0.is_region && r1.is_region);
        assert_eq!(r0.parent.as_deref(), Some("Active"));
        // Children reparented into their regions.
        assert_eq!(d.state("A1").unwrap().parent.as_deref(), Some("Active$0"));
        assert_eq!(d.state("B1").unwrap().parent.as_deref(), Some("Active$1"));
        // Each region has its own start pseudo-state.
        assert_eq!(d.transitions[0].from, "start$Active$0");
        assert_eq!(d.transitions[2].from, "start$Active$1");
    }

    #[test]
    fn test_history() {
        let d = parse(
            "state W {\n  [*] --> E\n}\nW --> S : pause\nS --> W[H] : resume\n",
        )
        .unwrap();
        assert_eq!(d.transitions[2].to, "hist$W");
        // `[H]` must not be declared as a regular state.
        assert!(d.state("hist$W").is_none());
        assert!(d.state("W[H]").is_none());
    }

    #[test]
    fn test_history_in_scope() {
        let d = parse("state W {\n  X --> [H]\n}\n").unwrap();
        assert_eq!(d.transitions[0].to, "hist$W");
    }

    #[test]
    fn test_state_alias() {
        let d = parse("state \"Long Name\" as LN\nLN --> LN2\n").unwrap();
        assert_eq!(d.states[0].name, "LN");
        assert_eq!(d.states[0].display_name, "Long Name");
    }
}
