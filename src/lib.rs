//! A pure-Rust PlantUML renderer — generates SVG from PlantUML text without
//! Java or Graphviz.
//!
//! Sequence diagrams and activity diagrams are supported; the output is
//! matched against the Java PlantUML 1.2026 default style (see the project
//! README for the supported syntax and the comparison workflow).
//!
//! ```
//! let svg = pumlr::render_svg("@startuml\nAlice -> Bob : Hello\n@enduml").unwrap();
//! assert!(svg.starts_with("<svg"));
//! ```

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
    /// Theme name: `None` or `"default"` for the PlantUML 1.2026 default
    /// style, `"classic"` for the pre-2023 pale-yellow style.
    pub theme: Option<String>,
    /// Whether to embed fonts in the SVG. Not implemented yet.
    pub embed_fonts: bool,
    /// Dark mode. Not implemented yet.
    pub dark_mode: bool,
    /// Custom font file paths. Not implemented yet.
    pub font_paths: Vec<PathBuf>,
    /// Search paths for `!include` directives. Not implemented yet.
    pub include_paths: Vec<PathBuf>,
}

/// Render PlantUML text to SVG with default options.
pub fn render_svg(input: &str) -> Result<String, PlantUmlError> {
    render_svg_with_options(input, &RenderOptions::default())
}

/// Render PlantUML text to SVG with the given options.
///
/// `options.theme` selects the visual style: `None` or `"default"` renders
/// the PlantUML 1.2026 default look, `"classic"` the pre-2023 pale-yellow
/// look. Unknown names fall back to the default theme.
pub fn render_svg_with_options(
    input: &str,
    options: &RenderOptions,
) -> Result<String, PlantUmlError> {
    let preprocessed = preprocess::preprocess(input)?;

    let diagram_type = refine_diagram_type(preprocessed.diagram_type, &preprocessed.body);

    let theme: &dyn theme::Theme = match options.theme.as_deref() {
        Some(name) if name.eq_ignore_ascii_case("classic") => &theme::ClassicTheme,
        _ => &theme::DefaultTheme,
    };

    match diagram_type {
        DiagramType::Sequence => {
            let diagram = parser::sequence::parse(&preprocessed.body)?;
            let laid_out = layout::sequence::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Activity => {
            let diagram = parser::activity::parse(&preprocessed.body)?;
            let laid_out = layout::activity::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Class => {
            let diagram = parser::class_diagram::parse(&preprocessed.body)?;
            let laid_out = layout::class_diagram::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::State => {
            let diagram = parser::state::parse(&preprocessed.body)?;
            let laid_out = layout::state::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::UseCase => {
            let diagram = parser::usecase::parse(&preprocessed.body)?;
            let laid_out = layout::usecase::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Component => {
            let diagram = parser::component::parse(&preprocessed.body)?;
            let laid_out = layout::component::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::MindMap => {
            let diagram = parser::tree::parse(&preprocessed.body)?;
            let laid_out = layout::mindmap::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Wbs => {
            let diagram = parser::tree::parse(&preprocessed.body)?;
            let laid_out = layout::wbs::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Gantt => {
            let diagram = parser::gantt::parse(&preprocessed.body)?;
            let laid_out = layout::gantt::layout_with_theme(&diagram, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Json => {
            let diagram = parser::data::parse_json(&preprocessed.body)?;
            let laid_out = layout::data::layout_with_theme(&diagram, true, theme);
            Ok(render::svg::render(&laid_out))
        }
        DiagramType::Yaml => {
            let diagram = parser::data::parse_yaml(&preprocessed.body)?;
            let laid_out = layout::data::layout_with_theme(&diagram, false, theme);
            Ok(render::svg::render(&laid_out))
        }
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

    // Component diagram indicators: `[Name]` bracket components or the
    // `component` keyword.
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("component ") {
            return DiagramType::Component;
        }
        if trimmed.starts_with('[') && trimmed.contains(']') && !trimmed.starts_with("[*]") {
            return DiagramType::Component;
        }
        // `... [X]` as an arrow target.
        let arrow_part = trimmed.split(" : ").next().unwrap_or(trimmed);
        if (arrow_part.contains("->") || arrow_part.contains("--"))
            && arrow_part.split_whitespace().any(|tok| {
                tok.starts_with('[') && tok.ends_with(']') && tok != "[*]"
            })
        {
            return DiagramType::Component;
        }
    }

    // Use case diagram indicators: `usecase` declarations, or arrow lines
    // whose endpoints use `(text)` / `:name:` forms.
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("usecase ") {
            return DiagramType::UseCase;
        }
        let arrow_part = trimmed.split(" : ").next().unwrap_or(trimmed);
        if arrow_part.contains("->") || arrow_part.contains("--") {
            let has_uc_endpoint = arrow_part.split_whitespace().any(|tok| {
                (tok.starts_with('(') && tok.ends_with(')'))
                    || (tok.len() >= 3 && tok.starts_with(':') && tok.ends_with(':'))
            }) || (arrow_part.contains('(') && arrow_part.contains(')'));
            if has_uc_endpoint && !arrow_part.contains("[*]") {
                return DiagramType::UseCase;
            }
        }
    }

    // State diagram indicators: `[*]` pseudo states or `state` declarations.
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("[*]")
            || trimmed.contains("--> [*]")
            || trimmed.contains("-> [*]")
            || trimmed.starts_with("state ")
        {
            return DiagramType::State;
        }
    }

    // Class diagram indicators: declarations or class-style relation arrows.
    let class_keywords = ["class ", "abstract class ", "interface ", "enum "];
    let class_arrows = ["<|--", "--|>", "<|..", "..|>", "*--", "--*", "o--", "--o"];
    for line in body.lines() {
        let trimmed = line.trim();
        if class_keywords.iter().any(|kw| trimmed.starts_with(kw))
            || class_arrows.iter().any(|a| trimmed.contains(a))
        {
            return DiagramType::Class;
        }
    }

    // Activity diagram indicators: these keywords appear at the start of a line
    // and are NOT valid in sequence diagrams.
    let activity_keywords = [
        "start",
        "stop",
        "if (",
        "if(",
        "while (",
        "while(",
        "repeat",
        "backward",
        "fork",
        "switch (",
        "switch(",
        "partition ",
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
