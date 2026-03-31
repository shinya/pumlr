/// Simple text measurement without font loading.
///
/// For SVG output, we use an approximation based on average character widths.
/// This avoids the need to bundle font files while producing reasonable layouts.
/// The SVG viewer (browser) will handle actual text rendering with its own fonts.
pub struct TextMeasurer {
    font_size: f32,
}

impl TextMeasurer {
    pub fn new(font_size: f32) -> Self {
        Self { font_size }
    }

    /// Estimate the width of a text string in pixels.
    pub fn measure_width(&self, text: &str) -> f32 {
        // Average character width ratio for sans-serif fonts is ~0.6 of font size.
        // This is a reasonable approximation for layout purposes.
        let avg_char_width = self.font_size * 0.6;
        let mut width = 0.0f32;
        for ch in text.chars() {
            width += if ch.is_ascii_uppercase() || ch == 'W' || ch == 'M' {
                avg_char_width * 1.2
            } else if ch == 'i' || ch == 'l' || ch == '!' || ch == '|' || ch == '.' || ch == ' ' {
                avg_char_width * 0.5
            } else {
                avg_char_width
            };
        }
        width
    }

    /// Returns the line height for the current font size.
    pub fn line_height(&self) -> f32 {
        self.font_size * 1.4
    }

    /// Measure the width of the widest line in a multi-line text.
    pub fn measure_multiline_width(&self, text: &str) -> f32 {
        text.lines()
            .map(|line| self.measure_width(line))
            .fold(0.0f32, f32::max)
    }

    /// Measure the total height of multi-line text.
    pub fn measure_multiline_height(&self, text: &str) -> f32 {
        let line_count = text.lines().count().max(1) as f32;
        line_count * self.line_height()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_measure_width_nonempty() {
        let m = TextMeasurer::new(14.0);
        let w = m.measure_width("Hello");
        assert!(w > 0.0);
    }

    #[test]
    fn test_measure_width_empty() {
        let m = TextMeasurer::new(14.0);
        assert_eq!(m.measure_width(""), 0.0);
    }

    #[test]
    fn test_uppercase_wider() {
        let m = TextMeasurer::new(14.0);
        let upper = m.measure_width("HELLO");
        let lower = m.measure_width("hello");
        assert!(upper > lower);
    }

    #[test]
    fn test_line_height() {
        let m = TextMeasurer::new(14.0);
        assert!((m.line_height() - 19.6).abs() < 0.01);
    }

    #[test]
    fn test_multiline() {
        let m = TextMeasurer::new(14.0);
        let h = m.measure_multiline_height("line1\nline2\nline3");
        assert!((h - 3.0 * m.line_height()).abs() < 0.01);
    }
}
