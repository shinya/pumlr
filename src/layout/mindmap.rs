/// Layout for mind maps, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::tree::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const FONT: f32 = 14.0;
const NODE_H: f32 = 36.4883;
const TEXT_BASELINE: f32 = 23.5351;
const H_GAP: f32 = 50.0;
const V_GAP: f32 = 20.0;
const MARGIN_LEFT: f32 = 10.0;
const MARGIN: f32 = 20.0;

pub fn layout_with_theme(diagram: &TreeDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);
    let mut prims: Vec<Primitive> = Vec::new();

    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let mut y = MARGIN + title_offset;
    let mut max_right = 0.0f32;
    for root in &diagram.roots {
        let h = subtree_height(root);
        let right = draw_subtree(
            &mut prims,
            root,
            MARGIN_LEFT,
            y,
            theme,
            &measurer,
        );
        max_right = max_right.max(right);
        y += h + V_GAP;
    }
    let content_bottom = y - V_GAP;

    let width = max_right + MARGIN;
    let height = content_bottom + MARGIN;

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

fn subtree_height(node: &TreeNode) -> f32 {
    if node.children.is_empty() {
        return NODE_H;
    }
    let children_h: f32 = node
        .children
        .iter()
        .map(subtree_height)
        .sum::<f32>()
        + V_GAP * (node.children.len() as f32 - 1.0);
    children_h.max(NODE_H)
}

/// Draw `node`'s subtree with its top-left at (x, top); returns the rightmost
/// extent used.
fn draw_subtree(
    prims: &mut Vec<Primitive>,
    node: &TreeNode,
    x: f32,
    top: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) -> f32 {
    let w = node_width(node, measurer);
    let total_h = subtree_height(node);

    // Children first (to know their centers).
    let child_x = x + w + H_GAP;
    let mut cy = top;
    let mut child_centers: Vec<f32> = Vec::new();
    let mut max_right = x + w;
    for child in &node.children {
        let ch = subtree_height(child);
        let right = draw_subtree(prims, child, child_x, cy, theme, measurer);
        max_right = max_right.max(right);
        child_centers.push(cy + ch_center(child, ch));
        cy += ch + V_GAP;
    }

    // This node: vertically centered on its children (or its own band).
    let node_cy = if child_centers.is_empty() {
        top + total_h / 2.0
    } else {
        child_centers.iter().sum::<f32>() / child_centers.len() as f32
    };
    let node_y = node_cy - NODE_H / 2.0;

    draw_node_box(prims, node, x, node_y, w, theme);

    // Connectors to children.
    for (child, &ccy) in node.children.iter().zip(child_centers.iter()) {
        let px = x + w;
        let py = node_cy;
        let cx = child_x;
        prims.push(Primitive::Path(Path {
            d: format!(
                "M{},{} L{},{} C{},{} {},{} {},{} L{},{}",
                px,
                py,
                px + 10.0,
                py,
                px + 25.0,
                py,
                px + 25.0,
                ccy,
                px + 40.0,
                ccy,
                cx,
                ccy,
            ),
            fill: "none".into(),
            stroke: theme.activity_shape_stroke().to_string(),
            stroke_width: 1.0,
            dashed: false,
        }));
        let _ = child;
    }

    max_right
}

/// Center-y offset of a node within its subtree band of height `total_h`.
fn ch_center(node: &TreeNode, total_h: f32) -> f32 {
    if node.children.is_empty() {
        return total_h / 2.0;
    }
    // Mirror of draw_subtree's centering: mean of child centers.
    let mut cy = 0.0f32;
    let mut centers: Vec<f32> = Vec::new();
    for child in &node.children {
        let ch = subtree_height(child);
        centers.push(cy + ch_center(child, ch));
        cy += ch + V_GAP;
    }
    centers.iter().sum::<f32>() / centers.len() as f32
}

fn draw_node_box(
    prims: &mut Vec<Primitive>,
    node: &TreeNode,
    x: f32,
    y: f32,
    w: f32,
    theme: &dyn Theme,
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
            stroke_width: 1.5,
            rx: 12.5,
            ry: 12.5,
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
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tree::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_basic_mindmap() {
        let d = parse("* Root\n** A\n** B\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rects: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 3);
        // Children are drawn first; the root box is drawn last.
        let (a, b, root) = (rects[0], rects[1], rects[2]);
        assert!(a.x > root.x && b.x > root.x);
        // Root vertically centered between the two children.
        let root_cy = root.y + root.height / 2.0;
        let mean_child_cy = (a.y + b.y + a.height) / 2.0;
        assert!((root_cy - mean_child_cy).abs() < 1.0);
        // Bezier connectors present.
        assert_eq!(
            laid.primitives
                .iter()
                .filter(|p| matches!(p, Primitive::Path(_)))
                .count(),
            2
        );
    }

    #[test]
    fn test_node_height() {
        let d = parse("* X\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .unwrap();
        assert!((rect.height - NODE_H).abs() < 0.01);
        assert!((rect.rx - 12.5).abs() < 0.01);
    }
}
