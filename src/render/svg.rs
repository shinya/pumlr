use crate::render::primitives::*;

/// A laid-out diagram ready to be rendered to SVG.
pub struct LaidOutDiagram {
    pub width: f32,
    pub height: f32,
    pub primitives: Vec<Primitive>,
}

/// Render a laid-out diagram to an SVG string.
pub fn render(diagram: &LaidOutDiagram) -> String {
    let mut svg = SvgBuilder::new(diagram.width, diagram.height);

    for prim in &diagram.primitives {
        match prim {
            Primitive::Rect(r) => svg.rect(r),
            Primitive::Line(l) => svg.line(l),
            Primitive::DashedLine(dl) => svg.dashed_line(dl),
            Primitive::Text(t) => svg.text(t),
            Primitive::Arrow(a) => svg.arrow(a),
            Primitive::Polygon(p) => svg.polygon(p),
            Primitive::Path(p) => svg.path(p),
        }
    }

    svg.finish()
}

struct SvgBuilder {
    content: String,
}

impl SvgBuilder {
    fn new(width: f32, height: f32) -> Self {
        let mut content = String::with_capacity(4096);
        content.push_str(&format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}" width="{}" height="{}">"#,
            width, height, width, height,
        ));
        content.push('\n');
        // Background
        content.push_str(&format!(
            r#"<rect width="{}" height="{}" fill="white"/>"#,
            width, height,
        ));
        content.push('\n');
        Self { content }
    }

    fn rect(&mut self, r: &Rect) {
        self.content.push_str(&format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}""#,
            r.x, r.y, r.width, r.height, r.fill, r.stroke, r.stroke_width,
        ));
        if r.rx > 0.0 || r.ry > 0.0 {
            self.content
                .push_str(&format!(r#" rx="{}" ry="{}""#, r.rx, r.ry));
        }
        self.content.push_str("/>\n");
    }

    fn line(&mut self, l: &Line) {
        self.content.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
            l.x1, l.y1, l.x2, l.y2, l.stroke, l.stroke_width,
        ));
        self.content.push('\n');
    }

    fn dashed_line(&mut self, dl: &DashedLine) {
        self.content.push_str(&format!(
            r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-dasharray="{}"/>"#,
            dl.x1, dl.y1, dl.x2, dl.y2, dl.stroke, dl.stroke_width, dl.dash_array,
        ));
        self.content.push('\n');
    }

    fn text(&mut self, t: &Text) {
        let anchor = match t.anchor {
            TextAnchor::Start => "start",
            TextAnchor::Middle => "middle",
            TextAnchor::End => "end",
        };
        let weight = if t.bold { r#" font-weight="bold""# } else { "" };
        self.content.push_str(&format!(
            r#"<text x="{}" y="{}" font-size="{}" font-family="{}" fill="{}" text-anchor="{}"{}>"#,
            t.x,
            t.y,
            t.font_size,
            xml_escape(&t.font_family),
            t.fill,
            anchor,
            weight,
        ));
        // Handle multi-line text
        let lines: Vec<&str> = t.content.lines().collect();
        if lines.len() <= 1 {
            self.content.push_str(&xml_escape(&t.content));
        } else {
            for (i, line) in lines.iter().enumerate() {
                if i == 0 {
                    self.content.push_str(&format!(
                        r#"<tspan x="{}" dy="0">{}</tspan>"#,
                        t.x,
                        xml_escape(line),
                    ));
                } else {
                    self.content.push_str(&format!(
                        r#"<tspan x="{}" dy="1.2em">{}</tspan>"#,
                        t.x,
                        xml_escape(line),
                    ));
                }
            }
        }
        self.content.push_str("</text>\n");
    }

    fn arrow(&mut self, a: &Arrow) {
        // Draw the line
        if a.dashed {
            self.content.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}" stroke-dasharray="2,2"/>"#,
                a.x1, a.y1, a.x2, a.y2, a.stroke, a.stroke_width,
            ));
        } else {
            self.content.push_str(&format!(
                r#"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="{}"/>"#,
                a.x1, a.y1, a.x2, a.y2, a.stroke, a.stroke_width,
            ));
        }
        self.content.push('\n');

        // Draw the arrowhead
        let (dx, dy) = (a.x2 - a.x1, a.y2 - a.y1);
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return;
        }
        let (ux, uy) = (dx / len, dy / len);
        // PlantUML-style concave arrowhead: length 10, half-width 4, notch 6 back from tip.
        let head_len = 10.0;
        let head_width = 4.0;
        let notch_len = 6.0;

        let tip_x = a.x2;
        let tip_y = a.y2;
        let base_x = tip_x - ux * head_len;
        let base_y = tip_y - uy * head_len;
        let left_x = base_x - uy * head_width;
        let left_y = base_y + ux * head_width;
        let right_x = base_x + uy * head_width;
        let right_y = base_y - ux * head_width;
        let notch_x = tip_x - ux * notch_len;
        let notch_y = tip_y - uy * notch_len;

        match a.head {
            ArrowHeadStyle::Filled => {
                self.content.push_str(&format!(
                    r#"<polygon points="{},{} {},{} {},{} {},{}" fill="{}" stroke="none"/>"#,
                    left_x, left_y, tip_x, tip_y, right_x, right_y, notch_x, notch_y, a.stroke,
                ));
            }
            ArrowHeadStyle::Open => {
                self.content.push_str(&format!(
                    r#"<polyline points="{},{} {},{} {},{}" fill="none" stroke="{}" stroke-width="{}"/>"#,
                    left_x, left_y, tip_x, tip_y, right_x, right_y, a.stroke, a.stroke_width,
                ));
            }
        }
        self.content.push('\n');
    }

    fn polygon(&mut self, p: &Polygon) {
        let points: Vec<String> = p
            .points
            .iter()
            .map(|(x, y)| format!("{},{}", x, y))
            .collect();
        self.content.push_str(&format!(
            r#"<polygon points="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            points.join(" "),
            p.fill,
            p.stroke,
            p.stroke_width,
        ));
        self.content.push('\n');
    }

    fn path(&mut self, p: &Path) {
        self.content.push_str(&format!(
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}""#,
            p.d, p.fill, p.stroke, p.stroke_width,
        ));
        if p.dashed {
            self.content.push_str(r#" stroke-dasharray="2,2""#);
        }
        self.content.push_str("/>\n");
    }

    fn finish(mut self) -> String {
        self.content.push_str("</svg>");
        self.content
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_diagram() {
        let diagram = LaidOutDiagram {
            width: 100.0,
            height: 100.0,
            primitives: vec![],
        };
        let svg = render(&diagram);
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("viewBox=\"0 0 100 100\""));
    }

    #[test]
    fn test_rect_rendering() {
        let diagram = LaidOutDiagram {
            width: 200.0,
            height: 200.0,
            primitives: vec![Primitive::Rect(Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 50.0,
                fill: "#FEFECE".into(),
                stroke: "#A80036".into(),
                stroke_width: 1.0,
                rx: 0.0,
                ry: 0.0,
            })],
        };
        let svg = render(&diagram);
        assert!(svg.contains(r##"fill="#FEFECE""##));
        assert!(svg.contains(r##"stroke="#A80036""##));
    }

    #[test]
    fn test_text_escaping() {
        let diagram = LaidOutDiagram {
            width: 200.0,
            height: 200.0,
            primitives: vec![Primitive::Text(Text {
                bold: false,
                x: 10.0,
                y: 20.0,
                content: "A <-> B".into(),
                font_size: 14.0,
                font_family: "sans-serif".into(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            })],
        };
        let svg = render(&diagram);
        assert!(svg.contains("A &lt;-&gt; B"));
    }

    #[test]
    fn test_arrow_rendering() {
        let diagram = LaidOutDiagram {
            width: 200.0,
            height: 200.0,
            primitives: vec![Primitive::Arrow(Arrow {
                x1: 10.0,
                y1: 50.0,
                x2: 190.0,
                y2: 50.0,
                stroke: "#A80036".into(),
                stroke_width: 1.0,
                head: ArrowHeadStyle::Filled,
                dashed: false,
            })],
        };
        let svg = render(&diagram);
        assert!(svg.contains("<line"));
        assert!(svg.contains("<polygon"));
    }

    #[test]
    fn test_dashed_arrow() {
        let diagram = LaidOutDiagram {
            width: 200.0,
            height: 200.0,
            primitives: vec![Primitive::Arrow(Arrow {
                x1: 10.0,
                y1: 50.0,
                x2: 190.0,
                y2: 50.0,
                stroke: "#A80036".into(),
                stroke_width: 1.0,
                head: ArrowHeadStyle::Filled,
                dashed: true,
            })],
        };
        let svg = render(&diagram);
        assert!(svg.contains("stroke-dasharray"));
    }
}
