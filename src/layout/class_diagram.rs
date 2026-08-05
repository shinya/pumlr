/// Layout for class diagrams, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::class_diagram::*;
use crate::layout::graph::{self, GraphEdge, GraphNode};
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const HEADER_H: f32 = 32.0;
// With a stereotype line the header grows (reference: 40.6211).
const HEADER_STEREO_H: f32 = 40.6211;
const STEREO_FONT: f32 = 12.0;
const STEREO_BASELINE: f32 = 16.6016;
const STEREO_NAME_BASELINE: f32 = 32.668;
const STEREO_ICON_CY: f32 = 20.3105;
// Lollipop interface circle (r=8) with the name centered below it.
const LOLLIPOP_R: f32 = 8.0;
const LOLLIPOP_LABEL_DROP: f32 = 30.535;
// Extra margin around a package nested inside another package.
const PKG_NEST_MARGIN: f32 = 8.0;
const ROW_H: f32 = 16.4883;
const FIRST_BASELINE: f32 = 17.5352;
const EMPTY_SECTION_H: f32 = 8.0;
const NAME_FONT: f32 = 14.0;
const MEMBER_FONT: f32 = 14.0;
const LABEL_FONT: f32 = 13.0;
const ICON_R: f32 = 11.0;
const ICON_GAP: f32 = 5.0;
const RANK_GAP: f32 = 60.0;
const LABEL_EXTRA: f32 = 17.0;
const SIBLING_GAP: f32 = 35.0;
const MARGIN: f32 = 7.0;
const EXTRA_MARGIN: f32 = 7.5; // right/bottom margin is ~14.5 in reference
const PKG_TAB_H: f32 = 22.5;
const PKG_PAD: f32 = 16.0;
const PKG_TAB_TO_CHILD: f32 = 12.5;

