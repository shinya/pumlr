/// Parsers for `@startjson` (full JSON) and `@startyaml` (a practical YAML
/// subset: nested maps, scalar lists, scalars).
use crate::ast::data::*;
use crate::error::PlantUmlError;

// ---------- JSON ----------

pub fn parse_json(body: &str) -> Result<DataDiagram, PlantUmlError> {
    let mut p = JsonParser {
        chars: body.chars().collect(),
        pos: 0,
    };
    p.skip_ws();
    let root = p.value()?;
    Ok(DataDiagram { title: None, root })
}

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn error(&self, message: &str) -> PlantUmlError {
        PlantUmlError::ParseError {
            line: 0,
            message: format!("JSON: {} (at offset {})", message, self.pos),
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, c: char) -> Result<(), PlantUmlError> {
        self.skip_ws();
        if self.bump() == Some(c) {
            Ok(())
        } else {
            Err(self.error(&format!("expected '{}'", c)))
        }
    }

    fn value(&mut self) -> Result<DataValue, PlantUmlError> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => Ok(DataValue::String(self.string()?)),
            Some('t') => self.literal("true", DataValue::Bool(true)),
            Some('f') => self.literal("false", DataValue::Bool(false)),
            Some('n') => self.literal("null", DataValue::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.number(),
            _ => Err(self.error("unexpected character")),
        }
    }

    fn literal(&mut self, word: &str, value: DataValue) -> Result<DataValue, PlantUmlError> {
        for expected in word.chars() {
            if self.bump() != Some(expected) {
                return Err(self.error(&format!("invalid literal, expected '{}'", word)));
            }
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<DataValue, PlantUmlError> {
        let start = self.pos;
        while matches!(
            self.peek(),
            Some(c) if c.is_ascii_digit() || matches!(c, '-' | '+' | '.' | 'e' | 'E')
        ) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.is_empty() {
            return Err(self.error("invalid number"));
        }
        Ok(DataValue::Number(text))
    }

    fn string(&mut self) -> Result<String, PlantUmlError> {
        self.expect('"')?;
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(out),
                Some('\\') => match self.bump() {
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some('r') => out.push('\r'),
                    Some('u') => {
                        let mut code = 0u32;
                        for _ in 0..4 {
                            let d = self
                                .bump()
                                .and_then(|c| c.to_digit(16))
                                .ok_or_else(|| self.error("invalid \\u escape"))?;
                            code = code * 16 + d;
                        }
                        out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                    }
                    Some(c) => out.push(c),
                    None => return Err(self.error("unterminated string")),
                },
                Some(c) => out.push(c),
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn object(&mut self) -> Result<DataValue, PlantUmlError> {
        self.expect('{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(DataValue::Object(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.expect(':')?;
            let value = self.value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some('}') => return Ok(DataValue::Object(entries)),
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self) -> Result<DataValue, PlantUmlError> {
        self.expect('[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.pos += 1;
            return Ok(DataValue::Array(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(DataValue::Array(items)),
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }
}

// ---------- YAML (subset) ----------

pub fn parse_yaml(body: &str) -> Result<DataDiagram, PlantUmlError> {
    let mut lines = yaml_logical_lines(body);
    let mut pos = 0;
    let root = yaml_block(&mut lines, &mut pos, 0)?;
    Ok(DataDiagram { title: None, root })
}

/// Block scalar indicator: `|` (literal) or `>` (folded), optionally with a
/// chomping suffix (`|-`, `|+`, `>-`, `>+`).
fn block_scalar_indicator(s: &str) -> Option<bool> {
    match s {
        "|" | "|-" | "|+" => Some(false),
        ">" | ">-" | ">+" => Some(true),
        _ => None,
    }
}

/// Fold lines per YAML folded-scalar rules: adjacent lines joined with a
/// space, blank lines become a newline.
fn fold_lines(content: &[String]) -> String {
    let mut out = String::new();
    for (i, line) in content.iter().enumerate() {
        if i == 0 {
            out.push_str(line);
        } else if line.is_empty() {
            out.push('\n');
        } else if out.is_empty() || out.ends_with('\n') {
            out.push_str(line);
        } else {
            out.push(' ');
            out.push_str(line);
        }
    }
    out
}

/// Turn the raw body into logical lines of `(indent, trimmed_text)`.
/// Block scalars (`key: |` / `key: >`) are collapsed into a single logical
/// `key: <value>` line whose value may contain embedded newlines.
fn yaml_logical_lines(body: &str) -> Vec<(usize, String)> {
    let raw: Vec<&str> = body.lines().collect();
    let mut lines: Vec<(usize, String)> = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        let l = raw[i];
        let trimmed = l.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            i += 1;
            continue;
        }
        let indent = l.len() - l.trim_start().len();
        if let Some((key, rest)) = trimmed.split_once(':') {
            if let Some(folded) = block_scalar_indicator(rest.trim()) {
                i += 1;
                // Collect content lines indented deeper than the key.
                let mut content: Vec<String> = Vec::new();
                let mut content_indent: Option<usize> = None;
                while i < raw.len() {
                    let cl = raw[i];
                    if cl.trim().is_empty() {
                        content.push(String::new());
                        i += 1;
                        continue;
                    }
                    let ci = cl.len() - cl.trim_start().len();
                    if ci <= indent {
                        break;
                    }
                    let keep = *content_indent.get_or_insert(ci);
                    content.push(cl.get(keep.min(cl.len())..).unwrap_or("").to_string());
                    i += 1;
                }
                while content.last().is_some_and(|s| s.is_empty()) {
                    content.pop();
                }
                let value = if folded {
                    fold_lines(&content)
                } else {
                    content.join("\n")
                };
                lines.push((indent, format!("{}: {}", key, value)));
                continue;
            }
        }
        lines.push((indent, trimmed.to_string()));
        i += 1;
    }
    lines
}

/// A list item like `- name: x` opens an inline map; a plain scalar does not.
fn is_map_entry(item: &str) -> bool {
    item.split_once(':')
        .is_some_and(|(_, rest)| rest.is_empty() || rest.starts_with(' '))
}

/// A bare `&anchor` token (no content after the anchor name).
fn is_bare_anchor(rest: &str) -> bool {
    rest.starts_with('&') && rest.len() > 1 && !rest.contains(' ')
}

fn yaml_block(
    lines: &mut [(usize, String)],
    pos: &mut usize,
    indent: usize,
) -> Result<DataValue, PlantUmlError> {
    if *pos >= lines.len() {
        return Ok(DataValue::Null);
    }
    // List block?
    if lines[*pos].1.starts_with("- ") || lines[*pos].1 == "-" {
        let mut items = Vec::new();
        while *pos < lines.len()
            && lines[*pos].0 == indent
            && (lines[*pos].1.starts_with("- ") || lines[*pos].1 == "-")
        {
            let line = lines[*pos].1.clone();
            let after_dash = line.strip_prefix('-').unwrap_or("");
            let item = after_dash.trim_start();
            // Column where the item's content starts, e.g. `- name: x` -> +2.
            let content_indent = indent + (line.len() - item.len());
            let item = item.trim_end().to_string();
            if item.is_empty() {
                *pos += 1;
                // Nested block under the dash.
                let next_indent = lines.get(*pos).map(|(i, _)| *i).unwrap_or(indent);
                items.push(yaml_block(lines, pos, next_indent)?);
            } else if is_map_entry(&item) {
                // `- key: ...` starts an inline map item; re-tag this line as
                // the map's first entry and parse the whole item as a map.
                lines[*pos] = (content_indent, item);
                items.push(yaml_block(lines, pos, content_indent)?);
            } else {
                *pos += 1;
                items.push(yaml_scalar(&item));
            }
        }
        return Ok(DataValue::Array(items));
    }

    // Map block.
    let mut entries = Vec::new();
    while *pos < lines.len() && lines[*pos].0 == indent {
        let line = lines[*pos].1.clone();
        let Some((key, rest)) = line.split_once(':') else {
            return Err(PlantUmlError::ParseError {
                line: *pos + 1,
                message: format!("YAML: expected 'key: value', got '{}'", line),
            });
        };
        let key = key.trim().trim_matches('"').to_string();
        let rest = rest.trim();
        *pos += 1;
        let has_deeper_block = *pos < lines.len() && lines[*pos].0 > indent;
        if rest.is_empty() || (is_bare_anchor(rest) && has_deeper_block) {
            // Nested block (map or list) with deeper indent. A bare anchor
            // (`key: &name`) introducing a block is treated the same way;
            // the anchor label itself is not displayed (the Java PlantUML
            // reference crashes on this construct).
            if has_deeper_block {
                let next_indent = lines[*pos].0;
                entries.push((key, yaml_block(lines, pos, next_indent)?));
            } else {
                entries.push((key, DataValue::Null));
            }
        } else {
            entries.push((key, yaml_scalar(rest)));
        }
    }
    Ok(DataValue::Object(entries))
}

fn yaml_scalar(s: &str) -> DataValue {
    match s {
        "true" | "True" => DataValue::Bool(true),
        "false" | "False" => DataValue::Bool(false),
        "null" | "~" => DataValue::Null,
        _ => {
            let unquoted = s.trim_matches('"').trim_matches('\'');
            if s.parse::<f64>().is_ok() && !s.contains(' ') {
                DataValue::Number(s.to_string())
            } else {
                DataValue::String(unquoted.to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_object() {
        let d = parse_json(r#"{"name": "pumlr", "n": 42, "ok": true, "none": null}"#).unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].1, DataValue::String("pumlr".into()));
        assert_eq!(entries[1].1, DataValue::Number("42".into()));
        assert_eq!(entries[2].1, DataValue::Bool(true));
        assert_eq!(entries[3].1, DataValue::Null);
    }

    #[test]
    fn test_json_nested() {
        let d = parse_json(r#"{"a": [1, 2], "b": {"c": "d"}}"#).unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert!(matches!(&entries[0].1, DataValue::Array(v) if v.len() == 2));
        assert!(matches!(&entries[1].1, DataValue::Object(o) if o.len() == 1));
    }

    #[test]
    fn test_json_escapes() {
        let d = parse_json(r#"{"s": "a\nbA"}"#).unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("a\nbA".into()));
    }

    #[test]
    fn test_yaml_basic() {
        let d = parse_yaml("name: pumlr\nversion: 0.1.0\nactive: true\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("pumlr".into()));
        // "0.1.0" is not a valid number; it stays a plain string.
        assert_eq!(entries[1].1, DataValue::String("0.1.0".into()));
        assert_eq!(entries[2].1, DataValue::Bool(true));
    }

    #[test]
    fn test_yaml_nested() {
        let d = parse_yaml("features:\n  - a\n  - b\nauthor:\n  name: s\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert!(matches!(&entries[0].1, DataValue::Array(v) if v.len() == 2));
        assert!(matches!(&entries[1].1, DataValue::Object(o) if o.len() == 1));
    }

    #[test]
    fn test_yaml_literal_block() {
        let d = parse_yaml("lit: |\n  aa\n  bb\n  cc\nafter: x\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("aa\nbb\ncc".into()));
        assert_eq!(entries[1].0, "after");
        assert_eq!(entries[1].1, DataValue::String("x".into()));
    }

    #[test]
    fn test_yaml_folded_block() {
        let d = parse_yaml("fold: >\n  aa\n  bb\nafter: x\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("aa bb".into()));
        assert_eq!(entries[1].0, "after");
    }

    #[test]
    fn test_yaml_folded_blank_line_becomes_newline() {
        let d = parse_yaml("fold: >\n  aa\n  bb\n\n  cc\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("aa bb\ncc".into()));
    }

    #[test]
    fn test_yaml_block_chomping_indicators() {
        let d = parse_yaml("a: |-\n  x\n  y\nb: >-\n  x\n  y\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("x\ny".into()));
        assert_eq!(entries[1].1, DataValue::String("x y".into()));
    }

    #[test]
    fn test_yaml_literal_block_keeps_comment_like_lines() {
        // '#' inside a block scalar is content, not a comment.
        let d = parse_yaml("lit: |\n  # not a comment\n  bb\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("# not a comment\nbb".into()));
    }

    #[test]
    fn test_yaml_scalar_anchor_kept_literal() {
        // PlantUML shows anchors/aliases literally (no resolution).
        let d = parse_yaml("name: &n pumlr\nalias: *n\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries[0].1, DataValue::String("&n pumlr".into()));
        assert_eq!(entries[1].1, DataValue::String("*n".into()));
    }

    #[test]
    fn test_yaml_map_anchor_opens_block() {
        // `key: &name` followed by a deeper block nests the block; the Java
        // reference crashes here, so we pick the useful behavior.
        let d = parse_yaml("base: &b\n  host: localhost\n  port: 5432\ncopy: *b\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries.len(), 2);
        let DataValue::Object(base) = &entries[0].1 else {
            panic!("expected nested map, got {:?}", entries[0].1)
        };
        assert_eq!(base.len(), 2);
        assert_eq!(base[0].1, DataValue::String("localhost".into()));
        assert_eq!(entries[1].1, DataValue::String("*b".into()));
    }

    #[test]
    fn test_yaml_list_of_maps() {
        let d = parse_yaml(
            "users:\n  - name: alice\n    role: admin\n  - name: bob\n    role: dev\ntags:\n  - x\n",
        )
        .unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        assert_eq!(entries.len(), 2);
        let DataValue::Array(users) = &entries[0].1 else {
            panic!("expected array, got {:?}", entries[0].1)
        };
        assert_eq!(users.len(), 2);
        let DataValue::Object(alice) = &users[0] else {
            panic!("expected map item, got {:?}", users[0])
        };
        assert_eq!(alice[0], ("name".into(), DataValue::String("alice".into())));
        assert_eq!(alice[1], ("role".into(), DataValue::String("admin".into())));
        let DataValue::Object(bob) = &users[1] else {
            panic!()
        };
        assert_eq!(bob[0].1, DataValue::String("bob".into()));
        assert!(matches!(&entries[1].1, DataValue::Array(v) if v.len() == 1));
    }

    #[test]
    fn test_yaml_list_scalar_with_colon_not_a_map() {
        // A URL-ish scalar must not be mistaken for a map entry.
        let d = parse_yaml("links:\n  - http://example.com\n").unwrap();
        let DataValue::Object(entries) = &d.root else {
            panic!()
        };
        let DataValue::Array(items) = &entries[0].1 else {
            panic!()
        };
        assert_eq!(items[0], DataValue::String("http://example.com".into()));
    }
}
