pub mod ast;
pub mod diagram;
pub mod error;
pub mod layout;
pub mod parser;
pub mod preprocess;
pub mod render;
pub mod theme;

use std::path::PathBuf;

pub use diagram::DiagramType;
pub use error::PlantUmlError;

/// Rendering options.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Theme name (None = default).
    pub theme: Option<String>,
    /// Whether to embed fonts in the SVG.
    pub embed_fonts: bool,
    /// Dark mode.
    pub dark_mode: bool,
    /// Custom font file paths.
    pub font_paths: Vec<PathBuf>,
    /// Search paths for `!include` directives.
    pub include_paths: Vec<PathBuf>,
}

/// Render PlantUML text to SVG with default options.
pub fn render_svg(input: &str) -> Result<String, PlantUmlError> {
    render_svg_with_options(input, &RenderOptions::default())
}

/// Render PlantUML text to SVG with the given options.
pub fn render_svg_with_options(
    input: &str,
    _options: &RenderOptions,
) -> Result<String, PlantUmlError> {
    let preprocessed = preprocess::preprocess(input)?;

    let diagram_type = refine_diagram_type(preprocessed.diagram_type, &preprocessed.body);

    match diagram_type {
        DiagramType::Sequence => {
            let diagram = parser::sequence::parse(&preprocessed.body)?;
            let laid_out = layout::sequence::layout(&diagram);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Activity => {
            let diagram = parser::activity::parse(&preprocessed.body)?;
            let laid_out = layout::activity::layout(&diagram);
            Ok(render::svg::render(&laid_out))
        }
        other => Err(PlantUmlError::UnsupportedDiagram(format!("{:?}", other))),
    }
}

/// Detect the diagram type of the input text.
pub fn detect_diagram_type(input: &str) -> Option<DiagramType> {
    preprocess::detect_diagram_type(input)
}

/// Refine the diagram type based on body content.
///
/// `@startuml` is used for both sequence and activity diagrams.
/// We detect activity diagram syntax by looking for characteristic keywords.
fn refine_diagram_type(initial: DiagramType, body: &str) -> DiagramType {
    if initial != DiagramType::Sequence {
        return initial;
    }

    // Activity diagram indicators: these keywords appear at the start of a line
    // and are NOT valid in sequence diagrams.
    let activity_keywords = [
        "start", "stop", "if (", "if(", "while (", "while(",
        "fork", "switch (", "switch(", "partition ",
    ];

    for line in body.lines() {
        let trimmed = line.trim().to_lowercase();
        if trimmed.is_empty() {
            continue;
        }
        // Action syntax `:text;` is unique to activity diagrams
        if trimmed.starts_with(':') && trimmed.ends_with(';') {
            return DiagramType::Activity;
        }
        for kw in &activity_keywords {
            if trimmed.starts_with(kw) {
                return DiagramType::Activity;
            }
        }
    }

    DiagramType::Sequence
}
