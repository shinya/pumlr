/// Layout for state diagrams, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::state::*;
use crate::layout::graph::{self, GraphEdge, GraphNode};
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const TITLE_BAND_H: f32 = 26.4883;
const NAME_BASELINE: f32 = 18.5352;
const ROW_H: f32 = 26.4883;
const MIN_SIZE: f32 = 50.0;
const CORNER_R: f32 = 12.5;
const START_R: f32 = 10.0;
const END_R: f32 = 11.0;
const FONT: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;
const RANK_GAP_TOP: f32 = 61.0;
const RANK_GAP_INNER: f32 = 37.0;
const LABEL_EXTRA: f32 = 16.0;
const SIBLING_GAP: f32 = 35.0;
const MARGIN: f32 = 7.0;
const COMP_PAD_X: f32 = 12.0;
const COMP_PAD_BOTTOM: f32 = 13.0;
const COMP_BAND_TO_CHILD: f32 = 11.0;
// History pseudo-state: circle r=11 with a centered 14px "H" (measured).
const HIST_R: f32 = 11.0;
const HIST_BASELINE: f32 = 5.291;
// Concurrent regions: dashed separator line between stacked regions.
const REGION_SEP_GAP_ABOVE: f32 = 8.0;
const REGION_SEP_GAP_BELOW: f32 = 6.0;
const REGION_SEP_INSET_LEFT: f32 = 5.0;
const REGION_SEP_INSET_RIGHT: f32 = 7.0;
const REGION_SEP_WIDTH: f32 = 1.5;
const REGION_SEP_DASH: &str = "8,10";

/// A positioned node: state box, composite frame, or pseudo circle.
#[derive(Debug, Clone)]
struct PlacedNode {
    /// State name, or pseudo name (`start$scope` / `end$scope`).
    name: String,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    kind: NodeKind,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum NodeKind {
    State,
    Composite,
    Start,
    End,
    /// Shallow history pseudo-state (circled H).
    Hist,
    /// Dashed separator between concurrent regions (h = 0; x/w are fixed up
    /// to span the enclosing composite when children are emitted).
    RegionSep,
}

pub fn layout_with_theme(diagram: &StateDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT);
    let mut placed: Vec<PlacedNode> = Vec::new();
    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let (w, h) = layout_scope(
        diagram,
        None,
        &measurer,
        MARGIN,
        MARGIN + title_offset,
        RANK_GAP_TOP,
        &mut placed,
    );

    let mut prims: Vec<Primitive> = Vec::new();
    let offsets = parallel_offsets(diagram);

    // Pre-pass: label extents may stick out on the left/right; shift content
    // right if needed and widen the canvas.
    let label_measurer = TextMeasurer::new(LABEL_FONT);
    let mut min_ext = MARGIN;
    let mut max_ext = MARGIN + w;
    for (i, tr) in diagram.transitions.iter().enumerate() {
        let Some(label) = &tr.label else { continue };
        let Some(geom) = transition_geometry(diagram, &placed, i, tr, offsets[i]) else {
            continue;
        };
        let lw = label_measurer.measure_width(label);
        let (lmin, lmax) = if geom.side < 0.0 {
            (geom.label_pos.0 - 4.0 - lw, geom.label_pos.0)
        } else {
            (geom.label_pos.0, geom.label_pos.0 + 4.0 + lw)
        };
        min_ext = min_ext.min(lmin);
        max_ext = max_ext.max(lmax);
    }
    let shift = (MARGIN - min_ext).max(0.0);
    for node in &mut placed {
        node.x += shift;
    }

    for (i, tr) in diagram.transitions.iter().enumerate() {
        if let Some(geom) = transition_geometry(diagram, &placed, i, tr, offsets[i]) {
            draw_transition(&mut prims, tr, &geom, theme);
        }
    }

    for node in &placed {
        draw_node(&mut prims, diagram, node, theme, &measurer);
    }

