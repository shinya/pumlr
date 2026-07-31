/// Layout for use case diagrams, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::usecase::*;
use crate::layout::graph::{self, GraphEdge, GraphNode};
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const FONT: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;
const LINE_H: f32 = 16.4883;
const ACTOR_FIG_W: f32 = 26.0;
const ACTOR_FIG_H: f32 = 58.0;
const ACTOR_LABEL_GAP: f32 = 15.04;
const RANK_GAP: f32 = 60.0;
const LABEL_EXTRA: f32 = 18.0;
const SIBLING_GAP: f32 = 35.0;
const MARGIN: f32 = 7.0;
const RECT_TITLE_H: f32 = 22.0;
const RECT_PAD: f32 = 16.0;

pub fn layout_with_theme(diagram: &UseCaseDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);

    // --- Node sizes.
    let sizes: Vec<(f32, f32)> = diagram
        .elements
        .iter()
        .map(|e| element_size(e, &measurer))
        .collect();

    let elem_index = |name: &str| diagram.elements.iter().position(|e| e.name == name);
    let mut container_of: Vec<Option<usize>> = vec![None; diagram.elements.len()];
    for (ci, c) in diagram.containers.iter().enumerate() {
        for m in &c.members {
            if let Some(ei) = elem_index(m) {
                container_of[ei] = Some(ci);
            }
        }
    }

    // --- Inner layout per container.
    let mut rel_pos: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.elements.len()];
    let mut cont_size: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.containers.len()];
    for (ci, cont) in diagram.containers.iter().enumerate() {
        let members: Vec<usize> = (0..diagram.elements.len())
            .filter(|&ei| container_of[ei] == Some(ci))
            .collect();
        if members.is_empty() {
            cont_size[ci] = (100.0, 60.0);
            continue;
        }
        let mut nodes: Vec<GraphNode> = members
            .iter()
            .map(|&ei| graph_node(sizes[ei], diagram.left_to_right))
            .collect();
        let edges = collect_edges(diagram, |name| {
            elem_index(name)
                .filter(|ei| container_of[*ei] == Some(ci))
                .and_then(|ei| members.iter().position(|&m| m == ei))
        });
        let result = graph::layout(&mut nodes, &edges, SIBLING_GAP, RANK_GAP, LABEL_EXTRA);
        let (rw, rh) = oriented_extent(&result, diagram.left_to_right);
        for (k, &ei) in members.iter().enumerate() {
            rel_pos[ei] = oriented_pos(&nodes[k], diagram.left_to_right);
        }
        let name_w = measurer.measure_width(&cont.name);
        cont_size[ci] = (
            (rw + RECT_PAD * 2.0).max(name_w + 20.0),
            RECT_TITLE_H + RECT_PAD + rh + RECT_PAD,
        );
    }

    // --- Top-level graph: containers + free elements.
    #[derive(Clone, Copy)]
    enum TopNode {
        Element(usize),
        Container(usize),
    }
    let mut metas: Vec<TopNode> = Vec::new();
    let mut top_of: Vec<usize> = vec![0; diagram.elements.len()];
    for ci in 0..diagram.containers.len() {
        metas.push(TopNode::Container(ci));
    }
    for ei in 0..diagram.elements.len() {
        match container_of[ei] {
            Some(ci) => top_of[ei] = ci,
            None => {
                metas.push(TopNode::Element(ei));
                top_of[ei] = metas.len() - 1;
            }
        }
    }
    let mut top_nodes: Vec<GraphNode> = metas
        .iter()
        .map(|m| match m {
            TopNode::Element(ei) => graph_node(sizes[*ei], diagram.left_to_right),
            TopNode::Container(ci) => graph_node(cont_size[*ci], diagram.left_to_right),
        })
        .collect();
    let top_edges = {
        let mut edges = Vec::new();
        for rel in &diagram.relations {
            let (Some(a), Some(b)) = (elem_index(&rel.from), elem_index(&rel.to)) else {
                continue;
            };
            let (ta, tb) = (top_of[a], top_of[b]);
            if ta == tb {
                continue;
            }
            edges.push(GraphEdge {
                from: ta,
                to: tb,
                min_len: if rel.rank_len <= 1 { 0 } else { 1 },
                labeled: rel.label.is_some(),
            });
        }
        edges
    };
    let top_result = graph::layout(&mut top_nodes, &top_edges, SIBLING_GAP, RANK_GAP, LABEL_EXTRA);
    let (tw, th) = oriented_extent(&top_result, diagram.left_to_right);

    // --- Absolute positions.
    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let mut abs: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.elements.len()];
    for ei in 0..diagram.elements.len() {
        let top = oriented_pos(&top_nodes[top_of[ei]], diagram.left_to_right);
        abs[ei] = match container_of[ei] {
            Some(_) => (
                MARGIN + top.0 + RECT_PAD + rel_pos[ei].0,
                MARGIN + title_offset + top.1 + RECT_TITLE_H + RECT_PAD + rel_pos[ei].1,
            ),
            None => (MARGIN + top.0, MARGIN + title_offset + top.1),
        };
    }

    // --- Primitives.
    let mut prims: Vec<Primitive> = Vec::new();

    for (ci, cont) in diagram.containers.iter().enumerate() {
        let top = oriented_pos(&top_nodes[ci], diagram.left_to_right);
        draw_container(
            &mut prims,
            cont,
            MARGIN + top.0,
            MARGIN + title_offset + top.1,
            cont_size[ci].0,
            cont_size[ci].1,
            theme,
        );
    }

    for rel in &diagram.relations {
        let (Some(a), Some(b)) = (elem_index(&rel.from), elem_index(&rel.to)) else {
            continue;
        };
        let obstacles: Vec<(f32, f32, f32, f32)> = (0..diagram.elements.len())
            .filter(|&k| k != a && k != b)
            .map(|k| (abs[k].0, abs[k].1, sizes[k].0, sizes[k].1))
            .collect();
        draw_relation(&mut prims, diagram, rel, a, b, &abs, &sizes, &obstacles, theme);
    }

    for (ei, elem) in diagram.elements.iter().enumerate() {
        draw_element(&mut prims, elem, abs[ei], sizes[ei], theme, &measurer);
    }

    let mut width = MARGIN + tw + 14.0;
    let height = MARGIN + title_offset + th + 14.0;

    if let Some(title) = &diagram.title {
        width = width.max(measurer.measure_width(title) + 2.0 * MARGIN);
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

/// Swap axes for `left to right direction` (layout ranks top-to-bottom, then
/// transpose the result).
fn graph_node(size: (f32, f32), transpose: bool) -> GraphNode {
    if transpose {
        GraphNode::new(size.1, size.0)
    } else {
        GraphNode::new(size.0, size.1)
    }
}

fn oriented_pos(node: &GraphNode, transpose: bool) -> (f32, f32) {
    if transpose {
        (node.y, node.x)
    } else {
        (node.x, node.y)
    }
}

fn oriented_extent(result: &graph::GraphLayoutResult, transpose: bool) -> (f32, f32) {
    if transpose {
        (result.height, result.width)
    } else {
        (result.width, result.height)
    }
}

fn collect_edges(
    diagram: &UseCaseDiagram,
    mut resolve: impl FnMut(&str) -> Option<usize>,
) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    for rel in &diagram.relations {
        let (Some(a), Some(b)) = (resolve(&rel.from), resolve(&rel.to)) else {
            continue;
        };
        edges.push(GraphEdge {
            from: a,
            to: b,
            min_len: if rel.rank_len <= 1 { 0 } else { 1 },
            labeled: rel.label.is_some(),
        });
    }
    edges
}

