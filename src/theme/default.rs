use super::Theme;

pub struct DefaultTheme;

impl Theme for DefaultTheme {
    fn background_color(&self) -> &str { "white" }
    fn participant_bg_color(&self) -> &str { "#FEFECE" }
    fn participant_border_color(&self) -> &str { "#A80036" }
    fn participant_font_size(&self) -> f32 { 14.0 }
    fn arrow_color(&self) -> &str { "#A80036" }
    fn lifeline_color(&self) -> &str { "#A80036" }
    fn note_bg_color(&self) -> &str { "#FBFB77" }
    fn note_border_color(&self) -> &str { "#A80036" }
    fn font_family(&self) -> &str { "sans-serif" }
    fn font_size(&self) -> f32 { 13.0 }
    fn group_border_color(&self) -> &str { "#A80036" }
    fn group_bg_color(&self) -> &str { "#EEEEEE" }
    fn group_label_bg_color(&self) -> &str { "#EEEEEE" }
    fn separator_color(&self) -> &str { "#A80036" }
    fn activation_bg_color(&self) -> &str { "#FEFECE" }
    fn activation_border_color(&self) -> &str { "#A80036" }
}
