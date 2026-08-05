/// Layout for WBS diagrams, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
///
/// The root sits on top with its level-1 children in a horizontal row; every
/// deeper level renders as an indented vertical list.
use crate::ast::tree::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const FONT: f32 = 12.0;
const NODE_H: f32 = 34.1328;
const TEXT_BASELINE: f32 = 21.6016;
const ROOT_DROP: f32 = 40.0; // root bottom -> child top (20 + rail + 20)
const LIST_INDENT: f32 = 10.0; // child left = parent center + 10
const LIST_GAP: f32 = 15.0;
const SIBLING_GAP: f32 = 20.0;
const MARGIN: f32 = 20.0;
const STROKE_W: f32 = 1.5;

pub fn layout_with_theme(diagram: &TreeDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);
    let mut prims: Vec<Primitive> = Vec::new();

    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let mut x = MARGIN;
    let mut max_bottom = 0.0f32;
    for root in &diagram.roots {
        let (w, h) = draw_root(&mut prims, root, x, MARGIN + title_offset, theme, &measurer);
        x += w + SIBLING_GAP * 2.0;
        max_bottom = max_bottom.max(MARGIN + title_offset + h);
    }

    let width = x - SIBLING_GAP * 2.0 + MARGIN;
    let height = max_bottom + MARGIN;

    if let Some(title) = &diagram.title {
        prims.push(Primitive::Text(Text {
            x: width / 2.0,
            y: 20.0,
            content: title.clone(),
            font_size: 14.0,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Middle,
            bold: true,
            italic: false,
            underline: false,
        }));
    }

    LaidOutDiagram {
        width,
        height,
        primitives: prims,
    }
}

fn node_width(node: &TreeNode, measurer: &TextMeasurer) -> f32 {
    measurer.measure_multiline_width(&node.text) + 20.0
}

/// Width of a vertical-list subtree whose node's left edge is at 0.
fn list_subtree_width(node: &TreeNode, measurer: &TextMeasurer) -> f32 {
    let w = node_width(node, measurer);
    let child_left = w / 2.0 + LIST_INDENT;
    let mut max_w = w;
    for child in &node.children {
        max_w = max_w.max(child_left + list_subtree_width(child, measurer));
    }
    max_w
}

/// Height of a vertical-list subtree.
fn list_subtree_height(node: &TreeNode) -> f32 {
    let mut h = NODE_H;
    for child in &node.children {
        h += LIST_GAP + list_subtree_height(child);
    }
    h
}

/// Draw the root with its horizontal row of children; returns (width, height).
fn draw_root(
    prims: &mut Vec<Primitive>,
    root: &TreeNode,
    x: f32,
    y: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) -> (f32, f32) {
    let root_w = node_width(root, measurer);

    if root.children.is_empty() {
        draw_box(prims, root, x, y, root_w, theme, measurer);
        return (root_w, NODE_H);
    }

    // Lay out level-1 subtrees in a row.
    let child_y = y + NODE_H + ROOT_DROP;
    let mut cx = x;
    let mut centers: Vec<f32> = Vec::new();
    let mut max_h = 0.0f32;
    for child in &root.children {
        let stw = list_subtree_width(child, measurer);
        let sth = list_subtree_height(child);
        draw_list_subtree(prims, child, cx, child_y, theme, measurer);
        centers.push(cx + node_width(child, measurer) / 2.0);
        cx += stw + SIBLING_GAP;
        max_h = max_h.max(sth);
    }
    let row_right = cx - SIBLING_GAP;

    // Root centered over the first/last child centers.
    let root_cx = (centers[0] + centers[centers.len() - 1]) / 2.0;
    let root_x = (root_cx - root_w / 2.0).max(x);
    draw_box(prims, root, root_x, y, root_w, theme, measurer);

    // Elbow: root bottom -> rail -> child tops.
    let stroke = theme.activity_shape_stroke().to_string();
    let rail_y = y + NODE_H + ROOT_DROP / 2.0;
    let line = |prims: &mut Vec<Primitive>, x1: f32, y1: f32, x2: f32, y2: f32| {
        prims.push(Primitive::Line(Line {
            x1,
            y1,
            x2,
            y2,
            stroke: stroke.clone(),
            stroke_width: STROKE_W,
        }));
    };
    line(prims, root_x + root_w / 2.0, y + NODE_H, root_x + root_w / 2.0, rail_y);
    line(prims, centers[0], rail_y, centers[centers.len() - 1], rail_y);
    for &ccx in &centers {
        line(prims, ccx, rail_y, ccx, child_y);
    }

    let width = (row_right - x).max(root_x + root_w - x);
    let height = NODE_H + ROOT_DROP + max_h;
    (width, height)
}

