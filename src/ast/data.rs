/// AST for JSON / YAML data diagrams: a generic data value tree.

#[derive(Debug, Clone, PartialEq)]
pub enum DataValue {
    Null,
    Bool(bool),
    /// Numbers keep their source text (no float rounding surprises).
    Number(String),
    String(String),
    Array(Vec<DataValue>),
    Object(Vec<(String, DataValue)>),
}

#[derive(Debug, Clone)]
pub struct DataDiagram {
    pub title: Option<String>,
    pub root: DataValue,
}

impl DataValue {
    pub fn is_scalar(&self) -> bool {
        !matches!(self, DataValue::Array(_) | DataValue::Object(_))
    }

    /// Display text for scalar values.
    pub fn scalar_text(&self) -> String {
        match self {
            DataValue::Null => "null".to_string(),
            DataValue::Bool(b) => b.to_string(),
            DataValue::Number(n) => n.clone(),
            DataValue::String(s) => s.clone(),
            _ => String::new(),
        }
    }
}
