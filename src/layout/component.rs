/// Layout for component diagrams, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::component::*;
use crate::layout::graph::{self, GraphEdge, GraphNode};
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const FONT: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;
const BOX_H: f32 = 46.4883;
const BOX_TEXT_BASELINE: f32 = 33.5352;
const IFACE_R: f32 = 8.0;
const RANK_GAP: f32 = 60.0;
const LABEL_EXTRA: f32 = 17.0;
const SIBLING_GAP: f32 = 35.0;
const MARGIN: f32 = 7.0;
const PKG_TAB_H: f32 = 22.5;
const PKG_PAD: f32 = 16.0;
const PKG_TAB_TO_CHILD: f32 = 12.5;

pub fn layout_with_theme(diagram: &ComponentDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);

    let sizes: Vec<(f32, f32)> = diagram
        .components
        .iter()
        .map(|c| comp_size(c, &measurer))
        .collect();

    let comp_index = |name: &str| diagram.components.iter().position(|c| c.name == name);
    let mut pkg_of: Vec<Option<usize>> = vec![None; diagram.components.len()];
    for (pi, pkg) in diagram.packages.iter().enumerate() {
        for m in &pkg.members {
            if let Some(ci) = comp_index(m) {
                pkg_of[ci] = Some(pi);
            }
        }
    }

    // Inner layouts per package.
    let mut rel_pos: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.components.len()];
    let mut pkg_size: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.packages.len()];
    for (pi, pkg) in diagram.packages.iter().enumerate() {
        let members: Vec<usize> = (0..diagram.components.len())
            .filter(|&ci| pkg_of[ci] == Some(pi))
            .collect();
        if members.is_empty() {
            pkg_size[pi] = (100.0, 60.0);
            continue;
        }
        let mut nodes: Vec<GraphNode> = members
            .iter()
            .map(|&ci| GraphNode::new(sizes[ci].0, sizes[ci].1))
            .collect();
        let edges = collect_edges(diagram, |name| {
            comp_index(name)
                .filter(|ci| pkg_of[*ci] == Some(pi))
                .and_then(|ci| members.iter().position(|&m| m == ci))
        });
        let result = graph::layout(&mut nodes, &edges, SIBLING_GAP, RANK_GAP, LABEL_EXTRA);
        for (k, &ci) in members.iter().enumerate() {
            rel_pos[ci] = (nodes[k].x, nodes[k].y);
        }
        let name_w = measurer.measure_width(&pkg.name);
        pkg_size[pi] = (
            (result.width + PKG_PAD * 2.0).max(name_w + 30.0),
            PKG_TAB_H + PKG_TAB_TO_CHILD + result.height + PKG_PAD,
        );
    }

    // Top-level graph.
    let mut top_of: Vec<usize> = vec![0; diagram.components.len()];
    let mut top_nodes: Vec<GraphNode> = Vec::new();
    for &(pw, ph) in &pkg_size {
        top_nodes.push(GraphNode::new(pw, ph));
    }
    for ci in 0..diagram.components.len() {
        match pkg_of[ci] {
            Some(pi) => top_of[ci] = pi,
            None => {
                top_nodes.push(GraphNode::new(sizes[ci].0, sizes[ci].1));
                top_of[ci] = top_nodes.len() - 1;
            }
        }
    }
    let top_edges = {
        let mut edges = Vec::new();
        for rel in &diagram.relations {
            let (Some(a), Some(b)) = (comp_index(&rel.from), comp_index(&rel.to)) else {
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

    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let mut abs: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.components.len()];
    for ci in 0..diagram.components.len() {
        let top = &top_nodes[top_of[ci]];
        abs[ci] = match pkg_of[ci] {
            Some(_) => (
                MARGIN + top.x + PKG_PAD + rel_pos[ci].0,
                MARGIN + title_offset + top.y + PKG_TAB_H + PKG_TAB_TO_CHILD + rel_pos[ci].1,
            ),
            None => (MARGIN + top.x, MARGIN + title_offset + top.y),
        };
    }

    let mut prims: Vec<Primitive> = Vec::new();

    for (pi, pkg) in diagram.packages.iter().enumerate() {
        draw_package(
            &mut prims,
            pkg,
            MARGIN + top_nodes[pi].x,
            MARGIN + title_offset + top_nodes[pi].y,
            pkg_size[pi].0,
            pkg_size[pi].1,
            theme,
            &measurer,
        );
    }

    for rel in &diagram.relations {
        let (Some(a), Some(b)) = (comp_index(&rel.from), comp_index(&rel.to)) else {
            continue;
        };
        let obstacles: Vec<(f32, f32, f32, f32)> = (0..diagram.components.len())
            .filter(|&k| k != a && k != b)
            .map(|k| (abs[k].0, abs[k].1, sizes[k].0, sizes[k].1))
            .collect();
        draw_relation(&mut prims, diagram, rel, a, b, &abs, &sizes, &obstacles, theme);
    }

    for (ci, comp) in diagram.components.iter().enumerate() {
        draw_component(&mut prims, comp, abs[ci], sizes[ci], theme, &measurer);
    }

    let width = MARGIN + top_result.width + 14.0;
    let height = MARGIN + title_offset + top_result.height + 14.0;

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

fn collect_edges(
    diagram: &ComponentDiagram,
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

fn comp_size(comp: &CompDef, measurer: &TextMeasurer) -> (f32, f32) {
    match comp.kind {
        CompKind::Component => (
            measurer.measure_width(&comp.display_name) + 40.0,
            BOX_H,
        ),
        CompKind::Interface => {
            // Circle plus the label below it.
            let label_w = measurer.measure_width(&comp.display_name);
            (label_w.max(IFACE_R * 2.0), IFACE_R * 2.0 + 26.5)
        }
    }
}

fn draw_component(
    prims: &mut Vec<Primitive>,
    comp: &CompDef,
    pos: (f32, f32),
    size: (f32, f32),
    theme: &dyn Theme,
    _measurer: &TextMeasurer,
) {
    let fill = comp
        .color
        .as_deref()
        .map(resolve_color)
        .unwrap_or_else(|| theme.activity_shape_fill().to_string());
    let stroke = theme.activity_shape_stroke().to_string();
    match comp.kind {
        CompKind::Component => {
            let (x, y) = pos;
            let (w, h) = size;
            prims.push(Primitive::Rect(Rect {
                x,
                y,
                width: w,
                height: h,
                fill: fill.clone(),
                stroke: stroke.clone(),
                stroke_width: 0.5,
                rx: 2.5,
                ry: 2.5,
            }));
            prims.push(Primitive::Text(Text {
                x: x + 15.0,
                y: y + BOX_TEXT_BASELINE,
                content: comp.display_name.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Start,
                bold: false,
                italic: false,
            }));
            // Component icon: body 15x10 at (right-20, top+5), two 4x2 tabs.
            let bx = x + w - 20.0;
            let by = y + 5.0;
            for (rx_, ry_, rw, rh) in [
                (bx, by, 15.0, 10.0),
                (bx - 2.0, by + 2.0, 4.0, 2.0),
                (bx - 2.0, by + 6.0, 4.0, 2.0),
            ] {
                prims.push(Primitive::Rect(Rect {
                    x: rx_,
                    y: ry_,
                    width: rw,
                    height: rh,
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                    stroke_width: 0.5,
                    rx: 0.0,
                    ry: 0.0,
                }));
            }
        }
        CompKind::Interface => {
            let cx = pos.0 + size.0 / 2.0;
            let cy = pos.1 + IFACE_R;
            prims.push(Primitive::Circle(Circle {
                cx,
                cy,
                r: IFACE_R,
                fill,
                stroke: stroke.clone(),
                stroke_width: 0.5,
            }));
            prims.push(Primitive::Text(Text {
                x: cx,
                y: cy + 30.5,
                content: comp.display_name.clone(),
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

#[allow(clippy::too_many_arguments)]
fn draw_package(
    prims: &mut Vec<Primitive>,
    pkg: &CompPackage,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) {
    let tab_w = measurer.measure_width(&pkg.name) + 10.0;
    let d = format!(
        "M{},{} L{},{} L{},{} L{},{} L{},{} L{},{} Z",
        x,
        y,
        x + tab_w,
        y,
        x + tab_w + 7.0,
        y + PKG_TAB_H,
        x + w,
        y + PKG_TAB_H,
        x + w,
        y + h,
        x,
        y + h,
    );
    prims.push(Primitive::Path(Path {
        d,
        fill: "none".into(),
        stroke: theme.partition_border_color().to_string(),
        stroke_width: 1.5,
        dashed: false,
    }));
    prims.push(Primitive::Line(Line {
        x1: x,
        y1: y + PKG_TAB_H,
        x2: x + tab_w + 7.0,
        y2: y + PKG_TAB_H,
        stroke: theme.partition_border_color().to_string(),
        stroke_width: 1.5,
    }));
    prims.push(Primitive::Text(Text {
        x: x + 4.0,
        y: y + 15.535,
        content: pkg.name.clone(),
        font_size: FONT,
        font_family: theme.font_family().to_string(),
        fill: "#000000".into(),
        anchor: TextAnchor::Start,
        bold: true,
        italic: false,
    }));
}

/// Border point of a component for a line coming from `from`.
fn comp_border(
    comp: &CompDef,
    pos: (f32, f32),
    size: (f32, f32),
    from: (f32, f32),
) -> (f32, f32) {
    match comp.kind {
        CompKind::Interface => {
            // Clip to the circle, not the label box.
            let cx = pos.0 + size.0 / 2.0;
            let cy = pos.1 + IFACE_R;
            let dx = from.0 - cx;
            let dy = from.1 - cy;
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            (
                cx + dx / len * (IFACE_R + 1.0),
                cy + dy / len * (IFACE_R + 1.0),
            )
        }
        CompKind::Component => {
            graph::clip_to_rect(from.0, from.1, pos.0, pos.1, size.0, size.1)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_relation(
    prims: &mut Vec<Primitive>,
    diagram: &ComponentDiagram,
    rel: &CompRelation,
    a: usize,
    b: usize,
    abs: &[(f32, f32)],
    sizes: &[(f32, f32)],
    obstacles: &[(f32, f32, f32, f32)],
    theme: &dyn Theme,
) {
    let ca_comp = &diagram.components[a];
    let cb_comp = &diagram.components[b];
    // Interfaces aim at their circle center.
    let center = |i: usize| -> (f32, f32) {
        let comp = &diagram.components[i];
        match comp.kind {
            CompKind::Interface => (abs[i].0 + sizes[i].0 / 2.0, abs[i].1 + IFACE_R),
            CompKind::Component => (
                abs[i].0 + sizes[i].0 / 2.0,
                abs[i].1 + sizes[i].1 / 2.0,
            ),
        }
    };
    let ca = center(a);
    let cb = center(b);
    let start = comp_border(ca_comp, abs[a], sizes[a], cb);
    let end = comp_border(cb_comp, abs[b], sizes[b], ca);
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

    if rel.arrow {
        let last = points[points.len() - 2];
        let dx = end.0 - last.0;
        let dy = end.1 - last.1;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        concave_arrowhead(prims, end, (dx / len, dy / len), &stroke);
    }
    if rel.back_arrow {
        let first = points[1];
        let dx = start.0 - first.0;
        let dy = start.1 - first.1;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        concave_arrowhead(prims, start, (dx / len, dy / len), &stroke);
    }

    if let Some(label) = &rel.label {
        let seg = points.len() / 2 - 1;
        let mid = (
            (points[seg].0 + points[seg + 1].0) / 2.0,
            (points[seg].1 + points[seg + 1].1) / 2.0,
        );
        prims.push(Primitive::Text(Text {
            x: mid.0 + 4.0,
            y: mid.1 + 5.0,
            content: label.clone(),
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
    use crate::parser::component::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_basic_layout() {
        let d = parse("[Web UI] --> [API Server] : HTTPS\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid.width > 0.0 && laid.height > 0.0);
        // Two component boxes + icon rects (3 per component).
        let rects = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(_)))
            .count();
        assert_eq!(rects, 8);
    }

    #[test]
    fn test_component_box_size() {
        let d = parse("[X]\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) if r.rx == 2.5 => Some(r),
                _ => None,
            })
            .unwrap();
        assert!((rect.height - BOX_H).abs() < 0.01);
    }

    #[test]
    fn test_lollipop() {
        let d = parse("interface Auth\nAuth - [API Server]\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Circle(c) if (c.r - IFACE_R).abs() < 0.01)));
        // No arrowheads for a plain association.
        assert!(!laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Polygon(_))));
    }

    #[test]
    fn test_package_frame() {
        let d = parse("package \"Backend\" {\n  [API Server]\n}\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Path(_))));
    }
}
