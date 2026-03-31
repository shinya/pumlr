use thiserror::Error;

/// Errors that can occur during PlantUML processing.
#[derive(Debug, Error)]
pub enum PlantUmlError {
    #[error("parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    #[error("layout error: {0}")]
    LayoutError(String),

    #[error("render error: {0}")]
    RenderError(String),

    #[error("unsupported diagram type: {0}")]
    UnsupportedDiagram(String),

    #[error("preprocess error: {0}")]
    PreprocessError(String),
}
