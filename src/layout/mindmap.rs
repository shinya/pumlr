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
        if root.children.iter().all(|c| c.side == Side::Right) {
            // Right-only mind map: original single-sided layout.
            let h = subtree_height(root);
            let right = draw_subtree(&mut prims, root, MARGIN_LEFT, y, theme, &measurer);
            max_right = max_right.max(right);
            y += h + V_GAP;
        } else {
            let (h, right) = draw_two_sided_root(&mut prims, root, y, theme, &measurer);
            max_right = max_right.max(right);
            y += h + V_GAP;
        }
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

/// Horizontal extent of a subtree: the node itself plus its deepest chain of
/// children (each level adds H_GAP + child width).
fn subtree_width(node: &TreeNode, measurer: &TextMeasurer) -> f32 {
    let w = node_width(node, measurer);
    let child_w = node
        .children
        .iter()
        .map(|c| subtree_width(c, measurer))
        .fold(0.0f32, f32::max);
    if node.children.is_empty() {
        w
    } else {
        w + H_GAP + child_w
    }
}

/// Height of a vertical stack of sibling subtrees (V_GAP between them).
fn stack_height(nodes: &[&TreeNode]) -> f32 {
    if nodes.is_empty() {
        return 0.0;
    }
    nodes.iter().map(|n| subtree_height(n)).sum::<f32>()
        + V_GAP * (nodes.len() as f32 - 1.0)
}

/// Draw a root whose children span both sides (`left side` / OrgMode `-`).
///
/// Matches the reference rendering: the root sits between the two child
/// stacks, and each stack is vertically centered on the root's center.
/// Returns (band height, rightmost extent).
fn draw_two_sided_root(
    prims: &mut Vec<Primitive>,
    root: &TreeNode,
    top: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) -> (f32, f32) {
    let left_children: Vec<&TreeNode> = root
        .children
        .iter()
        .filter(|c| c.side == Side::Left)
        .collect();
    let right_children: Vec<&TreeNode> = root
        .children
        .iter()
        .filter(|c| c.side == Side::Right)
        .collect();

    let left_extent = left_children
        .iter()
        .map(|c| subtree_width(c, measurer))
        .fold(0.0f32, f32::max);
    let root_w = node_width(root, measurer);
    let root_x = MARGIN_LEFT + left_extent + if left_children.is_empty() { 0.0 } else { H_GAP };

    let lh = stack_height(&left_children);
    let rh = stack_height(&right_children);
    let band_h = lh.max(rh).max(NODE_H);
    let root_cy = top + band_h / 2.0;

    // Right stack.
    let mut max_right = root_x + root_w;
    let child_x = root_x + root_w + H_GAP;
    let mut cy = root_cy - rh / 2.0;
    for child in &right_children {
        let ch = subtree_height(child);
        let right = draw_subtree(prims, child, child_x, cy, theme, measurer);
        max_right = max_right.max(right);
        let ccy = cy + ch_center(child, ch);
        push_connector(prims, root_x + root_w, root_cy, child_x, ccy, 1.0, theme);
        cy += ch + V_GAP;
    }

    // Left stack (mirrored).
    let child_right_x = root_x - H_GAP;
    let mut cy = root_cy - lh / 2.0;
    for child in &left_children {
        let ch = subtree_height(child);
        draw_subtree_left(prims, child, child_right_x, cy, theme, measurer);
        let ccy = cy + ch_center(child, ch);
        push_connector(prims, root_x, root_cy, child_right_x, ccy, -1.0, theme);
        cy += ch + V_GAP;
    }

    draw_node_box(prims, root, root_x, root_cy - NODE_H / 2.0, root_w, theme);
    (band_h, max_right)
}

/// Draw `node`'s subtree growing leftward, with its right edge at `right_x`
/// and its band's top at `top`.
fn draw_subtree_left(
    prims: &mut Vec<Primitive>,
    node: &TreeNode,
    right_x: f32,
    top: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) {
    let w = node_width(node, measurer);
    let x = right_x - w;
    let total_h = subtree_height(node);

    // Children first (to know their centers).
    let child_right_x = x - H_GAP;
    let mut cy = top;
    let mut child_centers: Vec<f32> = Vec::new();
    for child in &node.children {
        let ch = subtree_height(child);
        draw_subtree_left(prims, child, child_right_x, cy, theme, measurer);
        child_centers.push(cy + ch_center(child, ch));
        cy += ch + V_GAP;
    }

    // This node: vertically centered on its children (or its own band).
    let node_cy = if child_centers.is_empty() {
        top + total_h / 2.0
    } else {
        child_centers.iter().sum::<f32>() / child_centers.len() as f32
    };
    draw_node_box(prims, node, x, node_cy - NODE_H / 2.0, w, theme);

    for &ccy in &child_centers {
        push_connector(prims, x, node_cy, child_right_x, ccy, -1.0, theme);
    }
}

