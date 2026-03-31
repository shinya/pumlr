pub mod default;

pub use default::DefaultTheme;

/// Theme trait for styling diagrams.
pub trait Theme {
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
}
