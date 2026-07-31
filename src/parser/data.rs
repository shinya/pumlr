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
    let lines: Vec<(usize, String)> = body
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim().starts_with('#'))
        .map(|l| {
            let indent = l.len() - l.trim_start().len();
            (indent, l.trim().to_string())
        })
        .collect();
    let mut pos = 0;
    let root = yaml_block(&lines, &mut pos, 0)?;
    Ok(DataDiagram { title: None, root })
}

fn yaml_block(
    lines: &[(usize, String)],
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
            let item = lines[*pos].1.trim_start_matches('-').trim().to_string();
            *pos += 1;
            if item.is_empty() {
                // Nested block under the dash.
                let next_indent = lines.get(*pos).map(|(i, _)| *i).unwrap_or(indent);
                items.push(yaml_block(lines, pos, next_indent)?);
            } else {
                items.push(yaml_scalar(&item));
            }
        }
        return Ok(DataValue::Array(items));
    }

    // Map block.
    let mut entries = Vec::new();
    while *pos < lines.len() && lines[*pos].0 == indent {
        let line = &lines[*pos].1;
        let Some((key, rest)) = line.split_once(':') else {
            return Err(PlantUmlError::ParseError {
                line: *pos + 1,
                message: format!("YAML: expected 'key: value', got '{}'", line),
            });
        };
        let key = key.trim().trim_matches('"').to_string();
        let rest = rest.trim();
        *pos += 1;
        if rest.is_empty() {
            // Nested block (map or list) with deeper indent.
            if *pos < lines.len() && lines[*pos].0 > indent {
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
}
