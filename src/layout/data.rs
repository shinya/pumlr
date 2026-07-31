/// Layout for JSON / YAML data diagrams, matching the PlantUML 1.2026
/// default style (measured values recorded in SPEC.md).
///
/// Objects render as two-column tables (bold keys), arrays as single-column
/// stacks. Nested values get a bullet in the parent row and a dashed link to
/// a child box placed to the right.
use crate::ast::data::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::Theme;

const FONT: f32 = 14.0;
const ROW_H: f32 = 20.4883;
const ROW_BASELINE: f32 = 15.5352;
const CELL_PAD: f32 = 5.0;
const CHILD_GAP_X: f32 = 45.0;
const SIBLING_GAP_Y: f32 = 18.0;
const MARGIN: f32 = 10.0;
const CORNER_R: f32 = 5.0;

pub fn layout_with_theme(
    diagram: &DataDiagram,
    json_style: bool,
    theme: &dyn Theme,
) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);
    let mut prims: Vec<Primitive> = Vec::new();
    let mut extent = (0.0f32, 0.0f32);

    layout_value(
        &diagram.root,
        MARGIN,
        MARGIN,
        json_style,
        theme,
        &measurer,
        &mut prims,
        &mut extent,
    );

    LaidOutDiagram {
        width: extent.0 + MARGIN + 2.0,
        height: extent.1 + MARGIN + 2.0,
        primitives: prims,
    }
}

/// A row in a table box: label cells plus an optional nested child value.
struct RowSpec<'a> {
    key: Option<String>,
    value_text: String,
    child: Option<&'a DataValue>,
}

fn scalar_display(value: &DataValue, json_style: bool) -> String {
    match value {
        DataValue::Bool(b) if json_style => {
            if *b {
                "\u{2611} true".to_string()
            } else {
                "\u{2610} false".to_string()
            }
        }
        other => other.scalar_text(),
    }
}