fn element_size(elem: &Element, measurer: &TextMeasurer) -> (f32, f32) {
    match elem.kind {
        ElementKind::Actor => {
            let label_w = measurer.measure_width(&elem.display_name);
            (
                ACTOR_FIG_W.max(label_w),
                ACTOR_FIG_H + ACTOR_LABEL_GAP + 4.0,
            )
        }
        ElementKind::UseCase => {
            let (rx, ry) = ellipse_radii(measurer.measure_multiline_width(&elem.display_name));
            (rx * 2.0, ry * 2.0)
        }
    }
}

/// PlantUML's ellipse-radius rule (SPEC.md): circumscribe the text box, with
/// an aspect cap of 5 and 3px padding.
fn ellipse_radii(text_w: f32) -> (f32, f32) {
    let w = text_w.max(10.0);
    let h = LINE_H;
    if w / h > 5.0 {
        let ry = ((w / 10.0).powi(2) + (h / 2.0).powi(2)).sqrt();
        (5.0 * ry + 3.0, ry + 3.0)
    } else {
        let rx = w / 2.0 * std::f32::consts::SQRT_2;
        let ry = h / 2.0 * std::f32::consts::SQRT_2;
        (rx + 3.0, ry + 3.0)
    }
}

fn draw_element(
    prims: &mut Vec<Primitive>,
    elem: &Element,
    pos: (f32, f32),
    size: (f32, f32),
    theme: &dyn Theme,
    _measurer: &TextMeasurer,
) {
    let fill = elem
        .color
        .as_deref()
        .map(resolve_color)
        .unwrap_or_else(|| theme.activity_shape_fill().to_string());
    let stroke = theme.activity_shape_stroke().to_string();
    match elem.kind {
        ElementKind::Actor => {
            let cx = pos.0 + size.0 / 2.0;
            let top = pos.1;
            // Head.
            prims.push(Primitive::Circle(Circle {
                cx,
                cy: top + 8.0,
                r: 8.0,
                fill,
                stroke: stroke.clone(),
                stroke_width: 0.5,
            }));
            // Body, arms, legs.
            for (x1, y1, x2, y2) in [
                (cx, top + 16.0, cx, top + 43.0),
                (cx - 13.0, top + 24.0, cx + 13.0, top + 24.0),
                (cx, top + 43.0, cx - 13.0, top + 58.0),
                (cx, top + 43.0, cx + 13.0, top + 58.0),
            ] {
                prims.push(Primitive::Line(Line {
                    x1,
                    y1,
                    x2,
                    y2,
                    stroke: stroke.clone(),
                    stroke_width: 0.5,
                }));
            }
            prims.push(Primitive::Text(Text {
                x: cx,
                y: top + ACTOR_FIG_H + ACTOR_LABEL_GAP,
                content: elem.display_name.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
            }));
        }
        ElementKind::UseCase => {
            let cx = pos.0 + size.0 / 2.0;
            let cy = pos.1 + size.1 / 2.0;
            prims.push(Primitive::Ellipse(Ellipse {
                cx,
                cy,
                rx: size.0 / 2.0,
                ry: size.1 / 2.0,
                fill,
                stroke: stroke.clone(),
                stroke_width: 0.5,
            }));
            prims.push(Primitive::Text(Text {
                x: cx,
                y: cy + 4.744,
                content: elem.display_name.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
            }));
        }
    }
}

