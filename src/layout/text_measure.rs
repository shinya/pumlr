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
    ///
    /// Uses per-character advance widths of DejaVu Sans (the metrics PlantUML's
    /// default sans-serif rendering closely matches), expressed as em fractions.
    pub fn measure_width(&self, text: &str) -> f32 {
        let mut width = 0.0f32;
        for ch in text.chars() {
            width += char_em_width(ch) * self.font_size;
        }
        width
    }

    /// Returns the line height for the current font size.
    /// PlantUML's line height is ≈1.18em (e.g. 14.13px for a 12px font).
    pub fn line_height(&self) -> f32 {
        self.font_size * 1.18
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

/// Advance width of a character as a fraction of the font size (em),
/// based on DejaVu Sans metrics. Non-ASCII characters (e.g. CJK) are
/// treated as full-width.
// 0.318 is DejaVu's space advance, not an approximation of 1/π.
#[allow(clippy::approx_constant)]
fn char_em_width(ch: char) -> f32 {
    match ch {
        ' ' => 0.318,
        '!' => 0.401,
        '"' => 0.460,
        '#' => 0.838,
        '$' => 0.636,
        '%' => 0.950,
        '&' => 0.780,
        '\'' => 0.275,
        '(' | ')' => 0.390,
        '*' => 0.500,
        '+' => 0.838,
        ',' => 0.318,
        '-' => 0.361,
        '.' => 0.318,
        '/' => 0.337,
        '0'..='9' => 0.636,
        ':' | ';' => 0.337,
        '<' | '=' | '>' => 0.838,
        '?' => 0.531,
        '@' => 1.000,
        'A' => 0.684,
        'B' => 0.686,
        'C' => 0.698,
        'D' => 0.770,
        'E' => 0.632,
        'F' => 0.575,
        'G' => 0.775,
        'H' => 0.752,
        'I' => 0.295,
        'J' => 0.295,
        'K' => 0.656,
        'L' => 0.557,
        'M' => 0.863,
        'N' => 0.748,
        'O' => 0.787,
        'P' => 0.603,
        'Q' => 0.787,
        'R' => 0.695,
        'S' => 0.635,
        'T' => 0.611,
        'U' => 0.732,
        'V' => 0.684,
        'W' => 0.989,
        'X' => 0.685,
        'Y' => 0.611,
        'Z' => 0.685,
        '[' | ']' => 0.390,
        '\\' => 0.337,
        '^' => 0.838,
        '_' => 0.500,
        '`' => 0.500,
        'a' => 0.613,
        'b' => 0.635,
        'c' => 0.550,
        'd' => 0.635,
        'e' => 0.615,
        'f' => 0.352,
        'g' => 0.635,
        'h' => 0.634,
        'i' => 0.278,
        'j' => 0.278,
        'k' => 0.579,
        'l' => 0.278,
        'm' => 0.974,
        'n' => 0.634,
        'o' => 0.612,
        'p' => 0.635,
        'q' => 0.635,
        'r' => 0.411,
        's' => 0.521,
        't' => 0.392,
        'u' => 0.634,
        'v' => 0.592,
        'w' => 0.818,
        'x' => 0.592,
        'y' => 0.592,
        'z' => 0.525,
        '{' | '}' => 0.636,
        '|' => 0.337,
        '~' => 0.838,
        c if c.is_ascii() => 0.600,
        // CJK and other wide characters
        _ => 1.000,
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
        assert!((m.line_height() - 14.0 * 1.18).abs() < 0.01);
    }

    #[test]
    fn test_multiline() {
        let m = TextMeasurer::new(14.0);
        let h = m.measure_multiline_height("line1\nline2\nline3");
        assert!((h - 3.0 * m.line_height()).abs() < 0.01);
    }
}