pub fn layout_with_theme(diagram: &ClassDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(MEMBER_FONT);

    // --- Box geometry for every class.
    let boxes: Vec<BoxGeom> = diagram
        .classes
        .iter()
        .map(|c| box_geometry(c, &measurer))
        .collect();

    let class_index = |name: &str| diagram.classes.iter().position(|c| c.name == name);
    // Package membership: class index -> innermost package index.
    let mut pkg_of: Vec<Option<usize>> = vec![None; diagram.classes.len()];
    for (pi, pkg) in diagram.packages.iter().enumerate() {
        for cname in &pkg.classes {
            if let Some(ci) = class_index(cname) {
                pkg_of[ci] = Some(pi);
            }
        }
    }
    let parents: Vec<Option<usize>> = diagram.packages.iter().map(|p| p.parent).collect();

    // --- Inner layout per scope (a package, or the top level).
    // rel_pos[class] / pkg_rel[pkg] = position relative to the containing
    // scope's content origin.
    let mut rel_pos: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.classes.len()];
    let mut pkg_rel: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.packages.len()];
    let mut pkg_size: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.packages.len()];

    let name_measurer = TextMeasurer::new(NAME_FONT);
    // Children have larger indices than their parents (declaration order), so
    // a reverse sweep sizes inner packages before the packages containing them.
    for pi in (0..diagram.packages.len()).rev() {
        let (w, h) = layout_scope(
            diagram,
            &boxes,
            &pkg_of,
            &parents,
            &pkg_size,
            Some(pi),
            &mut rel_pos,
            &mut pkg_rel,
        );
        pkg_size[pi] = if w == 0.0 && h == 0.0 {
            (80.0, PKG_TAB_H + 30.0)
        } else {
            (
                w + PKG_PAD * 2.0,
                PKG_TAB_H + PKG_TAB_TO_CHILD + h + PKG_PAD,
            )
        };
        // Package width must at least fit the tab + name.
        let min_w = name_measurer.measure_width(&diagram.packages[pi].name) + 30.0;
        if pkg_size[pi].0 < min_w {
            pkg_size[pi].0 = min_w;
        }
    }

    // --- Top-level scope: root packages + unpackaged classes.
    let (top_meta, top_nodes) = layout_scope_nodes(
        diagram,
        &boxes,
        &pkg_of,
        &parents,
        &pkg_size,
        None,
        &mut rel_pos,
        &mut pkg_rel,
    );

    // --- Absolute positions.
    let title_offset = if diagram.title.is_some() { 30.0 } else { 0.0 };
    let origin = (MARGIN, MARGIN + title_offset);
    let mut abs: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.classes.len()];
    let mut pkg_abs: Vec<(f32, f32)> = vec![(0.0, 0.0); diagram.packages.len()];
    for ci in (0..diagram.classes.len()).filter(|&ci| pkg_of[ci].is_none()) {
        abs[ci] = (origin.0 + rel_pos[ci].0, origin.1 + rel_pos[ci].1);
    }
    for pi in (0..diagram.packages.len()).filter(|&pi| parents[pi].is_none()) {
        pkg_abs[pi] = (origin.0 + pkg_rel[pi].0, origin.1 + pkg_rel[pi].1);
    }
    // Parents come before children, so content origins resolve in one pass.
    for pi in 0..diagram.packages.len() {
        let content = (
            pkg_abs[pi].0 + PKG_PAD,
            pkg_abs[pi].1 + PKG_TAB_H + PKG_TAB_TO_CHILD,
        );
        for ci in (0..diagram.classes.len()).filter(|&ci| pkg_of[ci] == Some(pi)) {
            abs[ci] = (content.0 + rel_pos[ci].0, content.1 + rel_pos[ci].1);
        }
        for pj in (0..diagram.packages.len()).filter(|&pj| parents[pj] == Some(pi)) {
            pkg_abs[pj] = (content.0 + pkg_rel[pj].0, content.1 + pkg_rel[pj].1);
        }
    }

    // --- Emit primitives.
    let mut prims: Vec<Primitive> = Vec::new();

    // Packages first (frames behind boxes); parents before children.
    for (pi, pkg) in diagram.packages.iter().enumerate() {
        draw_package(
            &mut prims,
            pkg,
            pkg_abs[pi].0,
            pkg_abs[pi].1,
            pkg_size[pi].0,
            pkg_size[pi].1,
            theme,
            &name_measurer,
        );
    }

    // Edges under boxes (PlantUML draws links after entities, but lines meet
    // borders exactly, so order only matters for markers overlapping fills).
    // Lollipop circles clip against the circle, not the (label-wide) node.
    let clip_rect = |ci: usize| -> RectT {
        if diagram.classes[ci].kind == ClassKind::Circle {
            let cx = abs[ci].0 + boxes[ci].width / 2.0;
            let cy = abs[ci].1 + LOLLIPOP_R;
            let r = LOLLIPOP_R + 1.0;
            (cx - r, cy - r, r * 2.0, r * 2.0)
        } else {
            (abs[ci].0, abs[ci].1, boxes[ci].width, boxes[ci].height)
        }
    };
    for rel in &diagram.relations {
        let (Some(li), Some(ri)) = (class_index(&rel.left), class_index(&rel.right)) else {
            continue;
        };
        draw_relation(&mut prims, rel, clip_rect(li), clip_rect(ri), theme);
    }

    for (ci, class) in diagram.classes.iter().enumerate() {
        draw_class_box(
            &mut prims,
            class,
            &boxes[ci],
            abs[ci].0,
            abs[ci].1,
            theme,
            &measurer,
        );
    }

    // --- Diagram extent.
    let mut width = 0.0f32;
    let mut height = 0.0f32;
    for (meta, node) in top_meta.iter().zip(top_nodes.iter()) {
        let mut node_h = node.height;
        if let ScopeNode::Class(ci) = meta {
            if diagram.classes[*ci].kind == ClassKind::Circle {
                // The label hangs below the circle node.
                node_h = node_h.max(LOLLIPOP_R + LOLLIPOP_LABEL_DROP + 4.5);
            }
        }
        width = width.max(origin.0 + node.x + node.width);
        height = height.max(origin.1 + node.y + node_h);
    }
    width += MARGIN + EXTRA_MARGIN;
    height += MARGIN + EXTRA_MARGIN;

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

