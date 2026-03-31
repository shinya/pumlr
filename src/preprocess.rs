use crate::diagram::DiagramType;
use crate::error::PlantUmlError;

/// Result of preprocessing a PlantUML input.
#[derive(Debug)]
pub struct PreprocessedInput {
    /// The diagram type detected from the start directive.
    pub diagram_type: DiagramType,
    /// The body lines between @start and @end, with comments removed.
    pub body: String,
}

/// Extract the diagram body from a PlantUML input.
///
/// Detects `@startuml` / `@startmindmap` etc., strips comments,
/// and returns the cleaned body with the detected diagram type.
pub fn preprocess(input: &str) -> Result<PreprocessedInput, PlantUmlError> {
    let (diagram_type, start_idx, end_idx) = find_diagram_bounds(input)?;
    let body = extract_body(input, start_idx, end_idx);
    Ok(PreprocessedInput {
        diagram_type,
        body,
    })
}

/// Detect the diagram type from the input text without full preprocessing.
pub fn detect_diagram_type(input: &str) -> Option<DiagramType> {
    for line in input.lines() {
        let trimmed = line.trim();
        if let Some(dt) = parse_start_directive(trimmed) {
            return Some(dt);
        }
    }
    None
}

fn find_diagram_bounds(input: &str) -> Result<(DiagramType, usize, usize), PlantUmlError> {
    let lines: Vec<&str> = input.lines().collect();

    let mut diagram_type = None;
    let mut start_idx = None;
    let mut end_idx = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();

        if start_idx.is_none() {
            if let Some(dt) = parse_start_directive(trimmed) {
                diagram_type = Some(dt);
                start_idx = Some(i);
            }
        } else if is_end_directive(trimmed) {
            end_idx = Some(i);
            break;
        }
    }

    let start = start_idx
        .ok_or_else(|| PlantUmlError::PreprocessError("no @start directive found".into()))?;
    let end = end_idx.unwrap_or(lines.len());
    let dt = diagram_type.unwrap();

    Ok((dt, start, end))
}

fn parse_start_directive(line: &str) -> Option<DiagramType> {
    let lower = line.to_lowercase();
    if lower.starts_with("@startuml") {
        Some(DiagramType::Sequence) // default; actual type determined by parser
    } else if lower.starts_with("@startmindmap") {
        Some(DiagramType::MindMap)
    } else if lower.starts_with("@startwbs") {
        Some(DiagramType::Wbs)
    } else if lower.starts_with("@startgantt") {
        Some(DiagramType::Gantt)
    } else if lower.starts_with("@startjson") {
        Some(DiagramType::Json)
    } else if lower.starts_with("@startyaml") {
        Some(DiagramType::Yaml)
    } else {
        None
    }
}

fn is_end_directive(line: &str) -> bool {
    let lower = line.to_lowercase();
    lower.starts_with("@end")
}

fn extract_body(input: &str, start_idx: usize, end_idx: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let body_lines = &lines[start_idx + 1..end_idx];

    let mut result = Vec::new();
    let mut in_block_comment = false;

    for line in body_lines {
        let trimmed = line.trim();

        if in_block_comment {
            if trimmed.ends_with("'/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("/'") {
            if !trimmed.ends_with("'/") {
                in_block_comment = true;
            }
            continue;
        }

        // Remove single-line comments (lines starting with ')
        if trimmed.starts_with('\'') {
            continue;
        }

        result.push(*line);
    }

    result.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_startuml() {
        let input = "@startuml\nAlice -> Bob\n@enduml";
        let result = preprocess(input).unwrap();
        assert_eq!(result.diagram_type, DiagramType::Sequence);
        assert_eq!(result.body, "Alice -> Bob");
    }

    #[test]
    fn test_detect_startmindmap() {
        let input = "@startmindmap\n* root\n@endmindmap";
        let result = preprocess(input).unwrap();
        assert_eq!(result.diagram_type, DiagramType::MindMap);
        assert_eq!(result.body, "* root");
    }

    #[test]
    fn test_no_start_directive() {
        let input = "Alice -> Bob";
        let result = preprocess(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_single_line_comment() {
        let input = "@startuml\n' this is a comment\nAlice -> Bob\n@enduml";
        let result = preprocess(input).unwrap();
        assert_eq!(result.body, "Alice -> Bob");
    }

    #[test]
    fn test_strip_block_comment() {
        let input = "@startuml\n/'\nthis is\na block comment\n'/\nAlice -> Bob\n@enduml";
        let result = preprocess(input).unwrap();
        assert_eq!(result.body, "Alice -> Bob");
    }

    #[test]
    fn test_strip_inline_block_comment() {
        let input = "@startuml\n/' inline '/\nAlice -> Bob\n@enduml";
        let result = preprocess(input).unwrap();
        assert_eq!(result.body, "Alice -> Bob");
    }

    #[test]
    fn test_no_enduml_takes_until_end() {
        let input = "@startuml\nAlice -> Bob\nBob -> Alice";
        let result = preprocess(input).unwrap();
        assert_eq!(result.body, "Alice -> Bob\nBob -> Alice");
    }

    #[test]
    fn test_leading_whitespace_before_directive() {
        let input = "  @startuml  \nAlice -> Bob\n  @enduml  ";
        let result = preprocess(input).unwrap();
        assert_eq!(result.body, "Alice -> Bob");
    }

    #[test]
    fn test_detect_diagram_type_function() {
        assert_eq!(detect_diagram_type("@startuml\n"), Some(DiagramType::Sequence));
        assert_eq!(detect_diagram_type("@startmindmap\n"), Some(DiagramType::MindMap));
        assert_eq!(detect_diagram_type("@startwbs\n"), Some(DiagramType::Wbs));
        assert_eq!(detect_diagram_type("hello"), None);
    }
}