/// Draw a vertical-list subtree with the node's top-left at (x, y);
/// returns the bottom y used.
fn draw_list_subtree(
    prims: &mut Vec<Primitive>,
    node: &TreeNode,
    x: f32,
    y: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) -> f32 {
    let w = node_width(node, measurer);
    draw_box(prims, node, x, y, w, theme, measurer);

    let spine_x = x + w / 2.0;
    let child_x = spine_x + LIST_INDENT;
    let stroke = theme.activity_shape_stroke().to_string();

    let mut cy = y + NODE_H + LIST_GAP;
    let mut last_center = y + NODE_H;
    for child in &node.children {
        let center = cy + NODE_H / 2.0;
        // Horizontal stub into the child's left edge.
        prims.push(Primitive::Line(Line {
            x1: spine_x,
            y1: center,
            x2: child_x,
            y2: center,
            stroke: stroke.clone(),
            stroke_width: STROKE_W,
        }));
        let bottom = draw_list_subtree(prims, child, child_x, cy, theme, measurer);
        last_center = center;
        cy = bottom + LIST_GAP;
    }
    if !node.children.is_empty() {
        // Vertical spine from the parent's bottom to the last child's center.
        prims.push(Primitive::Line(Line {
            x1: spine_x,
            y1: y + NODE_H,
            x2: spine_x,
            y2: last_center,
            stroke,
            stroke_width: STROKE_W,
        }));
    }
    cy - LIST_GAP
}

#[allow(clippy::too_many_arguments)]
fn draw_box(
    prims: &mut Vec<Primitive>,
    node: &TreeNode,
    x: f32,
    y: f32,
    w: f32,
    theme: &dyn Theme,
    _measurer: &TextMeasurer,
) {
    if !node.boxless {
        let fill = node
            .color
            .as_deref()
            .map(resolve_color)
            .unwrap_or_else(|| theme.activity_shape_fill().to_string());
        prims.push(Primitive::Rect(Rect {
            x,
            y,
            width: w,
            height: NODE_H,
            fill,
            stroke: theme.activity_shape_stroke().to_string(),
            stroke_width: STROKE_W,
            rx: 0.0,
            ry: 0.0,
        }));
    }
    prims.push(Primitive::Text(Text {
        x: x + 10.0,
        y: y + TEXT_BASELINE,
        content: node.text.clone(),
        font_size: FONT,
        font_family: theme.font_family().to_string(),
        fill: theme.activity_text_color().to_string(),
        anchor: TextAnchor::Start,
        bold: false,
        italic: false,
        underline: false,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tree::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_basic_wbs() {
        let d = parse("* Root\n** A\n*** A1\n** B\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rects: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 4);
        // WBS boxes are square-cornered and 34.13 tall.
        assert!(rects.iter().all(|r| r.rx == 0.0));
        assert!(rects.iter().all(|r| (r.height - NODE_H).abs() < 0.01));
    }

    #[test]
    fn test_level1_row_below_root() {
        let d = parse("* R\n** A\n** B\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rects: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        // Children (drawn first) share a row below the root (drawn last).
        let root = rects[2];
        assert!(rects[0].y > root.y);
        assert!((rects[0].y - rects[1].y).abs() < 0.01);
    }

    #[test]
    fn test_list_indent() {
        let d = parse("* R\n** A\n*** A1\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rects: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        // A1 (index 1: drawn after A? order: A drawn, then A1, then root)
        // A = rects[0], A1 = rects[1].
        let a = rects[0];
        let a1 = rects[1];
        assert!((a1.x - (a.x + a.width / 2.0 + LIST_INDENT)).abs() < 0.01);
        assert!(a1.y > a.y);
    }
}
