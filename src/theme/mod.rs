pub mod classic;
pub mod default;

pub use classic::ClassicTheme;
pub use default::DefaultTheme;

/// Resolve a PlantUML color token (`#RRGGBB`, `#RGB`, or `#Name` like
/// `#LightBlue`) into a value usable as an SVG fill.
pub fn resolve_color(token: &str) -> String {
    let body = token.trim().trim_start_matches('#');
    if !body.is_empty() && body.chars().all(|c| c.is_ascii_hexdigit()) {
        format!("#{}", body)
    } else {
        // Named colors: SVG/CSS understands the standard names directly.
        body.to_ascii_lowercase()
    }
}

/// Theme trait for styling diagrams.
///
/// Sequence-diagram colors are required methods; activity-diagram colors have
/// defaults matching the PlantUML 1.2026 default style, so a theme only needs
/// to override them when it deviates.
pub trait Theme {
    // --- Shared / sequence diagram ---
    fn background_color(&self) -> &str;
    fn participant_bg_color(&self) -> &str;
    fn participant_border_color(&self) -> &str;
    fn participant_font_size(&self) -> f32;
    fn arrow_color(&self) -> &str;
    fn lifeline_color(&self) -> &str;
    fn note_bg_color(&self) -> &str;
    fn note_border_color(&self) -> &str;
    fn font_family(&self) -> &str;
    fn font_size(&self) -> f32;
    fn group_border_color(&self) -> &str;
    fn group_bg_color(&self) -> &str;
    fn group_label_bg_color(&self) -> &str;
    fn separator_color(&self) -> &str;
    fn activation_bg_color(&self) -> &str;
    fn activation_border_color(&self) -> &str;

    // --- Activity diagram ---
    /// Fill of actions, condition hexagons, and merge diamonds.
    fn activity_shape_fill(&self) -> &str {
        "#F1F1F1"
    }
    /// Stroke of actions, condition hexagons, and merge diamonds.
    fn activity_shape_stroke(&self) -> &str {
        "#181818"
    }
    fn activity_shape_stroke_width(&self) -> f32 {
        0.5
    }
    /// Start/stop circles and fork bars.
    fn activity_start_stop_color(&self) -> &str {
        "#222222"
    }
    /// Flow arrows and connector lines.
    fn activity_edge_color(&self) -> &str {
        "#181818"
    }
    fn activity_text_color(&self) -> &str {
        "#000000"
    }
    fn activity_note_fill(&self) -> &str {
        "#FEFFDD"
    }
    /// Partition frame stroke.
    fn partition_border_color(&self) -> &str {
        "#000000"
    }
}