fn draw_container(
    prims: &mut Vec<Primitive>,
    cont: &ContainerDef,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    theme: &dyn Theme,
) {
    prims.push(Primitive::Rect(Rect {
        x,
        y,
        width: w,
        height: h,
        fill: "none".into(),
        stroke: theme.activity_shape_stroke().to_string(),
        stroke_width: 1.0,
        rx: 2.5,
        ry: 2.5,
    }));
    prims.push(Primitive::Text(Text {
        x: x + w / 2.0,
        y: y + 15.54,
        content: cont.name.clone(),
        font_size: FONT,
        font_family: theme.font_family().to_string(),
        fill: "#000000".into(),
        anchor: TextAnchor::Middle,
        bold: true,
        italic: false,
    }));
}

/// Border point of an element for a line coming from `from`.
fn element_border(
    elem: &Element,
    pos: (f32, f32),
    size: (f32, f32),
    from: (f32, f32),
) -> (f32, f32) {
    let cx = pos.0 + size.0 / 2.0;
    let cy = pos.1 + size.1 / 2.0;
    match elem.kind {
        ElementKind::UseCase => {
            let rx = size.0 / 2.0;
            let ry = size.1 / 2.0;
            let dx = from.0 - cx;
            let dy = from.1 - cy;
            let denom = ((dx / rx).powi(2) + (dy / ry).powi(2)).sqrt().max(0.001);
            (cx + dx / denom, cy + dy / denom)
        }
        ElementKind::Actor => graph::clip_to_rect(from.0, from.1, pos.0, pos.1, size.0, size.1),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_relation(
    prims: &mut Vec<Primitive>,
    diagram: &UseCaseDiagram,
    rel: &UcRelation,
    a: usize,
    b: usize,
    abs: &[(f32, f32)],
    sizes: &[(f32, f32)],
    obstacles: &[(f32, f32, f32, f32)],
    theme: &dyn Theme,
) {
    let ea = &diagram.elements[a];
    let eb = &diagram.elements[b];
    let ca = (
        abs[a].0 + sizes[a].0 / 2.0,
        abs[a].1 + sizes[a].1 / 2.0,
    );
    let cb = (
        abs[b].0 + sizes[b].0 / 2.0,
        abs[b].1 + sizes[b].1 / 2.0,
    );
    let start = element_border(ea, abs[a], sizes[a], cb);
    let end = element_border(eb, abs[b], sizes[b], ca);
    let points = graph::route_with_detour(start, end, obstacles);

    let stroke = theme.activity_edge_color().to_string();
    for pair in points.windows(2) {
        if rel.dashed {
            prims.push(Primitive::DashedLine(DashedLine {
                x1: pair[0].0,
                y1: pair[0].1,
                x2: pair[1].0,
                y2: pair[1].1,
                stroke: stroke.clone(),
                stroke_width: 1.0,
                dash_array: "7,7".into(),
            }));
        } else {
            prims.push(Primitive::Line(Line {
                x1: pair[0].0,
                y1: pair[0].1,
                x2: pair[1].0,
                y2: pair[1].1,
                stroke: stroke.clone(),
                stroke_width: 1.0,
            }));
        }
    }

    let last = points[points.len() - 2];
    let dx = end.0 - last.0;
    let dy = end.1 - last.1;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    if rel.arrow {
        concave_arrowhead(prims, end, (ux, uy), &stroke);
    }
    if rel.back_arrow {
        let first = points[1];
        let dx0 = start.0 - first.0;
        let dy0 = start.1 - first.1;
        let len0 = (dx0 * dx0 + dy0 * dy0).sqrt().max(0.001);
        concave_arrowhead(prims, start, (dx0 / len0, dy0 / len0), &stroke);
    }

    if let Some(label) = &rel.label {
        let display = label.replace("<<", "\u{ab}").replace(">>", "\u{bb}");
        let seg = points.len() / 2 - 1;
        let mid = (
            (points[seg].0 + points[seg + 1].0) / 2.0,
            (points[seg].1 + points[seg + 1].1) / 2.0,
        );
        prims.push(Primitive::Text(Text {
            x: mid.0 + 4.0,
            y: mid.1 + 4.0,
            content: display,
            font_size: LABEL_FONT,
            font_family: theme.font_family().to_string(),
            fill: theme.activity_text_color().to_string(),
            anchor: TextAnchor::Start,
            bold: false,
            italic: false,
        }));
    }
}

fn concave_arrowhead(
    prims: &mut Vec<Primitive>,
    tip: (f32, f32),
    dir: (f32, f32),
    stroke: &str,
) {
    let (ux, uy) = dir;
    let (px, py) = (-uy, ux);
    let bx = tip.0 - ux * 9.0;
    let by = tip.1 - uy * 9.0;
    prims.push(Primitive::Polygon(Polygon {
        points: vec![
            (tip.0, tip.1),
            (bx + px * 4.0, by + py * 4.0),
            (tip.0 - ux * 5.0, tip.1 - uy * 5.0),
            (bx - px * 4.0, by - py * 4.0),
        ],
        fill: stroke.to_string(),
        stroke: stroke.to_string(),
        stroke_width: 1.0,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::usecase::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_basic_layout() {
        let d = parse("actor Customer\nusecase (Browse) as UC1\nCustomer --> UC1\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid.width > 0.0 && laid.height > 0.0);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Ellipse(_))));
        // Actor head circle.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Circle(c) if (c.r - 8.0).abs() < 0.01)));
    }

    #[test]
    fn test_ellipse_radii_aspect_cap() {
        // Wide text hits the aspect cap.
        let (rx, ry) = ellipse_radii(90.0);
        assert!((rx - 5.0 * (ry - 3.0) - 3.0).abs() < 0.01);
        // Narrow text uses the sqrt(2) rule.
        let (rx2, ry2) = ellipse_radii(40.0);
        assert!((rx2 - (20.0 * std::f32::consts::SQRT_2 + 3.0)).abs() < 0.01);
        assert!(ry2 > 3.0);
    }

    #[test]
    fn test_guillemet_label() {
        let d = parse("usecase (A) as U1\nusecase (B) as U2\nU2 ..> U1 : <<include>>\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid.primitives.iter().any(
            |p| matches!(p, Primitive::Text(t) if t.content.contains('\u{ab}') && t.content.contains('\u{bb}'))
        ));
    }

    #[test]
    fn test_left_to_right() {
        let d = parse("left to right direction\nactor A\nusecase (Do it) as U\nA --> U\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        // Actor should be left of the use case ellipse.
        let head_cx = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Circle(c) if (c.r - 8.0).abs() < 0.01 => Some(c.cx),
                _ => None,
            })
            .unwrap();
        let uc_cx = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Ellipse(e) => Some(e.cx),
                _ => None,
            })
            .unwrap();
        assert!(head_cx < uc_cx);
    }
}