/// Bezier connector from a parent edge at (px, py) to a child edge at
/// (cx, ccy). `dir` is 1.0 when the child is to the right, -1.0 to the left.
fn push_connector(
    prims: &mut Vec<Primitive>,
    px: f32,
    py: f32,
    cx: f32,
    ccy: f32,
    dir: f32,
    theme: &dyn Theme,
) {
    prims.push(Primitive::Path(Path {
        d: format!(
            "M{},{} L{},{} C{},{} {},{} {},{} L{},{}",
            px,
            py,
            px + 10.0 * dir,
            py,
            px + 25.0 * dir,
            py,
            px + 25.0 * dir,
            ccy,
            px + 40.0 * dir,
            ccy,
            cx,
            ccy,
        ),
        fill: "none".into(),
        stroke: theme.activity_shape_stroke().to_string(),
        stroke_width: 1.0,
        dashed: false,
    }));
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
    for &ccy in &child_centers {
        push_connector(prims, x + w, node_cy, child_x, ccy, 1.0, theme);
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
        underline: false,
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

    /// Collect (rect, following text content) pairs; draw_node_box pushes the
    /// box immediately before its label.
    fn boxes(laid: &LaidOutDiagram) -> Vec<(Rect, String)> {
        let mut out = Vec::new();
        let mut pending: Option<Rect> = None;
        for p in &laid.primitives {
            match p {
                Primitive::Rect(r) => pending = Some(r.clone()),
                Primitive::Text(t) => {
                    if let Some(r) = pending.take() {
                        out.push((r, t.content.clone()));
                    }
                }
                _ => {}
            }
        }
        out
    }

    fn find(boxes: &[(Rect, String)], text: &str) -> Rect {
        boxes
            .iter()
            .find(|(_, t)| t == text)
            .map(|(r, _)| r.clone())
            .unwrap()
    }

    #[test]
    fn test_left_side_layout() {
        let d = parse("* R\nleft side\n** L\n*** LL\nright side\n** X\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let bs = boxes(&laid);
        let (root, l, ll, x) = (
            find(&bs, "R"),
            find(&bs, "L"),
            find(&bs, "LL"),
            find(&bs, "X"),
        );
        // Left children end H_GAP left of their parent's left edge.
        assert!((l.x + l.width + H_GAP - root.x).abs() < 0.01);
        assert!((ll.x + ll.width + H_GAP - l.x).abs() < 0.01);
        // Right children keep the original H_GAP to the right.
        assert!((x.x - (root.x + root.width + H_GAP)).abs() < 0.01);
        // Leftmost node starts at the left margin.
        assert!((ll.x - MARGIN_LEFT).abs() < 0.01);
        // Root is centered on the band shared by both stacks.
        let root_cy = root.y + root.height / 2.0;
        let l_cy = l.y + l.height / 2.0;
        let x_cy = x.y + x.height / 2.0;
        assert!((root_cy - l_cy).abs() < 0.01);
        assert!((root_cy - x_cy).abs() < 0.01);
        // One connector per parent-child edge.
        assert_eq!(
            laid.primitives
                .iter()
                .filter(|p| matches!(p, Primitive::Path(_)))
                .count(),
            3
        );
    }

    #[test]
    fn test_left_only_mindmap() {
        let d = parse("* R\nleft side\n** A\n** B\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let bs = boxes(&laid);
        let (root, a, b) = (find(&bs, "R"), find(&bs, "A"), find(&bs, "B"));
        assert!(a.x + a.width < root.x);
        assert!(b.x + b.width < root.x);
        // Root vertically centered between the two children.
        let root_cy = root.y + root.height / 2.0;
        let mean = (a.y + a.height / 2.0 + b.y + b.height / 2.0) / 2.0;
        assert!((root_cy - mean).abs() < 0.01);
    }

    #[test]
    fn test_right_only_unchanged_by_side_support() {
        // A right-only map must keep the original single-sided layout.
        let d = parse("* Root\n** A\n** B\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let root = find(&boxes(&laid), "Root");
        assert!((root.x - MARGIN_LEFT).abs() < 0.01);
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
