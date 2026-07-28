use super::Theme;

/// PlantUML 1.2026 default style (values extracted from reference SVG output,
/// see SPEC.md).
pub struct DefaultTheme;

impl Theme for DefaultTheme {
    fn background_color(&self) -> &str {
        "white"
    }
    fn participant_bg_color(&self) -> &str {
        "#E2E2F0"
    }
    fn participant_border_color(&self) -> &str {
        "#181818"
    }
    fn participant_font_size(&self) -> f32 {
        14.0
    }
    fn arrow_color(&self) -> &str {
        "#181818"
    }
    fn lifeline_color(&self) -> &str {
        "#181818"
    }
    fn note_bg_color(&self) -> &str {
        "#FEFFDD"
    }
    fn note_border_color(&self) -> &str {
        "#181818"
    }
    fn font_family(&self) -> &str {
        "sans-serif"
    }
    fn font_size(&self) -> f32 {
        13.0
    }
    fn group_border_color(&self) -> &str {
        "#000000"
    }
    fn group_bg_color(&self) -> &str {
        "#EEEEEE"
    }
    fn group_label_bg_color(&self) -> &str {
        "#EEEEEE"
    }
    fn separator_color(&self) -> &str {
        "#000000"
    }
    fn activation_bg_color(&self) -> &str {
        "#FFFFFF"
    }
    fn activation_border_color(&self) -> &str {
        "#181818"
    }
}