struct BoxGeom {
    width: f32,
    height: f32,
    header_h: f32,
    fields_h: f32,
    name_w: f32,
    /// Width of the `«stereotype»` line (0 when absent).
    stereo_w: f32,
}

fn section_height(n: usize) -> f32 {
    if n == 0 {
        EMPTY_SECTION_H
    } else {
        n as f32 * ROW_H + 8.0
    }
}

fn box_geometry(class: &ClassDef, measurer: &TextMeasurer) -> BoxGeom {
    let name_w = measurer.measure_width(&class.display_name);
    // Lollipop circle: the node is the circle; the label hangs below it.
    if class.kind == ClassKind::Circle {
        return BoxGeom {
            width: name_w.max(LOLLIPOP_R * 2.0),
            height: LOLLIPOP_R * 2.0,
            header_h: 0.0,
            fields_h: 0.0,
            name_w,
            stereo_w: 0.0,
        };
    }
    let stereo_w = class
        .stereotype
        .as_deref()
        .map(|s| TextMeasurer::new(STEREO_FONT).measure_width(&format!("\u{ab}{}\u{bb}", s)))
        .unwrap_or(0.0);
    let header_h = if class.stereotype.is_some() {
        HEADER_STEREO_H
    } else {
        HEADER_H
    };
    // Header: 4 + icon(22) + gap + text + 3 (min); centered when wider.
    let header_w = 4.0 + ICON_R * 2.0 + ICON_GAP + name_w.max(stereo_w) + 3.0;
    let member_w = class
        .fields
        .iter()
        .chain(class.methods.iter())
        .map(|m| {
            let indent = if m.visibility.is_some() { 20.0 } else { 6.0 };
            indent + measurer.measure_width(&m.text) + 6.0
        })
        .fold(0.0f32, f32::max);
    let width = header_w.max(member_w).max(54.0);
    let fields_h = section_height(class.fields.len());
    let methods_h = section_height(class.methods.len());
    BoxGeom {
        width,
        height: header_h + fields_h + methods_h,
        header_h,
        fields_h,
        name_w,
        stereo_w,
    }
}