    let width = max_ext + shift + 14.0;
    let height = MARGIN + title_offset + h + 13.0;

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

fn find_node<'a>(placed: &'a [PlacedNode], name: &str) -> Option<&'a PlacedNode> {
    placed.iter().find(|n| n.name == name)
}

/// Perpendicular offsets so edges sharing the same node pair (either
/// direction) don't overlap.
fn parallel_offsets(diagram: &StateDiagram) -> Vec<f32> {
    let pair_key = |a: &str, b: &str| {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    };
    diagram
        .transitions
        .iter()
        .enumerate()
        .map(|(i, tr)| {
            let key = pair_key(&tr.from, &tr.to);
            let group: Vec<usize> = diagram
                .transitions
                .iter()
                .enumerate()
                .filter(|(_, t)| pair_key(&t.from, &t.to) == key)
                .map(|(j, _)| j)
                .collect();
            if group.len() > 1 {
                let pos = group.iter().position(|&j| j == i).unwrap() as f32;
                let spread = (pos - (group.len() as f32 - 1.0) / 2.0) * 18.0;
                // The perpendicular flips with the travel direction, so keep
                // the spread in a canonical frame or opposite edges land on
                // the same side.
                if tr.from > tr.to {
                    -spread
                } else {
                    spread
                }
            } else {
                0.0
            }
        })
        .collect()
}

/// Widen composite inner content so labels of transitions that live entirely
/// inside it stay within the frame: shifts `inner` right if labels stick out
/// on the left and returns the new content width.
fn expand_for_inner_labels(
    diagram: &StateDiagram,
    inner: &mut [PlacedNode],
    iw: f32,
) -> f32 {
    let label_measurer = TextMeasurer::new(LABEL_FONT);
    let offsets = parallel_offsets(diagram);
    let mut min_x = 0.0f32;
    let mut max_x = iw;
    for (i, tr) in diagram.transitions.iter().enumerate() {
        let Some(label) = &tr.label else { continue };
        if find_node(inner, &tr.from).is_none() || find_node(inner, &tr.to).is_none() {
            continue;
        }
        let Some(geom) = transition_geometry(diagram, inner, i, tr, offsets[i]) else {
            continue;
        };
        let lw = label_measurer.measure_width(label);
        if geom.side < 0.0 {
            min_x = min_x.min(geom.label_pos.0 - 4.0 - lw);
        } else {
            max_x = max_x.max(geom.label_pos.0 + 4.0 + lw);
        }
    }
    for node in inner.iter_mut() {
        node.x -= min_x;
    }
    max_x - min_x
}