/// Lay out `value` as a box with its top-left at (x, y); draws primitives and
/// updates `extent`. Returns (box_width, subtree_height, box_height).
#[allow(clippy::too_many_arguments)]
fn layout_value(
    value: &DataValue,
    x: f32,
    y: f32,
    json_style: bool,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
    prims: &mut Vec<Primitive>,
    extent: &mut (f32, f32),
) -> (f32, f32, f32) {
    // Build rows.
    let rows: Vec<RowSpec> = match value {
        DataValue::Object(entries) => entries
            .iter()
            .map(|(k, v)| RowSpec {
                key: Some(k.clone()),
                value_text: if v.is_scalar() {
                    scalar_display(v, json_style)
                } else {
                    String::new()
                },
                child: if v.is_scalar() { None } else { Some(v) },
            })
            .collect(),
        DataValue::Array(items) => items
            .iter()
            .map(|v| RowSpec {
                key: None,
                value_text: if v.is_scalar() {
                    scalar_display(v, json_style)
                } else {
                    String::new()
                },
                child: if v.is_scalar() { None } else { Some(v) },
            })
            .collect(),
        scalar => vec![RowSpec {
            key: None,
            value_text: scalar_display(scalar, json_style),
            child: None,
        }],
    };

    let has_keys = rows.iter().any(|r| r.key.is_some());
    let key_col_w = if has_keys {
        rows.iter()
            .filter_map(|r| r.key.as_deref())
            .map(|k| measurer.measure_width(k))
            .fold(0.0f32, f32::max)
            + CELL_PAD * 2.0
    } else {
        0.0
    };
    let val_col_w = rows
        .iter()
        .map(|r| {
            let w = measurer.measure_width(&r.value_text);
            if r.child.is_some() {
                w.max(20.0)
            } else {
                w
            }
        })
        .fold(0.0f32, f32::max)
        + CELL_PAD * 2.0;
    let box_w = key_col_w + val_col_w;
    let box_h = rows.len().max(1) as f32 * ROW_H;

    // Background rect (frame drawn after content).
    prims.push(Primitive::Rect(Rect {
        x,
        y,
        width: box_w,
        height: box_h,
        fill: theme.activity_shape_fill().to_string(),
        stroke: theme.activity_shape_fill().to_string(),
        stroke_width: 1.5,
        rx: CORNER_R,
        ry: CORNER_R,
    }));

    // Rows.
    for (i, row) in rows.iter().enumerate() {
        let row_top = y + i as f32 * ROW_H;
        if i > 0 {
            prims.push(Primitive::Line(Line {
                x1: x,
                y1: row_top,
                x2: x + box_w,
                y2: row_top,
                stroke: "#000000".into(),
                stroke_width: 1.0,
            }));
        }
        if let Some(key) = &row.key {
            prims.push(Primitive::Text(Text {
                x: x + CELL_PAD,
                y: row_top + ROW_BASELINE,
                content: key.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: "#000000".into(),
                anchor: TextAnchor::Start,
                bold: true,
                italic: false,
            }));
        }
        if !row.value_text.is_empty() {
            prims.push(Primitive::Text(Text {
                x: x + key_col_w + CELL_PAD,
                y: row_top + ROW_BASELINE,
                content: row.value_text.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: "#000000".into(),
                anchor: TextAnchor::Start,
                bold: false,
                italic: false,
            }));
        }
    }
    // Column separator.
    if has_keys {
        prims.push(Primitive::Line(Line {
            x1: x + key_col_w,
            y1: y,
            x2: x + key_col_w,
            y2: y + box_h,
            stroke: "#000000".into(),
            stroke_width: 1.0,
        }));
    }

    extent.0 = extent.0.max(x + box_w);
    extent.1 = extent.1.max(y + box_h);

    // Children to the right, stacked vertically.
    let child_x = x + box_w + CHILD_GAP_X;
    let mut child_y = y.min(y); // children start level with the parent top
    let mut subtree_bottom = y + box_h;
    for (i, row) in rows.iter().enumerate() {
        let Some(child) = row.child else { continue };
        let (child_w, child_subtree_h, child_h) = layout_value(
            child, child_x, child_y, json_style, theme, measurer, prims, extent,
        );
        let _ = child_w;

        // Bullet in the parent row + dashed link to the child's left edge.
        let row_center = y + i as f32 * ROW_H + ROW_H / 2.0;
        let bullet_x = x + box_w - 12.0;
        prims.push(Primitive::Circle(Circle {
            cx: bullet_x,
            cy: row_center,
            r: 3.0,
            fill: "#000000".into(),
            stroke: "#000000".into(),
            stroke_width: 1.0,
        }));
        let tip_y = child_y + child_h / 2.0;
        prims.push(Primitive::Path(Path {
            d: format!(
                "M{},{} L{},{} C{},{} {},{} {},{}",
                bullet_x,
                row_center,
                x + box_w + 2.0,
                row_center,
                x + box_w + 20.0,
                row_center,
                child_x - 25.0,
                tip_y,
                child_x - 8.0,
                tip_y,
            ),
            fill: "none".into(),
            stroke: "#000000".into(),
            stroke_width: 1.0,
            dashed: true,
        }));
        prims.push(Primitive::Polygon(Polygon {
            points: vec![
                (child_x, tip_y),
                (child_x - 8.0, tip_y - 4.0),
                (child_x - 8.0, tip_y + 4.0),
            ],
            fill: "#000000".into(),
            stroke: "#000000".into(),
            stroke_width: 1.0,
        }));

        child_y += child_subtree_h + SIBLING_GAP_Y;
        subtree_bottom = subtree_bottom.max(child_y - SIBLING_GAP_Y);
    }

    // Frame on top of the content.
    prims.push(Primitive::Rect(Rect {
        x,
        y,
        width: box_w,
        height: box_h,
        fill: "none".into(),
        stroke: "#000000".into(),
        stroke_width: 1.5,
        rx: CORNER_R,
        ry: CORNER_R,
    }));

    (box_w, subtree_bottom - y, box_h)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::data::{parse_json, parse_yaml};
    use crate::theme::DefaultTheme;

    #[test]
    fn test_json_table() {
        let d = parse_json(r#"{"name": "pumlr", "ok": true}"#).unwrap();
        let laid = layout_with_theme(&d, true, &DefaultTheme);
        // Checkbox for JSON booleans.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content.contains('\u{2611}'))));
        // Bold key text.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "name" && t.bold)));
    }

    #[test]
    fn test_yaml_plain_bool() {
        let d = parse_yaml("ok: true\n").unwrap();
        let laid = layout_with_theme(&d, false, &DefaultTheme);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "true")));
        assert!(!laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content.contains('\u{2611}'))));
    }

    #[test]
    fn test_nested_child_box() {
        let d = parse_json(r#"{"a": {"b": "c"}, "list": [1, 2]}"#).unwrap();
        let laid = layout_with_theme(&d, true, &DefaultTheme);
        // 3 boxes = 3 background + 3 frame rects.
        let rects = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(_)))
            .count();
        assert_eq!(rects, 6);
        // Two bullets and two dashed links with arrowheads.
        let bullets = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Circle(c) if c.r == 3.0))
            .count();
        assert_eq!(bullets, 2);
        assert_eq!(
            laid.primitives
                .iter()
                .filter(|p| matches!(p, Primitive::Polygon(_)))
                .count(),
            2
        );
    }
}