fn stereotype_icon(kind: ClassKind) -> (&'static str, &'static str) {
    match kind {
        ClassKind::Class => ("C", "#ADD1B2"),
        ClassKind::AbstractClass => ("A", "#A9DCDF"),
        ClassKind::Interface => ("I", "#B4A7E5"),
        ClassKind::Enum => ("E", "#EB937F"),
        // Circles are drawn without a letter icon (see draw_class_box).
        ClassKind::Circle => ("", ""),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_class_box(
    prims: &mut Vec<Primitive>,
    class: &ClassDef,
    geom: &BoxGeom,
    x: f32,
    y: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) {
    let fill = class
        .color
        .as_deref()
        .map(resolve_color)
        .unwrap_or_else(|| theme.activity_shape_fill().to_string());

    // Lollipop interface: circle with the name centered below.
    if class.kind == ClassKind::Circle {
        let cx = x + geom.width / 2.0;
        let cy = y + LOLLIPOP_R;
        prims.push(Primitive::Circle(Circle {
            cx,
            cy,
            r: LOLLIPOP_R,
            fill,
            stroke: theme.activity_shape_stroke().to_string(),
            stroke_width: 0.5,
        }));
        prims.push(Primitive::Text(Text {
            x: cx,
            y: cy + LOLLIPOP_LABEL_DROP,
            content: class.display_name.clone(),
            font_size: NAME_FONT,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Middle,
            bold: false,
            italic: false,
            underline: false,
        }));
        return;
    }

    prims.push(Primitive::Rect(Rect {
        x,
        y,
        width: geom.width,
        height: geom.height,
        fill,
        stroke: theme.activity_shape_stroke().to_string(),
        stroke_width: 0.5,
        rx: 2.5,
        ry: 2.5,
    }));

    // Header: icon circle + (stereotype line +) name, centered as a group.
    let has_stereo = class.stereotype.is_some();
    let text_w = geom.name_w.max(geom.stereo_w);
    let group_w = ICON_R * 2.0 + ICON_GAP + text_w;
    let group_x = x + (geom.width - group_w) / 2.0;
    let icon_cy = y + if has_stereo { STEREO_ICON_CY } else { 16.0 };
    let (letter, icon_fill) = stereotype_icon(class.kind);
    prims.push(Primitive::Circle(Circle {
        cx: group_x + ICON_R,
        cy: icon_cy,
        r: ICON_R,
        fill: icon_fill.to_string(),
        stroke: theme.activity_shape_stroke().to_string(),
        stroke_width: 1.0,
    }));
    prims.push(Primitive::Text(Text {
        x: group_x + ICON_R,
        y: icon_cy + 5.0,
        content: letter.to_string(),
        font_size: 14.0,
        font_family: "monospace".to_string(),
        fill: "#000000".into(),
        anchor: TextAnchor::Middle,
        bold: true,
        italic: class.kind == ClassKind::AbstractClass,
        underline: false,
    }));
    let italic_name = matches!(class.kind, ClassKind::AbstractClass | ClassKind::Interface);
    let text_x = group_x + ICON_R * 2.0 + ICON_GAP;
    if let Some(stereo) = &class.stereotype {
        prims.push(Primitive::Text(Text {
            x: text_x + text_w / 2.0,
            y: y + STEREO_BASELINE,
            content: format!("\u{ab}{}\u{bb}", stereo),
            font_size: STEREO_FONT,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Middle,
            bold: false,
            italic: true,
            underline: false,
        }));
    }
    prims.push(Primitive::Text(Text {
        x: if has_stereo {
            text_x + text_w / 2.0
        } else {
            text_x
        },
        y: y + if has_stereo {
            STEREO_NAME_BASELINE
        } else {
            21.291
        },
        content: class.display_name.clone(),
        font_size: NAME_FONT,
        font_family: theme.font_family().to_string(),
        fill: "#000000".into(),
        anchor: if has_stereo {
            TextAnchor::Middle
        } else {
            TextAnchor::Start
        },
        bold: false,
        italic: italic_name,
        underline: false,
    }));

    let sep = |prims: &mut Vec<Primitive>, sy: f32| {
        prims.push(Primitive::Line(Line {
            x1: x + 1.0,
            y1: sy,
            x2: x + geom.width - 1.0,
            y2: sy,
            stroke: theme.activity_shape_stroke().to_string(),
            stroke_width: 0.5,
        }));
    };

    let fields_top = y + geom.header_h;
    sep(prims, fields_top);
    draw_members(prims, &class.fields, x, fields_top, false, theme, measurer);
    let methods_top = fields_top + geom.fields_h;
    sep(prims, methods_top);
    draw_members(prims, &class.methods, x, methods_top, true, theme, measurer);
}

fn visibility_colors(v: Visibility) -> (&'static str, &'static str) {
    // (stroke, method fill)
    match v {
        Visibility::Public => ("#038048", "#84BE84"),
        Visibility::Private => ("#C82930", "#F24D5C"),
        Visibility::Protected => ("#B38D22", "#FFFF44"),
        Visibility::PackagePrivate => ("#036FA7", "#4177AF"),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_members(
    prims: &mut Vec<Primitive>,
    members: &[Member],
    x: f32,
    section_top: f32,
    is_method: bool,
    theme: &dyn Theme,
    _measurer: &TextMeasurer,
) {
    for (i, m) in members.iter().enumerate() {
        let baseline = section_top + FIRST_BASELINE + i as f32 * ROW_H;
        let text_x = if m.visibility.is_some() {
            x + 20.0
        } else {
            x + 6.0
        };
        if let Some(v) = m.visibility {
            let (stroke, method_fill) = visibility_colors(v);
            let fill = if is_method { method_fill } else { "none" };
            let icx = x + 11.0;
            let icy = baseline - 3.791;
            match v {
                Visibility::Public => prims.push(Primitive::Circle(Circle {
                    cx: icx,
                    cy: icy,
                    r: 3.0,
                    fill: fill.to_string(),
                    stroke: stroke.to_string(),
                    stroke_width: 1.0,
                })),
                Visibility::Private => prims.push(Primitive::Rect(Rect {
                    x: icx - 3.0,
                    y: icy - 3.0,
                    width: 6.0,
                    height: 6.0,
                    fill: fill.to_string(),
                    stroke: stroke.to_string(),
                    stroke_width: 1.0,
                    rx: 0.0,
                    ry: 0.0,
                })),
                Visibility::Protected => prims.push(Primitive::Polygon(Polygon {
                    points: vec![
                        (icx, icy - 4.0),
                        (icx + 4.0, icy),
                        (icx, icy + 4.0),
                        (icx - 4.0, icy),
                    ],
                    fill: fill.to_string(),
                    stroke: stroke.to_string(),
                    stroke_width: 1.0,
                })),
                Visibility::PackagePrivate => prims.push(Primitive::Polygon(Polygon {
                    points: vec![
                        (icx, icy - 4.0),
                        (icx + 4.0, icy + 4.0),
                        (icx - 4.0, icy + 4.0),
                    ],
                    fill: fill.to_string(),
                    stroke: stroke.to_string(),
                    stroke_width: 1.0,
                })),
            }
        }
        prims.push(Primitive::Text(Text {
            x: text_x,
            y: baseline,
            content: m.text.clone(),
            font_size: MEMBER_FONT,
            font_family: theme.font_family().to_string(),
            fill: theme.activity_text_color().to_string(),
            anchor: TextAnchor::Start,
            bold: false,
            italic: m.is_abstract,
            underline: m.is_static,
        }));
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_package(
    prims: &mut Vec<Primitive>,
    pkg: &PackageDef,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    theme: &dyn Theme,
    measurer: &TextMeasurer,
) {
    let tab_w = measurer.measure_width(&pkg.name) + 10.0;
    // Folder outline: tab on the top-left with a slanted right edge.
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
    // Tab bottom line.
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
        font_size: NAME_FONT,
        font_family: theme.font_family().to_string(),
        fill: "#000000".into(),
        anchor: TextAnchor::Start,
        bold: true,
        italic: false,
        underline: false,
    }));
}

/// A node directly inside a layout scope (a package's content, or the top
/// level): either a class box or a (nested) package.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScopeNode {
    Class(usize),
    Pkg(usize),
}

/// The node directly inside `scope` that contains class `ci` (the class
/// itself, or the child package whose subtree it lives in).
fn container_in(
    pkg_of: &[Option<usize>],
    parents: &[Option<usize>],
    ci: usize,
    scope: Option<usize>,
) -> Option<ScopeNode> {
    if pkg_of[ci] == scope {
        return Some(ScopeNode::Class(ci));
    }
    let mut p = pkg_of[ci];
    while let Some(pp) = p {
        if parents[pp] == scope {
            return Some(ScopeNode::Pkg(pp));
        }
        p = parents[pp];
    }
    None
}

/// Lay out the direct children of `scope` with the shared graph engine and
/// write their positions (relative to the scope's content origin) into
/// `rel_pos` / `pkg_rel`. Returns the node list for extent computations.
#[allow(clippy::too_many_arguments)]
fn layout_scope_nodes(
    diagram: &ClassDiagram,
    boxes: &[BoxGeom],
    pkg_of: &[Option<usize>],
    parents: &[Option<usize>],
    pkg_size: &[(f32, f32)],
    scope: Option<usize>,
    rel_pos: &mut [(f32, f32)],
    pkg_rel: &mut [(f32, f32)],
) -> (Vec<ScopeNode>, Vec<GraphNode>) {
    let mut meta: Vec<ScopeNode> = Vec::new();
    for (pi, parent) in parents.iter().enumerate() {
        if *parent == scope {
            meta.push(ScopeNode::Pkg(pi));
        }
    }
    for (ci, pkg) in pkg_of.iter().enumerate() {
        if *pkg == scope {
            meta.push(ScopeNode::Class(ci));
        }
    }
    if meta.is_empty() {
        return (meta, Vec::new());
    }

    let class_index = |name: &str| diagram.classes.iter().position(|c| c.name == name);
    let resolve = |name: &str| {
        class_index(name).and_then(|ci| container_in(pkg_of, parents, ci, scope))
    };

    // Same-rank relations (`A - B`) order A to the left of B (dot semantics).
    for rel in &diagram.relations {
        if rel.rank_len > 1 {
            continue;
        }
        let (Some(a), Some(b)) = (resolve(&rel.left), resolve(&rel.right)) else {
            continue;
        };
        if a == b {
            continue;
        }
        let pa = meta.iter().position(|&m| m == a).unwrap();
        let pb = meta.iter().position(|&m| m == b).unwrap();
        if pa > pb {
            let moved = meta.remove(pa);
            meta.insert(pb, moved);
        }
    }

    // Packages nested inside another package get extra margin all around
    // (reference: 24px against the parent frame instead of 16).
    let nest = if scope.is_some() { PKG_NEST_MARGIN } else { 0.0 };
    let mut nodes: Vec<GraphNode> = meta
        .iter()
        .map(|m| match *m {
            ScopeNode::Class(ci) => GraphNode::new(boxes[ci].width, boxes[ci].height),
            ScopeNode::Pkg(pi) => {
                GraphNode::new(pkg_size[pi].0 + nest * 2.0, pkg_size[pi].1 + nest * 2.0)
            }
        })
        .collect();

    let mut edges: Vec<GraphEdge> = Vec::new();
    for rel in &diagram.relations {
        let (Some(a), Some(b)) = (resolve(&rel.left), resolve(&rel.right)) else {
            continue;
        };
        if a == b {
            continue;
        }
        edges.push(GraphEdge {
            from: meta.iter().position(|&m| m == a).unwrap(),
            to: meta.iter().position(|&m| m == b).unwrap(),
            min_len: if rel.rank_len <= 1 { 0 } else { 1 },
            labeled: rel.label.is_some(),
        });
    }

    graph::layout(&mut nodes, &edges, SIBLING_GAP, RANK_GAP, LABEL_EXTRA);

    for (m, node) in meta.iter().zip(nodes.iter()) {
        match *m {
            ScopeNode::Class(ci) => rel_pos[ci] = (node.x, node.y),
            ScopeNode::Pkg(pi) => pkg_rel[pi] = (node.x + nest, node.y + nest),
        }
    }
    (meta, nodes)
}

/// Lay out a scope and return its content extent (width, height).
#[allow(clippy::too_many_arguments)]
fn layout_scope(
    diagram: &ClassDiagram,
    boxes: &[BoxGeom],
    pkg_of: &[Option<usize>],
    parents: &[Option<usize>],
    pkg_size: &[(f32, f32)],
    scope: Option<usize>,
    rel_pos: &mut [(f32, f32)],
    pkg_rel: &mut [(f32, f32)],
) -> (f32, f32) {
    let (_, nodes) = layout_scope_nodes(
        diagram, boxes, pkg_of, parents, pkg_size, scope, rel_pos, pkg_rel,
    );
    let w = nodes.iter().map(|n| n.x + n.width).fold(0.0f32, f32::max);
    let h = nodes.iter().map(|n| n.y + n.height).fold(0.0f32, f32::max);
    (w, h)
}

/// Rect as (x, y, w, h).
type RectT = (f32, f32, f32, f32);

fn draw_relation(
    prims: &mut Vec<Primitive>,
    rel: &Relation,
    left_rect: RectT,
    right_rect: RectT,
    theme: &dyn Theme,
) {
    let (lx, ly, lw, lh) = left_rect;
    let (rx, ry, rw, rh) = right_rect;
    let lc = (lx + lw / 2.0, ly + lh / 2.0);
    let rc = (rx + rw / 2.0, ry + rh / 2.0);

    // End points on the box borders.
    let left_pt = graph::clip_to_rect(rc.0, rc.1, lx, ly, lw, lh);
    let right_pt = graph::clip_to_rect(lc.0, lc.1, rx, ry, rw, rh);

    let dx = right_pt.0 - left_pt.0;
    let dy = right_pt.1 - left_pt.1;
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let (ux, uy) = (dx / len, dy / len);

    let stroke = theme.activity_edge_color().to_string();

    // The line is shortened where a marker occupies the end.
    let left_inset = marker_line_inset(rel.left_marker);
    let right_inset = marker_line_inset(rel.right_marker);
    let line_start = (
        left_pt.0 + ux * left_inset,
        left_pt.1 + uy * left_inset,
    );
    let line_end = (
        right_pt.0 - ux * right_inset,
        right_pt.1 - uy * right_inset,
    );

    if rel.dashed {
        prims.push(Primitive::DashedLine(DashedLine {
            x1: line_start.0,
            y1: line_start.1,
            x2: line_end.0,
            y2: line_end.1,
            stroke: stroke.clone(),
            stroke_width: 1.0,
            dash_array: "7,7".into(),
        }));
    } else {
        prims.push(Primitive::Line(Line {
            x1: line_start.0,
            y1: line_start.1,
            x2: line_end.0,
            y2: line_end.1,
            stroke: stroke.clone(),
            stroke_width: 1.0,
        }));
    }

    // Markers point INTO the entity they are attached to.
    draw_marker(prims, rel.left_marker, left_pt, (-ux, -uy), &stroke);
    draw_marker(prims, rel.right_marker, right_pt, (ux, uy), &stroke);

    // Label at the midpoint.
    if let Some(label) = &rel.label {
        prims.push(Primitive::Text(Text {
            x: (left_pt.0 + right_pt.0) / 2.0 + 5.0,
            y: (left_pt.1 + right_pt.1) / 2.0 + 4.0,
            content: label.clone(),
            font_size: LABEL_FONT,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Start,
            bold: false,
            italic: false,
            underline: false,
        }));
    }
    // Cardinalities near each end.
    if let Some(card) = &rel.left_card {
        prims.push(Primitive::Text(Text {
            x: left_pt.0 + ux * 16.0 + 5.0,
            y: left_pt.1 + uy * 16.0 + 4.0,
            content: card.clone(),
            font_size: LABEL_FONT,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Start,
            bold: false,
            italic: false,
            underline: false,
        }));
    }
    if let Some(card) = &rel.right_card {
        prims.push(Primitive::Text(Text {
            x: right_pt.0 - ux * 16.0 + 5.0,
            y: right_pt.1 - uy * 16.0 + 4.0,
            content: card.clone(),
            font_size: LABEL_FONT,
            font_family: theme.font_family().to_string(),
            fill: "#000000".into(),
            anchor: TextAnchor::Start,
            bold: false,
            italic: false,
            underline: false,
        }));
    }
}

fn marker_line_inset(marker: EndMarker) -> f32 {
    match marker {
        EndMarker::Triangle => 18.0,
        EndMarker::Diamond | EndMarker::FilledDiamond => 12.0,
        EndMarker::ArrowHead | EndMarker::None => 0.0,
    }
}

/// Draw a marker with its tip at `pt`, pointing along `dir` (unit vector
/// pointing INTO the attached entity, i.e. away from the line body).
fn draw_marker(
    prims: &mut Vec<Primitive>,
    marker: EndMarker,
    pt: (f32, f32),
    dir: (f32, f32),
    stroke: &str,
) {
    let (ux, uy) = dir;
    let (px, py) = (-uy, ux); // perpendicular
    match marker {
        EndMarker::None => {}
        EndMarker::Triangle => {
            // Hollow triangle: 18 long, 12 wide at the base.
            let bx = pt.0 - ux * 18.0;
            let by = pt.1 - uy * 18.0;
            prims.push(Primitive::Polygon(Polygon {
                points: vec![
                    (pt.0, pt.1),
                    (bx + px * 6.0, by + py * 6.0),
                    (bx - px * 6.0, by - py * 6.0),
                ],
                fill: "#FFFFFF".into(),
                stroke: stroke.to_string(),
                stroke_width: 1.0,
            }));
        }
        EndMarker::Diamond | EndMarker::FilledDiamond => {
            let fill = if marker == EndMarker::FilledDiamond {
                stroke.to_string()
            } else {
                "#FFFFFF".to_string()
            };
            let mx = pt.0 - ux * 6.0;
            let my = pt.1 - uy * 6.0;
            prims.push(Primitive::Polygon(Polygon {
                points: vec![
                    (pt.0, pt.1),
                    (mx + px * 4.0, my + py * 4.0),
                    (pt.0 - ux * 12.0, pt.1 - uy * 12.0),
                    (mx - px * 4.0, my - py * 4.0),
                ],
                fill,
                stroke: stroke.to_string(),
                stroke_width: 1.0,
            }));
        }
        EndMarker::ArrowHead => {
            // Concave head: 9 long, half-width 4, notch 5 back from the tip.
            let bx = pt.0 - ux * 9.0;
            let by = pt.1 - uy * 9.0;
            prims.push(Primitive::Polygon(Polygon {
                points: vec![
                    (pt.0, pt.1),
                    (bx + px * 4.0, by + py * 4.0),
                    (pt.0 - ux * 5.0, pt.1 - uy * 5.0),
                    (bx - px * 4.0, by - py * 4.0),
                ],
                fill: stroke.to_string(),
                stroke: stroke.to_string(),
                stroke_width: 1.0,
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::class_diagram::parse;
    use crate::theme::DefaultTheme;

    #[test]
    fn test_simple_layout() {
        let d = parse("class Animal {\n  +name: String\n}\nclass Dog\nAnimal <|-- Dog\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid.width > 0.0);
        assert!(laid.height > 100.0);
        // Should contain 2 class boxes (rects with rx 2.5).
        let boxes = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(r) if r.rx == 2.5))
            .count();
        assert_eq!(boxes, 2);
    }

    #[test]
    fn test_empty_class_min_height() {
        let d = parse("class Cat\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .unwrap();
        assert!((rect.height - 48.0).abs() < 0.01);
    }

    #[test]
    fn test_package_frame() {
        let d = parse("package domain {\n  class A\n}\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Path(_))));
    }

    #[test]
    fn test_stereotype_header_height() {
        let d = parse("class Foo <<entity>>\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .unwrap();
        // 40.6211 header + two empty 8px sections.
        assert!((rect.height - 56.6211).abs() < 0.01);
        assert!(laid.primitives.iter().any(
            |p| matches!(p, Primitive::Text(t) if t.content == "\u{ab}entity\u{bb}" && t.italic)
        ));
    }

    #[test]
    fn test_static_member_underlined() {
        let d = parse("class Counter {\n  {static} count: int\n}\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let text = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Text(t) if t.content == "count: int" => Some(t),
                _ => None,
            })
            .unwrap();
        assert!(text.underline);
        assert!(!text.bold);
    }

    #[test]
    fn test_lollipop_circle() {
        let d = parse("class C\n() \"Runnable\" as R\nR - C\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let circle = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Circle(c) if c.r == 8.0 => Some(c),
                _ => None,
            })
            .expect("lollipop circle");
        // Vertically centered against the class box (same rank).
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) if r.rx == 2.5 => Some(r),
                _ => None,
            })
            .unwrap();
        assert!((circle.cy - (rect.y + rect.height / 2.0)).abs() < 0.5);
        // `R - C` puts R to the left of C.
        assert!(circle.cx < rect.x);
        // Label centered below the circle.
        let label = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Text(t) if t.content == "Runnable" => Some(t),
                _ => None,
            })
            .unwrap();
        assert!((label.x - circle.cx).abs() < 0.01);
        assert!(label.y > circle.cy + 8.0);
    }

    #[test]
    fn test_nested_package_frames() {
        let d = parse("package outer {\n  package inner {\n    class A\n  }\n}\n").unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        let paths: Vec<_> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Path(path) => Some(path),
                _ => None,
            })
            .collect();
        assert_eq!(paths.len(), 2, "one folder frame per package");
        // The class box sits inside both frames; the inner frame is inset.
        let rect = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Rect(r) if r.rx == 2.5 => Some(r),
                _ => None,
            })
            .unwrap();
        // outer origin = MARGIN, inner origin = outer + PKG_PAD + nest margin.
        assert!(rect.x > MARGIN + PKG_PAD + PKG_NEST_MARGIN);
    }
}