/// Lay out the direct contents of `scope` and append placed nodes with
/// absolute origin (ox, oy). Returns the content extent (w, h).
#[allow(clippy::too_many_arguments)]
fn layout_scope(
    diagram: &StateDiagram,
    scope: Option<&str>,
    measurer: &TextMeasurer,
    ox: f32,
    oy: f32,
    rank_gap: f32,
    placed: &mut Vec<PlacedNode>,
) -> (f32, f32) {
    let scope_name = scope.unwrap_or("");
    let children = diagram.children_of(scope);

    // Local node list: pseudo start/history/end + child states.
    let start_name = format!("{}{}", START_PREFIX, scope_name);
    let end_name = format!("{}{}", END_PREFIX, scope_name);
    let hist_name = format!("{}{}", HIST_PREFIX, scope_name);
    let uses_start = diagram
        .transitions
        .iter()
        .any(|t| t.from == start_name || t.to == start_name);
    let uses_end = diagram
        .transitions
        .iter()
        .any(|t| t.from == end_name || t.to == end_name);
    let uses_hist = diagram
        .transitions
        .iter()
        .any(|t| t.from == hist_name || t.to == hist_name);

    #[derive(Clone)]
    enum Local {
        Start,
        Hist,
        End,
        State(usize), // index into diagram.states
    }
    let mut locals: Vec<(String, Local)> = Vec::new();
    if uses_start {
        locals.push((start_name.clone(), Local::Start));
    }
    if uses_hist {
        locals.push((hist_name.clone(), Local::Hist));
    }
    for st in &children {
        let idx = diagram
            .states
            .iter()
            .position(|s| s.name == st.name)
            .unwrap();
        locals.push((st.name.clone(), Local::State(idx)));
    }
    if uses_end {
        locals.push((end_name.clone(), Local::End));
    }

    // Recursively size composite children first (place at origin 0,0; shift later).
    // We lay out composites into a scratch list to learn their size.
    let mut sizes: Vec<(f32, f32)> = Vec::new();
    let mut comp_scratch: Vec<(String, Vec<PlacedNode>, f32, f32)> = Vec::new();
    for (name, local) in &locals {
        let size = match local {
            Local::Start => (START_R * 2.0, START_R * 2.0),
            Local::Hist => (HIST_R * 2.0, HIST_R * 2.0),
            Local::End => (END_R * 2.0, END_R * 2.0),
            Local::State(idx) => {
                let st = &diagram.states[*idx];
                let st_children = diagram.children_of(Some(&st.name));
                let regions: Vec<&StateDef> = st_children
                    .iter()
                    .filter(|c| c.is_region)
                    .copied()
                    .collect();
                if !regions.is_empty() {
                    // Concurrent composite: stack each region vertically,
                    // separated by dashed lines.
                    let mut region_layouts: Vec<(Vec<PlacedNode>, f32, f32)> = Vec::new();
                    let mut iw = 0.0f32;
                    for region in &regions {
                        let mut nodes: Vec<PlacedNode> = Vec::new();
                        let (rw, rh) = layout_scope(
                            diagram,
                            Some(&region.name),
                            measurer,
                            0.0,
                            0.0,
                            RANK_GAP_INNER,
                            &mut nodes,
                        );
                        iw = iw.max(rw);
                        region_layouts.push((nodes, rw, rh));
                    }
                    let mut inner: Vec<PlacedNode> = Vec::new();
                    let mut y = 0.0f32;
                    for (ri, (nodes, rw, rh)) in region_layouts.into_iter().enumerate() {
                        if ri > 0 {
                            y += REGION_SEP_GAP_ABOVE;
                            inner.push(PlacedNode {
                                name: format!("sep${}${}", st.name, ri),
                                x: 0.0,
                                y,
                                w: 0.0,
                                h: 0.0,
                                kind: NodeKind::RegionSep,
                            });
                            y += REGION_SEP_GAP_BELOW;
                        }
                        let dx = (iw - rw) / 2.0;
                        for mut node in nodes {
                            node.x += dx;
                            node.y += y;
                            inner.push(node);
                        }
                        y += rh;
                    }
                    let ih = y;
                    let iw = expand_for_inner_labels(diagram, &mut inner, iw);
                    let name_w = measurer.measure_width(&st.display_name);
                    let w = (iw + COMP_PAD_X * 2.0).max(name_w + 20.0).max(MIN_SIZE);
                    let h = TITLE_BAND_H + COMP_BAND_TO_CHILD + ih + COMP_PAD_BOTTOM;
                    comp_scratch.push((name.clone(), inner, iw, ih));
                    (w, h)
                } else if st.composite || !st_children.is_empty() {
                    let mut inner: Vec<PlacedNode> = Vec::new();
                    let (iw, ih) = layout_scope(
                        diagram,
                        Some(&st.name),
                        measurer,
                        0.0,
                        0.0,
                        RANK_GAP_INNER,
                        &mut inner,
                    );
                    let iw = expand_for_inner_labels(diagram, &mut inner, iw);
                    let name_w = measurer.measure_width(&st.display_name);
                    let w = (iw + COMP_PAD_X * 2.0).max(name_w + 20.0).max(MIN_SIZE);
                    let h = TITLE_BAND_H + COMP_BAND_TO_CHILD + ih + COMP_PAD_BOTTOM;
                    comp_scratch.push((name.clone(), inner, iw, ih));
                    (w, h)
                } else {
                    state_box_size(st, measurer)
                }
            }
        };
        sizes.push(size);
    }

    // Graph edges: transitions whose lifted endpoints are both local.
    let resolve_local = |endpoint: &str| -> Option<usize> {
        // Direct local?
        if let Some(i) = locals.iter().position(|(n, _)| n == endpoint) {
            return Some(i);
        }
        // Pseudo of a deeper scope or deeper state: lift to the direct child
        // of this scope that contains it.
        let mut state_name: Option<String> = None;
        if let Some(s) = endpoint.strip_prefix(START_PREFIX) {
            if !s.is_empty() {
                state_name = Some(s.to_string());
            }
        } else if let Some(s) = endpoint.strip_prefix(END_PREFIX) {
            if !s.is_empty() {
                state_name = Some(s.to_string());
            }
        } else if let Some(s) = endpoint.strip_prefix(HIST_PREFIX) {
            if !s.is_empty() {
                state_name = Some(s.to_string());
            }
        } else {
            state_name = Some(endpoint.to_string());
        }
        let mut current = state_name?;
        loop {
            if let Some(i) = locals.iter().position(|(n, _)| n == &current) {
                return Some(i);
            }
            current = diagram.state(&current)?.parent.clone()?;
        }
    };

    let mut edges: Vec<GraphEdge> = Vec::new();
    for tr in &diagram.transitions {
        let (Some(a), Some(b)) = (resolve_local(&tr.from), resolve_local(&tr.to)) else {
            continue;
        };
        if a == b {
            continue;
        }
        // Only add if at least one endpoint is truly local (avoid duplicating
        // edges that live entirely in a deeper scope).
        let direct_a = locals.iter().any(|(n, _)| n == &tr.from)
            || diagram
                .state(&tr.from)
                .map(|s| s.parent.as_deref() == scope)
                .unwrap_or(false)
            || tr.from == start_name
            || tr.from == end_name;
        let direct_b = locals.iter().any(|(n, _)| n == &tr.to)
            || diagram
                .state(&tr.to)
                .map(|s| s.parent.as_deref() == scope)
                .unwrap_or(false)
            || tr.to == start_name
            || tr.to == end_name;
        if !direct_a && !direct_b {
            continue;
        }
        edges.push(GraphEdge {
            from: a,
            to: b,
            min_len: if tr.rank_len <= 1 { 0 } else { 1 },
            labeled: tr.label.is_some(),
        });
    }

    let mut nodes: Vec<GraphNode> = sizes
        .iter()
        .map(|&(w, h)| GraphNode::new(w, h))
        .collect();
    let result = graph::layout(&mut nodes, &edges, SIBLING_GAP, rank_gap, LABEL_EXTRA);

    // Emit placed nodes (absolute coordinates).
    for (i, (name, local)) in locals.iter().enumerate() {
        let x = ox + nodes[i].x;
        let y = oy + nodes[i].y;
        let (w, h) = sizes[i];
        let kind = match local {
            Local::Start => NodeKind::Start,
            Local::Hist => NodeKind::Hist,
            Local::End => NodeKind::End,
            Local::State(idx) => {
                let st = &diagram.states[*idx];
                if comp_scratch.iter().any(|(n, _, _, _)| n == &st.name) {
                    NodeKind::Composite
                } else {
                    NodeKind::State
                }
            }
        };
        placed.push(PlacedNode {
            name: name.clone(),
            x,
            y,
            w,
            h,
            kind,
        });
        // Shift composite children to absolute coordinates.
        if kind == NodeKind::Composite {
            if let Some((_, inner, iw, _)) = comp_scratch.iter().find(|(n, _, _, _)| n == name) {
                let content_x = x + (w - iw) / 2.0;
                let content_y = y + TITLE_BAND_H + COMP_BAND_TO_CHILD;
                // Separators of *this* composite span its frame width;
                // deeper ones were already sized and only need shifting.
                let sep_prefix = format!("sep${}$", name);
                for child in inner {
                    let mut c = child.clone();
                    if c.kind == NodeKind::RegionSep && c.name.starts_with(&sep_prefix) {
                        c.x = x + REGION_SEP_INSET_LEFT;
                        c.w = w - REGION_SEP_INSET_LEFT - REGION_SEP_INSET_RIGHT;
                        c.y += content_y;
                    } else {
                        c.x += content_x;
                        c.y += content_y;
                    }
                    placed.push(c);
                }
            }
        }
    }

    (result.width, result.height)
}

fn state_box_size(st: &StateDef, measurer: &TextMeasurer) -> (f32, f32) {
    let name_w = measurer.measure_width(&st.display_name);
    let desc_w = st
        .descriptions
        .iter()
        .map(|d| measurer.measure_width(d))
        .fold(0.0f32, f32::max);
    let w = (name_w + 20.0).max(desc_w + 20.0).max(MIN_SIZE);
    let h = if st.descriptions.is_empty() {
        MIN_SIZE
    } else {
        TITLE_BAND_H + st.descriptions.len() as f32 * ROW_H
    };
    (w, h)
}

fn draw_node(
    prims: &mut Vec<Primitive>,
    diagram: &StateDiagram,
    node: &PlacedNode,
    theme: &dyn Theme,
    _measurer: &TextMeasurer,
) {
    match node.kind {
        NodeKind::Start => {
            prims.push(Primitive::Circle(Circle {
                cx: node.x + START_R,
                cy: node.y + START_R,
                r: START_R,
                fill: theme.activity_start_stop_color().to_string(),
                stroke: theme.activity_start_stop_color().to_string(),
                stroke_width: 1.0,
            }));
        }
        NodeKind::Hist => {
            let cx = node.x + HIST_R;
            let cy = node.y + HIST_R;
            prims.push(Primitive::Circle(Circle {
                cx,
                cy,
                r: HIST_R,
                fill: theme.activity_shape_fill().to_string(),
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: 0.5,
            }));
            prims.push(Primitive::Text(Text {
                x: cx,
                y: cy + HIST_BASELINE,
                content: "H".into(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
                underline: false,
            }));
        }
        NodeKind::RegionSep => {
            prims.push(Primitive::DashedLine(DashedLine {
                x1: node.x,
                y1: node.y,
                x2: node.x + node.w,
                y2: node.y,
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: REGION_SEP_WIDTH,
                dash_array: REGION_SEP_DASH.into(),
            }));
        }
        NodeKind::End => {
            let cx = node.x + END_R;
            let cy = node.y + END_R;
            prims.push(Primitive::Circle(Circle {
                cx,
                cy,
                r: END_R,
                fill: "none".into(),
                stroke: theme.activity_start_stop_color().to_string(),
                stroke_width: 1.0,
            }));
            prims.push(Primitive::Circle(Circle {
                cx,
                cy,
                r: 6.0,
                fill: theme.activity_start_stop_color().to_string(),
                stroke: theme.activity_start_stop_color().to_string(),
                stroke_width: 1.0,
            }));
        }
        NodeKind::State => {
            let st = diagram.state(&node.name).expect("placed state exists");
            let fill = st
                .color
                .as_deref()
                .map(resolve_color)
                .unwrap_or_else(|| theme.activity_shape_fill().to_string());
            prims.push(Primitive::Rect(Rect {
                x: node.x,
                y: node.y,
                width: node.w,
                height: node.h,
                fill,
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: 0.5,
                rx: CORNER_R,
                ry: CORNER_R,
            }));
            prims.push(Primitive::Line(Line {
                x1: node.x,
                y1: node.y + TITLE_BAND_H,
                x2: node.x + node.w,
                y2: node.y + TITLE_BAND_H,
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: 0.5,
            }));
            prims.push(Primitive::Text(Text {
                x: node.x + node.w / 2.0,
                y: node.y + NAME_BASELINE,
                content: st.display_name.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
                underline: false,
            }));
            for (i, desc) in st.descriptions.iter().enumerate() {
                prims.push(Primitive::Text(Text {
                    x: node.x + 5.0,
                    y: node.y + TITLE_BAND_H + NAME_BASELINE + i as f32 * ROW_H,
                    content: desc.clone(),
                    font_size: FONT,
                    font_family: theme.font_family().to_string(),
                    fill: theme.activity_text_color().to_string(),
                    anchor: TextAnchor::Start,
                    bold: false,
                    italic: false,
                    underline: false,
                }));
            }
        }
        NodeKind::Composite => {
            let st = diagram.state(&node.name).expect("placed state exists");
            // Frame (no fill).
            prims.push(Primitive::Rect(Rect {
                x: node.x,
                y: node.y,
                width: node.w,
                height: node.h,
                fill: "none".into(),
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: 0.5,
                rx: CORNER_R,
                ry: CORNER_R,
            }));
            // Title band: filled path with rounded top corners.
            let (x, y, w) = (node.x, node.y, node.w);
            let r = CORNER_R;
            let band_fill = st
                .color
                .as_deref()
                .map(resolve_color)
                .unwrap_or_else(|| theme.activity_shape_fill().to_string());
            prims.push(Primitive::Path(Path {
                d: format!(
                    "M{},{} A{},{} 0 0 1 {},{} L{},{} A{},{} 0 0 1 {},{} L{},{} L{},{} Z",
                    x,
                    y + r,
                    r,
                    r,
                    x + r,
                    y,
                    x + w - r,
                    y,
                    r,
                    r,
                    x + w,
                    y + r,
                    x + w,
                    y + TITLE_BAND_H,
                    x,
                    y + TITLE_BAND_H,
                ),
                fill: band_fill,
                stroke: "none".into(),
                stroke_width: 0.0,
                dashed: false,
            }));
            prims.push(Primitive::Line(Line {
                x1: x,
                y1: y + TITLE_BAND_H,
                x2: x + w,
                y2: y + TITLE_BAND_H,
                stroke: theme.activity_shape_stroke().to_string(),
                stroke_width: 0.5,
            }));
            prims.push(Primitive::Text(Text {
                x: x + w / 2.0,
                y: y + NAME_BASELINE,
                content: st.display_name.clone(),
                font_size: FONT,
                font_family: theme.font_family().to_string(),
                fill: theme.activity_text_color().to_string(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
                underline: false,
            }));
        }
    }
}

/// Border point of `node` for a line aimed from `aim` towards `own_center`
/// (own_center may be perpendicular-shifted for parallel edges).
fn border_point(node: &PlacedNode, aim: (f32, f32), own_center: (f32, f32)) -> (f32, f32) {
    match node.kind {
        NodeKind::Start | NodeKind::End | NodeKind::Hist => {
            let r = node.w / 2.0;
            let cx = node.x + r;
            let cy = node.y + r;
            let dx = cx - aim.0;
            let dy = cy - aim.1;
            let len = (dx * dx + dy * dy).sqrt().max(0.001);
            (cx - dx / len * r, cy - dy / len * r)
        }
        _ => graph::clip_segment_to_rect(
            aim.0,
            aim.1,
            own_center.0,
            own_center.1,
            node.x,
            node.y,
            node.w,
            node.h,
        ),
    }
}

struct TransitionGeom {
    /// Polyline from source border to target border (2 or 4 points).
    points: Vec<(f32, f32)>,
    label_pos: (f32, f32),
    /// Horizontal component of the parallel-edge shift; decides which side
    /// of the line the label goes on.
    side: f32,
}

/// Compute the polyline for transition `ti`, detouring around boxes that the
/// straight line would cross.
fn transition_geometry(
    _diagram: &StateDiagram,
    placed: &[PlacedNode],
    _ti: usize,
    tr: &Transition,
    offset: f32,
) -> Option<TransitionGeom> {
    let from = find_node(placed, &tr.from)?;
    let to = find_node(placed, &tr.to)?;

    let mut fc = (from.x + from.w / 2.0, from.y + from.h / 2.0);
    let mut tc = (to.x + to.w / 2.0, to.y + to.h / 2.0);
    let mut side = 0.0;
    if offset != 0.0 {
        // Shift both aim points perpendicular to the connecting line so
        // parallel edges stay separated over their whole length.
        let dx = tc.0 - fc.0;
        let dy = tc.1 - fc.1;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let (px, py) = (-dy / len, dx / len);
        side = px * offset;
        fc = (fc.0 + px * offset, fc.1 + py * offset);
        tc = (tc.0 + px * offset, tc.1 + py * offset);
    }
    let start = border_point(from, tc, fc);
    let end = border_point(to, fc, tc);

    // Detour around the first box the straight segment passes through.
    let contains = |n: &PlacedNode, p: (f32, f32)| {
        p.0 >= n.x && p.0 <= n.x + n.w && p.1 >= n.y && p.1 <= n.y + n.h
    };
    let obstacle = placed.iter().find(|n| {
        n.kind != NodeKind::RegionSep
            && n.name != from.name
            && n.name != to.name
            && !contains(n, (from.x + from.w / 2.0, from.y + from.h / 2.0))
            && !contains(n, (to.x + to.w / 2.0, to.y + to.h / 2.0))
            && graph::segment_intersects_rect(start, end, n.x, n.y, n.w, n.h)
    });

    let mut points = vec![start];
    if let Some(obs) = obstacle {
        let line_x = (start.0 + end.0) / 2.0;
        let obs_cx = obs.x + obs.w / 2.0;
        let detour_x = if line_x <= obs_cx {
            obs.x - 15.0
        } else {
            obs.x + obs.w + 15.0
        };
        let top_y = obs.y - 10.0;
        let bot_y = obs.y + obs.h + 10.0;
        if start.1 <= end.1 {
            points.push((detour_x, top_y));
            points.push((detour_x, bot_y));
        } else {
            points.push((detour_x, bot_y));
            points.push((detour_x, top_y));
        }
    }
    points.push(end);

    // Label anchor: midpoint of the middle segment.
    let seg = points.len() / 2 - 1;
    let label_pos = (
        (points[seg].0 + points[seg + 1].0) / 2.0,
        (points[seg].1 + points[seg + 1].1) / 2.0,
    );
    Some(TransitionGeom {
        points,
        label_pos,
        side,
    })
}

fn draw_transition(
    prims: &mut Vec<Primitive>,
    tr: &Transition,
    geom: &TransitionGeom,
    theme: &dyn Theme,
) {
    let stroke = theme.activity_edge_color().to_string();
    for pair in geom.points.windows(2) {
        prims.push(Primitive::Line(Line {
            x1: pair[0].0,
            y1: pair[0].1,
            x2: pair[1].0,
            y2: pair[1].1,
            stroke: stroke.clone(),
            stroke_width: 1.0,
        }));
    }

    // Concave arrowhead at the target: 9 long, half-width 4, notch at 5.
    let last = geom.points[geom.points.len() - 2];
    let end = geom.points[geom.points.len() - 1];
    let dx = end.0 - last.0;
    let dy = end.1 - last.1;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);
    let (px, py) = (-uy, ux);
    let bx = end.0 - ux * 9.0;
    let by = end.1 - uy * 9.0;
    prims.push(Primitive::Polygon(Polygon {
        points: vec![
            (end.0, end.1),
            (bx + px * 4.0, by + py * 4.0),
            (end.0 - ux * 5.0, end.1 - uy * 5.0),
            (bx - px * 4.0, by - py * 4.0),
        ],
        fill: stroke.clone(),
        stroke,
        stroke_width: 1.0,
    }));

    if let Some(label) = &tr.label {
        // For a left-shifted parallel edge, put the label on its left side so
        // labels of opposite edges don't collide.
        let (anchor, dx_label) = if geom.side < 0.0 {
            (TextAnchor::End, -4.0)
        } else {
            (TextAnchor::Start, 4.0)
        };
        prims.push(Primitive::Text(Text {
            x: geom.label_pos.0 + dx_label,
            y: geom.label_pos.1 + 4.0,
            content: label.clone(),
            font_size: LABEL_FONT,
            font_family: theme.font_family().to_string(),
            fill: theme.activity_text_color().to_string(),
            anchor,
            bold: false,
            italic: false,
            underline: false,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::state::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_simple_state_layout() {
        let d = parse("[*] --> Idle\nIdle --> Running : go\nRunning --> [*]\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid.width > 0.0 && laid.height > 0.0);
        // Start circle + end double-circle = at least 3 circles.
        let circles = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Circle(_)))
            .count();
        assert!(circles >= 3);
        // Two state boxes with rx 12.5.
        let boxes = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(r) if r.rx == CORNER_R))
            .count();
        assert_eq!(boxes, 2);
    }

    #[test]
    fn test_min_box_size() {
        let d = parse("[*] --> A\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .unwrap();
        assert!((rect.width - MIN_SIZE).abs() < 0.01);
        assert!((rect.height - MIN_SIZE).abs() < 0.01);
    }

    #[test]
    fn test_concurrent_region_layout() {
        let d = parse(
            "state Active {\n  [*] --> A1\n  --\n  [*] --> B1\n}\n[*] --> Active\n",
        )
        .unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        // One dashed separator between the two regions.
        let seps: Vec<_> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::DashedLine(dl) => Some(dl),
                _ => None,
            })
            .collect();
        assert_eq!(seps.len(), 1);
        assert_eq!(seps[0].dash_array, REGION_SEP_DASH);
        assert_eq!(seps[0].y1, seps[0].y2);
        // Regions are stacked: A1 fully above the separator, B1 below.
        let rects: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) if r.rx == CORNER_R && r.fill != "none" => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(rects.len(), 2);
        let sep_y = seps[0].y1;
        assert!(rects.iter().any(|r| r.y + r.height < sep_y));
        assert!(rects.iter().any(|r| r.y > sep_y));
    }

    #[test]
    fn test_history_layout() {
        let d = parse(
            "state W {\n  [*] --> E\n}\nW --> S : pause\nS --> W[H] : resume\n[*] --> W\n",
        )
        .unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        // History circle: r=11 with light fill (unlike the end circle r=11
        // which has fill "none").
        let hist = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Circle(c) if c.r == HIST_R && c.fill != "none" => Some(c),
                _ => None,
            })
            .expect("history circle present");
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "H")));
        // The circle sits inside the W composite frame.
        let frame = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) if r.rx == CORNER_R && r.fill == "none" => Some(r),
                _ => None,
            })
            .expect("composite frame");
        assert!(hist.cx > frame.x && hist.cx < frame.x + frame.width);
        assert!(hist.cy > frame.y && hist.cy < frame.y + frame.height);
    }

    #[test]
    fn test_composite_layout() {
        let d = parse(
            "[*] --> NS\nstate NS {\n  [*] --> Idle\n  Idle --> Conf : ev\n}\nNS --> Out\n",
        )
        .unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        // Composite frame + 3 boxes (Idle, Conf, Out).
        let boxes = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(r) if r.rx == CORNER_R))
            .count();
        assert_eq!(boxes, 4);
        // Band path present.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Path(_))));
    }
}
