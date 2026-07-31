use crate::ast::sequence::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{DefaultTheme, Theme};

// Layout constants (matched against PlantUML 1.2026 default output, see SPEC.md)
const PADDING: f32 = 17.0;
const PARTICIPANT_PADDING_H: f32 = 7.0;
const PARTICIPANT_PADDING_V: f32 = 7.0;
const PARTICIPANT_MARGIN: f32 = 30.0;
/// Vertical distance between consecutive message lines.
const MESSAGE_SPACING: f32 = 30.0;
const NOTE_PADDING: f32 = 5.0;
const NOTE_MARGIN: f32 = 5.0;
const NOTE_FOLD: f32 = 10.0;
/// Gap between the previous row and the top of a note.
const NOTE_TOP_GAP: f32 = 13.0;
const GROUP_PADDING: f32 = 8.0;
const GROUP_LABEL_HEIGHT: f32 = 17.5;
const GROUP_TOP_MARGIN: f32 = 15.0;
const ACTIVATION_HALF_W: f32 = 5.0;
// Database cylinder (participant head/tail icon)
const DB_CYL_W: f32 = 36.0;
const DB_CYL_H: f32 = 46.0;
const DB_CYL_CAP: f32 = 10.0;
const SELF_MSG_WIDTH: f32 = 42.0;
const SELF_MSG_HEIGHT: f32 = 13.0;
// Actor stick figure (participant head/tail icon)
const ACTOR_HEAD_R: f32 = 8.0;
const ACTOR_ICON_H: f32 = 58.0;
const ACTOR_ARM_HALF: f32 = 13.0;
// Circle-based icons (boundary / control / entity)
const CIRCLE_ICON_R: f32 = 12.0;
const CIRCLE_ICON_BLOCK_H: f32 = 29.0;
// Queue horizontal cylinder
const QUEUE_H: f32 = 26.5;
const QUEUE_CAP: f32 = 5.0;
// Collections stacked boxes offset
const COLLECTIONS_OFFSET: f32 = 4.0;
/// Vertical space reserved for `box` titles above the participant heads.
const BOX_TITLE_BAND: f32 = 24.0;
const DIAGRAM_MARGIN: f32 = 10.0;
const TITLE_MARGIN: f32 = 10.0;

/// Layout a parsed sequence diagram into primitives with the default theme.
pub fn layout(diagram: &SequenceDiagram) -> LaidOutDiagram {
    layout_with_theme(diagram, &DefaultTheme)
}

/// Layout a parsed sequence diagram into primitives with the given theme.
pub fn layout_with_theme(diagram: &SequenceDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(theme.font_size());
    // Used for both the title and participant labels (both 14px).
    let title_measurer = TextMeasurer::new(theme.participant_font_size());

    let mut ctx = LayoutContext::new(theme, &measurer, &title_measurer);
    ctx.layout(diagram)
}

struct ParticipantInfo {
    name: String,
    label: String,
    #[allow(dead_code)]
    kind: ParticipantKind,
    /// Fill override from `participant Foo #Color`.
    color: Option<String>,
    /// Declared with `create`: drawn at its first received message.
    created: bool,
    /// Where a created participant's head box ended (lifeline start).
    materialized_y: Option<f32>,
    x_center: f32,
    box_width: f32,
    box_height: f32,
}

struct LayoutContext<'a> {
    theme: &'a dyn Theme,
    measurer: &'a TextMeasurer,
    title_measurer: &'a TextMeasurer,
    participants: Vec<ParticipantInfo>,
    /// Bottom layer: `box` background panels (behind everything)
    box_primitives: Vec<Primitive>,
    /// Background layer: lifelines (drawn behind everything)
    bg_primitives: Vec<Primitive>,
    /// Middle layer: group frames (behind messages but above lifelines)
    group_primitives: Vec<Primitive>,
    /// Foreground layer: participant boxes, messages, notes, text
    fg_primitives: Vec<Primitive>,
    y_cursor: f32,
    auto_number: Option<u32>,
    auto_number_step: u32,
    auto_number_format: Option<String>,
    /// The (from, to) of the most recent message, for `activate`/`return`.
    last_message: Option<(String, String)>,
    /// Rightmost extent of content that sticks out past the participants
    /// (self-message loops, notes), used to widen the diagram.
    max_right: f32,
    /// Currently open activations: (participant name, start y, activator).
    active_participants: Vec<(String, f32, Option<String>)>,
    /// Finished activations: (participant name, start y, end y).
    finished_activations: Vec<(String, f32, f32)>,
}

impl<'a> LayoutContext<'a> {
    fn new(
        theme: &'a dyn Theme,
        measurer: &'a TextMeasurer,
        title_measurer: &'a TextMeasurer,
    ) -> Self {
        Self {
            theme,
            measurer,
            title_measurer,
            participants: Vec::new(),
            box_primitives: Vec::new(),
            bg_primitives: Vec::new(),
            group_primitives: Vec::new(),
            fg_primitives: Vec::new(),
            y_cursor: DIAGRAM_MARGIN,
            auto_number: None,
            auto_number_step: 1,
            auto_number_format: None,
            last_message: None,
            max_right: 0.0,
            active_participants: Vec::new(),
            finished_activations: Vec::new(),
        }
    }

    fn layout(&mut self, diagram: &SequenceDiagram) -> LaidOutDiagram {
        // Collect all participants (declared + implicit from messages)
        self.collect_participants(diagram);

        // Position participants horizontally, spacing them out enough for
        // message labels between each pair. Group frames extend 10px past the
        // outer participant boxes, so reserve room for them on the left.
        let has_group = diagram
            .elements
            .iter()
            .any(|e| matches!(e, SequenceElement::Group(_)));
        let constraints = self.gather_spacing_constraints(&diagram.elements);
        let base_extra = if has_group { 10.0 } else { 0.0 };
        self.position_participants(&constraints, base_extra);

        // Notes hanging left of the first participant would be clipped;
        // re-position everything with enough extra left margin for them.
        let note_min = self.min_note_x(&diagram.elements);
        if note_min < DIAGRAM_MARGIN {
            self.position_participants(&constraints, base_extra + DIAGRAM_MARGIN - note_min);
        }

        // Header: small gray text at the top right
        if let Some(header) = &diagram.header {
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: self.calculate_total_width() - 4.0,
                y: self.y_cursor + 5.0,
                content: header.clone(),
                font_size: 10.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#888888".into(),
                anchor: TextAnchor::End,
            }));
            self.y_cursor += 14.0;
        }

        // Draw title if present
        if let Some(title) = &diagram.title {
            self.draw_title(title);
        }

        // Boxes reserve a title band above the participant heads
        if !diagram.boxes.is_empty() {
            self.y_cursor += BOX_TITLE_BAND;
        }

        // Draw participant heads (top row); lifelines start at its bottom edge
        let participant_top_y = self.y_cursor;
        self.draw_participant_boxes(participant_top_y, false);
        self.y_cursor += self.head_row_height();

        let lifeline_start_y = self.y_cursor;

        // Layout elements
        self.layout_elements(&diagram.elements);

        self.y_cursor += PADDING;

        // Draw participant tails (bottom row) unless `hide footbox`
        let bottom_box_y = self.y_cursor;
        if diagram.hide_footbox {
            self.y_cursor += DIAGRAM_MARGIN;
        } else {
            self.draw_participant_boxes(bottom_box_y, true);
            self.y_cursor += self.head_row_height() + DIAGRAM_MARGIN;
        }

        // Draw lifelines (from bottom of top box to top of bottom box)
        self.draw_lifelines(lifeline_start_y, bottom_box_y);

        // Close any activations left open, then draw all activation bars
        // (above the lifelines, below messages).
        let open: Vec<(String, f32, Option<String>)> =
            std::mem::take(&mut self.active_participants);
        for (name, start_y, _) in open {
            self.finished_activations
                .push((name, start_y, bottom_box_y));
        }
        let bars: Vec<(f32, f32, f32)> = self
            .finished_activations
            .iter()
            .map(|(name, start, end)| (self.participant_x(name), *start, *end))
            .collect();
        for (x, start, end) in bars {
            self.group_primitives.push(Primitive::Rect(Rect {
                x: x - ACTIVATION_HALF_W,
                y: start,
                width: ACTIVATION_HALF_W * 2.0,
                height: end - start,
                fill: self.theme.activation_bg_color().to_string(),
                stroke: self.theme.activation_border_color().to_string(),
                stroke_width: 1.0,
                rx: 0.0,
                ry: 0.0,
            }));
        }

        // Caption (centered) and footer (small gray, centered) at the bottom
        if let Some(caption) = &diagram.caption {
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: self.calculate_total_width() / 2.0,
                y: self.y_cursor + 6.0,
                content: caption.clone(),
                font_size: self.theme.participant_font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
            self.y_cursor += 20.0;
        }
        if let Some(footer) = &diagram.footer {
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: self.calculate_total_width() / 2.0,
                y: self.y_cursor + 4.0,
                content: footer.clone(),
                font_size: 10.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#888888".into(),
                anchor: TextAnchor::Middle,
            }));
            self.y_cursor += 14.0;
        }

        // Box panels: full-height backgrounds behind their participants
        let box_bottom = if diagram.hide_footbox {
            bottom_box_y + 4.0
        } else {
            bottom_box_y + self.head_row_height() + 4.0
        };
        for pbox in &diagram.boxes {
            let members: Vec<&ParticipantInfo> = self
                .participants
                .iter()
                .filter(|p| pbox.participants.contains(&p.name))
                .collect();
            if members.is_empty() {
                continue;
            }
            let left = members
                .iter()
                .map(|p| p.x_center - p.box_width / 2.0)
                .fold(f32::MAX, f32::min)
                - 4.0;
            let right = members
                .iter()
                .map(|p| p.x_center + p.box_width / 2.0)
                .fold(f32::MIN, f32::max)
                + 4.0;
            let top = participant_top_y - BOX_TITLE_BAND;
            let fill = pbox
                .color
                .as_deref()
                .map(crate::theme::resolve_color)
                .unwrap_or_else(|| "#DDDDDD".to_string());
            self.box_primitives.push(Primitive::Rect(Rect {
                x: left,
                y: top,
                width: right - left,
                height: box_bottom - top,
                fill,
                stroke: "#181818".into(),
                stroke_width: 0.5,
                rx: 0.0,
                ry: 0.0,
            }));
            if !pbox.title.is_empty() {
                self.box_primitives.push(Primitive::Text(Text {
                    bold: true,
                    italic: false,
                    x: (left + right) / 2.0,
                    y: top + 13.0,
                    content: pbox.title.clone(),
                    font_size: self.theme.font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            }
            self.max_right = self.max_right.max(right + 1.0);
        }

        // Calculate total dimensions (content like self-message loops and
        // notes can stick out past the last participant)
        let total_width = self
            .calculate_total_width()
            .max(self.max_right + DIAGRAM_MARGIN);
        let total_height = self.y_cursor;

        // Merge layers: background (lifelines) → groups → foreground (boxes, messages, notes)
        let mut primitives = Vec::new();
        primitives.append(&mut self.box_primitives);
        primitives.append(&mut self.bg_primitives);
        primitives.append(&mut self.group_primitives);
        primitives.append(&mut self.fg_primitives);

        LaidOutDiagram {
            width: total_width,
            height: total_height,
            primitives,
        }
    }

    fn collect_participants(&mut self, diagram: &SequenceDiagram) {
        // First: explicit declarations
        for element in &diagram.elements {
            if let SequenceElement::ParticipantDecl(p) = element {
                if !self.participants.iter().any(|pi| pi.name == p.name) {
                    let label = p.label.clone().unwrap_or_else(|| p.name.clone());
                    let label_w = self.title_measurer.measure_width(&label);
                    let box_width = match p.kind {
                        ParticipantKind::Database => label_w.max(DB_CYL_W),
                        ParticipantKind::Actor => label_w.max(ACTOR_ARM_HALF * 2.0),
                        ParticipantKind::Boundary => label_w.max(53.0),
                        ParticipantKind::Control | ParticipantKind::Entity => {
                            label_w.max(CIRCLE_ICON_R * 2.0)
                        }
                        ParticipantKind::Queue => label_w + 22.0,
                        ParticipantKind::Collections => {
                            label_w + PARTICIPANT_PADDING_H * 2.0 + COLLECTIONS_OFFSET
                        }
                        _ => label_w + PARTICIPANT_PADDING_H * 2.0,
                    };
                    self.participants.push(ParticipantInfo {
                        name: p.name.clone(),
                        label,
                        kind: p.kind,
                        color: p.color.clone(),
                        created: p.created,
                        materialized_y: None,
                        x_center: 0.0,
                        box_width,
                        box_height: 0.0,
                    });
                }
            }
        }

        // Then: implicit participants from messages
        self.collect_implicit_participants(&diagram.elements);
    }

    fn collect_implicit_participants(&mut self, elements: &[SequenceElement]) {
        for element in elements {
            match element {
                SequenceElement::Message(msg) => {
                    for name in [&msg.from, &msg.to] {
                        if !self.participants.iter().any(|pi| pi.name == *name) {
                            let box_width = self.title_measurer.measure_width(name)
                                + PARTICIPANT_PADDING_H * 2.0;
                            self.participants.push(ParticipantInfo {
                                name: name.clone(),
                                label: name.clone(),
                                kind: ParticipantKind::Participant,
                                color: None,
                                created: false,
                                materialized_y: None,
                                x_center: 0.0,
                                box_width,
                                box_height: 0.0,
                            });
                        }
                    }
                }
                SequenceElement::Group(group) => {
                    self.collect_implicit_participants(&group.elements);
                    for else_block in &group.else_blocks {
                        self.collect_implicit_participants(&else_block.elements);
                    }
                }
                _ => {}
            }
        }
    }

    /// Minimum center-to-center distances required between participant pairs
    /// so that message labels fit: (left index, right index, min distance).
    fn gather_spacing_constraints(&self, elements: &[SequenceElement]) -> Vec<(usize, usize, f32)> {
        let mut constraints = Vec::new();
        self.gather_spacing_constraints_into(elements, &mut constraints);
        constraints
    }

    fn gather_spacing_constraints_into(
        &self,
        elements: &[SequenceElement],
        constraints: &mut Vec<(usize, usize, f32)>,
    ) {
        let index_of = |name: &str| self.participants.iter().position(|p| p.name == name);
        for element in elements {
            match element {
                SequenceElement::Message(msg) if !msg.is_self_referencing => {
                    if let (Some(i), Some(j)) = (index_of(&msg.from), index_of(&msg.to)) {
                        let (a, b) = if i < j { (i, j) } else { (j, i) };
                        let dist = self.measurer.measure_width(&msg.label) + 25.0;
                        constraints.push((a, b, dist));
                    }
                }
                SequenceElement::Group(group) => {
                    self.gather_spacing_constraints_into(&group.elements, constraints);
                    for else_block in &group.else_blocks {
                        self.gather_spacing_constraints_into(&else_block.elements, constraints);
                    }
                }
                SequenceElement::RefOver(r) => {
                    let mut idxs: Vec<usize> =
                        r.participants.iter().filter_map(|n| index_of(n)).collect();
                    idxs.sort_unstable();
                    if idxs.len() >= 2 {
                        let w = r
                            .text
                            .lines()
                            .map(|l| self.measurer.measure_width(l))
                            .fold(0.0f32, f32::max);
                        constraints.push((idxs[0], idxs[idxs.len() - 1], w + 30.0));
                    }
                }
                _ => {}
            }
        }
    }

    fn position_participants(&mut self, constraints: &[(usize, usize, f32)], left_extra: f32) {
        let box_height = self.participant_box_height();
        let n = self.participants.len();
        let mut centers = vec![0.0f32; n];

        for i in 0..n {
            let w = self.participants[i].box_width;
            let mut x = if i == 0 {
                DIAGRAM_MARGIN + left_extra + w / 2.0
            } else {
                let prev_w = self.participants[i - 1].box_width;
                centers[i - 1] + prev_w / 2.0 + PARTICIPANT_MARGIN + w / 2.0
            };
            for &(a, b, dist) in constraints {
                if b == i {
                    x = x.max(centers[a] + dist);
                }
            }
            centers[i] = x;
        }

        for (p, &x) in self.participants.iter_mut().zip(&centers) {
            p.box_height = box_height;
            p.x_center = x;
        }
    }

    fn participant_box_height(&self) -> f32 {
        self.title_measurer.line_height() + PARTICIPANT_PADDING_V * 2.0
    }

    /// Height of the participant head/tail row. Database cylinders and actor
    /// figures are taller than plain boxes; everything is aligned within this
    /// row height.
    fn head_row_height(&self) -> f32 {
        let label_lh = self.title_measurer.line_height();
        self.participants
            .iter()
            .map(|p| match p.kind {
                ParticipantKind::Database => DB_CYL_H + label_lh,
                ParticipantKind::Actor => ACTOR_ICON_H + label_lh + 2.0,
                ParticipantKind::Boundary | ParticipantKind::Control | ParticipantKind::Entity => {
                    CIRCLE_ICON_BLOCK_H + label_lh + 2.0
                }
                ParticipantKind::Queue => QUEUE_H + 4.0,
                ParticipantKind::Collections => self.participant_box_height() + COLLECTIONS_OFFSET,
                _ => self.participant_box_height(),
            })
            .fold(self.participant_box_height(), f32::max)
    }

    fn participant_x(&self, name: &str) -> f32 {
        self.participants
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.x_center)
            .unwrap_or(0.0)
    }

    fn draw_title(&mut self, title: &str) {
        let total_width = self.calculate_total_width();
        let center_x = total_width / 2.0;
        self.fg_primitives.push(Primitive::Text(Text {
            bold: true,
            italic: false,
            x: center_x,
            y: self.y_cursor + self.title_measurer.line_height(),
            content: title.to_string(),
            font_size: self.theme.participant_font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Middle,
        }));
        self.y_cursor += self.title_measurer.line_height() + TITLE_MARGIN;
    }

    /// Draw the participant head (top) or tail (bottom) row starting at `y`.
    /// Heads are bottom-aligned within the row, tails top-aligned, so boxes
    /// and taller database cylinders sit flush against the lifeline.
    fn draw_participant_boxes(&mut self, y: f32, bottom: bool) {
        let row_h = self.head_row_height();
        let box_h = self.participant_box_height();
        let label_lh = self.title_measurer.line_height();

        let mut prims = Vec::new();
        for p in &self.participants {
            // Created participants appear at their first message instead
            if !bottom && p.created {
                continue;
            }
            let fill = p
                .color
                .as_deref()
                .map(crate::theme::resolve_color)
                .unwrap_or_else(|| self.theme.participant_bg_color().to_string());
            if p.kind == ParticipantKind::Database {
                let (cyl_top, label_baseline) = if bottom {
                    (y + label_lh, y + label_lh - 3.0)
                } else {
                    (y + row_h - DB_CYL_H - label_lh, y + row_h - 3.0)
                };
                prims.push(database_cylinder(
                    p.x_center,
                    cyl_top,
                    &fill,
                    self.theme.participant_border_color(),
                ));
                prims.push(database_cylinder_lens(
                    p.x_center,
                    cyl_top,
                    self.theme.participant_border_color(),
                ));
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: p.x_center,
                    y: label_baseline,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            } else if matches!(
                p.kind,
                ParticipantKind::Boundary | ParticipantKind::Control | ParticipantKind::Entity
            ) {
                // Circle-based icons with the label below (head) / above (tail)
                let (icon_top, label_baseline) = if bottom {
                    (y + label_lh + 3.0, y + label_lh - 3.0)
                } else {
                    (
                        y + row_h - CIRCLE_ICON_BLOCK_H - label_lh - 2.0,
                        y + row_h - 3.0,
                    )
                };
                let cx = p.x_center;
                let cy = if p.kind == ParticipantKind::Control {
                    icon_top + 5.0 + CIRCLE_ICON_R
                } else {
                    icon_top + CIRCLE_ICON_R
                };
                prims.push(Primitive::Path(Path {
                    d: circle_path_at(cx, cy, CIRCLE_ICON_R),
                    fill: fill.clone(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    dashed: false,
                }));
                match p.kind {
                    ParticipantKind::Boundary => {
                        // Vertical bar left of the circle, linked at the middle
                        prims.push(Primitive::Path(Path {
                            d: format!(
                                "M {},{} L {},{} M {},{} L {},{}",
                                cx - 29.0,
                                cy - CIRCLE_ICON_R,
                                cx - 29.0,
                                cy + CIRCLE_ICON_R,
                                cx - 29.0,
                                cy,
                                cx - CIRCLE_ICON_R,
                                cy,
                            ),
                            fill: "none".into(),
                            stroke: self.theme.participant_border_color().to_string(),
                            stroke_width: 0.5,
                            dashed: false,
                        }));
                    }
                    ParticipantKind::Control => {
                        // Small arrowhead on top of the circle
                        let t = cy - CIRCLE_ICON_R;
                        prims.push(Primitive::Polygon(Polygon {
                            points: vec![
                                (cx - 4.0, t),
                                (cx + 2.0, t - 5.0),
                                (cx, t),
                                (cx + 2.0, t + 5.0),
                            ],
                            fill: self.theme.participant_border_color().to_string(),
                            stroke: "none".into(),
                            stroke_width: 0.0,
                        }));
                    }
                    ParticipantKind::Entity => {
                        // Underline below the circle
                        prims.push(Primitive::Line(Line {
                            x1: cx - CIRCLE_ICON_R,
                            y1: cy + CIRCLE_ICON_R + 2.0,
                            x2: cx + CIRCLE_ICON_R,
                            y2: cy + CIRCLE_ICON_R + 2.0,
                            stroke: self.theme.participant_border_color().to_string(),
                            stroke_width: 0.5,
                        }));
                    }
                    _ => {}
                }
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: cx,
                    y: label_baseline,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            } else if p.kind == ParticipantKind::Queue {
                // Horizontal cylinder with the label inside
                let top = if bottom {
                    y + 2.0
                } else {
                    y + row_h - QUEUE_H - 2.0
                };
                let bot = top + QUEUE_H;
                let cy = top + QUEUE_H / 2.0;
                let l = p.x_center - p.box_width / 2.0 + QUEUE_CAP;
                let r = p.x_center + p.box_width / 2.0 - QUEUE_CAP;
                prims.push(Primitive::Path(Path {
                    d: format!(
                        "M {l},{top} L {r},{top} C {rc},{top} {rc},{cy} {rc},{cy} C {rc},{cy} {rc},{bot} {r},{bot} L {l},{bot} C {lc},{bot} {lc},{cy} {lc},{cy} C {lc},{cy} {lc},{top} {l},{top}",
                        l = l,
                        r = r,
                        top = top,
                        bot = bot,
                        cy = cy,
                        rc = r + QUEUE_CAP,
                        lc = l - QUEUE_CAP,
                    ),
                    fill: fill.clone(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    dashed: false,
                }));
                // Inner lens on the right end
                prims.push(Primitive::Path(Path {
                    d: format!(
                        "M {r},{top} C {ri},{top} {ri},{cy} {ri},{cy} C {ri},{bot} {r},{bot} {r},{bot}",
                        r = r,
                        ri = r - QUEUE_CAP,
                        top = top,
                        bot = bot,
                        cy = cy,
                    ),
                    fill: "none".into(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    dashed: false,
                }));
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: p.x_center - QUEUE_CAP,
                    y: cy + 5.0,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            } else if p.kind == ParticipantKind::Collections {
                // Two stacked boxes; label inside the front (lower-left) one
                let front_w = p.box_width - COLLECTIONS_OFFSET;
                let (front_y, back_y) = if bottom {
                    (y + COLLECTIONS_OFFSET, y)
                } else {
                    (y + row_h - box_h, y + row_h - box_h - COLLECTIONS_OFFSET)
                };
                let front_x = p.x_center - p.box_width / 2.0;
                for (bx, by) in [(front_x + COLLECTIONS_OFFSET, back_y), (front_x, front_y)] {
                    prims.push(Primitive::Rect(Rect {
                        x: bx,
                        y: by,
                        width: front_w,
                        height: box_h,
                        fill: fill.clone(),
                        stroke: self.theme.participant_border_color().to_string(),
                        stroke_width: 0.5,
                        rx: 0.0,
                        ry: 0.0,
                    }));
                }
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: front_x + front_w / 2.0,
                    y: front_y + box_h / 2.0 + self.title_measurer.line_height() * 0.32,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            } else if p.kind == ParticipantKind::Actor {
                // Stick figure with the label below (head) / above (tail)
                let (icon_top, label_baseline) = if bottom {
                    (y + label_lh + 3.0, y + label_lh - 3.0)
                } else {
                    (y + row_h - ACTOR_ICON_H - label_lh - 2.0, y + row_h - 3.0)
                };
                prims.push(Primitive::Path(Path {
                    d: circle_path_at(p.x_center, icon_top + ACTOR_HEAD_R, ACTOR_HEAD_R),
                    fill: fill.clone(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    dashed: false,
                }));
                let cx = p.x_center;
                let neck = icon_top + ACTOR_HEAD_R * 2.0;
                let hip = neck + 27.0;
                let feet = icon_top + ACTOR_ICON_H;
                let arms = neck + 8.0;
                prims.push(Primitive::Path(Path {
                    d: format!(
                        "M {},{} L {},{} M {},{} L {},{} M {},{} L {},{} M {},{} L {},{}",
                        cx,
                        neck,
                        cx,
                        hip,
                        cx - ACTOR_ARM_HALF,
                        arms,
                        cx + ACTOR_ARM_HALF,
                        arms,
                        cx,
                        hip,
                        cx - ACTOR_ARM_HALF,
                        feet,
                        cx,
                        hip,
                        cx + ACTOR_ARM_HALF,
                        feet,
                    ),
                    fill: "none".into(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    dashed: false,
                }));
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: cx,
                    y: label_baseline,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            } else {
                let box_y = if bottom { y } else { y + row_h - box_h };
                prims.push(Primitive::Rect(Rect {
                    x: p.x_center - p.box_width / 2.0,
                    y: box_y,
                    width: p.box_width,
                    height: box_h,
                    fill: fill.clone(),
                    stroke: self.theme.participant_border_color().to_string(),
                    stroke_width: 0.5,
                    rx: 2.5,
                    ry: 2.5,
                }));
                prims.push(Primitive::Text(Text {
                    bold: false,
                    italic: false,
                    x: p.x_center,
                    y: box_y + box_h / 2.0 + self.title_measurer.line_height() * 0.32,
                    content: p.label.clone(),
                    font_size: self.theme.participant_font_size(),
                    font_family: self.theme.font_family().to_string(),
                    fill: "black".into(),
                    anchor: TextAnchor::Middle,
                }));
            }
        }
        self.fg_primitives.append(&mut prims);
    }

    fn draw_lifelines(&mut self, start_y: f32, end_y: f32) {
        for p in &self.participants {
            self.bg_primitives.push(Primitive::DashedLine(DashedLine {
                x1: p.x_center,
                y1: p.materialized_y.unwrap_or(start_y),
                x2: p.x_center,
                y2: end_y,
                stroke: self.theme.lifeline_color().to_string(),
                stroke_width: 0.5,
                dash_array: "5,5".into(),
            }));
        }
    }

    fn layout_elements(&mut self, elements: &[SequenceElement]) {
        let mut i = 0;
        while i < elements.len() {
            let element = &elements[i];
            match element {
                SequenceElement::ParticipantDecl(_) => {
                    // Already handled
                }
                SequenceElement::Message(msg) => {
                    // Activations declared right after a message (or with the
                    // `++` shorthand) start at that message's line.
                    let mut pending = Vec::new();
                    if msg.activate_target {
                        pending.push(msg.to.clone());
                    }
                    while let Some(SequenceElement::Activate(name)) = elements.get(i + 1) {
                        pending.push(name.clone());
                        i += 1;
                    }
                    self.layout_message(msg, &pending);
                }
                SequenceElement::Note(note) => self.layout_note(note),
                SequenceElement::Group(group) => self.layout_group(group),
                SequenceElement::Separator(sep) => self.layout_separator(sep),
                SequenceElement::Activate(name) => {
                    // The bar starts at the message line that activated it,
                    // which is where the cursor sits right after a message.
                    let activator = self
                        .last_message
                        .as_ref()
                        .filter(|(_, to)| to == name)
                        .map(|(from, _)| from.clone());
                    self.active_participants
                        .push((name.clone(), self.y_cursor, activator));
                }
                SequenceElement::Deactivate(name) => {
                    if let Some(pos) = self
                        .active_participants
                        .iter()
                        .rposition(|(n, _, _)| n == name)
                    {
                        let (n, start_y, _) = self.active_participants.remove(pos);
                        self.finished_activations.push((n, start_y, self.y_cursor));
                    }
                }
                SequenceElement::AutoNumber(config) => {
                    self.auto_number = Some(config.start.unwrap_or(1));
                    self.auto_number_step = config.increment.unwrap_or(1);
                    self.auto_number_format = config.format.clone();
                }
                SequenceElement::Return(label) => self.layout_return(label),
                SequenceElement::RefOver(r) => self.layout_ref_over(r),
                SequenceElement::Delay(label) => self.layout_delay(label),
                SequenceElement::Space(n) => {
                    self.y_cursor += n.unwrap_or(15) as f32;
                }
            }
            i += 1;
        }
    }

    /// Format an autonumber value: the run of `0`s in the format string is
    /// replaced by the zero-padded number, other characters are literal.
    fn format_auto_number(&self, num: u32) -> String {
        let Some(fmt) = &self.auto_number_format else {
            return num.to_string();
        };
        let zeros = fmt.chars().filter(|c| *c == '0').count().max(1);
        let padded = format!("{:0width$}", num, width = zeros);
        let mut out = String::new();
        let mut replaced = false;
        for c in fmt.chars() {
            if c == '0' {
                if !replaced {
                    out.push_str(&padded);
                    replaced = true;
                }
            } else {
                out.push(c);
            }
        }
        if !replaced {
            out = format!("{} {}", padded, out);
        }
        out
    }

    /// Whether `name` currently has an open activation bar.
    fn is_active(&self, name: &str) -> bool {
        self.active_participants.iter().any(|(n, _, _)| n == name)
    }

    fn layout_message(&mut self, msg: &Message, pending_activations: &[String]) {
        // The message line sits MESSAGE_SPACING below the previous row;
        // the label is drawn just above the line.
        let y = self.y_cursor + MESSAGE_SPACING;

        // Bars activated by this message start at its line, and the arrow
        // already stops at the new bar's edge.
        for name in pending_activations {
            self.active_participants
                .push((name.clone(), y, Some(msg.from.clone())));
        }

        let mut label = msg.label.clone();
        if let Some(num) = self.auto_number {
            label = format!("{} {}", self.format_auto_number(num), label);
            self.auto_number = Some(num + self.auto_number_step);
        }

        self.last_message = Some((msg.from.clone(), msg.to.clone()));
        if msg.is_self_referencing {
            self.layout_self_message(msg, &label, y);
            self.y_cursor = y + SELF_MSG_HEIGHT;
        } else {
            self.layout_normal_message(msg, &label, y);
            self.y_cursor = y;
        }

        // `--` shorthand: the source's activation ends at this message
        if msg.deactivate_source {
            if let Some(pos) = self
                .active_participants
                .iter()
                .rposition(|(n, _, _)| *n == msg.from)
            {
                let (n, start_y, _) = self.active_participants.remove(pos);
                self.finished_activations.push((n, start_y, y));
            }
        }
    }

    /// `return <label>`: reply from the most recently activated participant to
    /// its activator, closing that activation at this line.
    fn layout_return(&mut self, label: &str) {
        let Some((callee, start_y, activator)) = self.active_participants.pop() else {
            return;
        };
        let y = self.y_cursor + MESSAGE_SPACING;

        let mut label = label.to_string();
        if let Some(num) = self.auto_number {
            label = format!("{} {}", self.format_auto_number(num), label);
            self.auto_number = Some(num + self.auto_number_step);
        }

        if let Some(activator) = activator {
            let msg = Message {
                from: callee.clone(),
                to: activator,
                label: label.clone(),
                arrow: ArrowStyle {
                    line: LineStyle::Dashed,
                    head: ArrowHead::Filled,
                },
                is_self_referencing: false,
                activate_target: false,
                deactivate_source: false,
                color: None,
            };
            // The callee is still active while drawing, so the line leaves
            // from its bar edge.
            self.active_participants
                .push((callee.clone(), start_y, None));
            self.layout_normal_message(&msg, &label, y);
            self.active_participants.pop();
        }

        self.finished_activations.push((callee, start_y, y));
        self.y_cursor = y;
    }

    fn layout_normal_message(&mut self, msg: &Message, label: &str, y: f32) {
        let from_x = self.participant_x(&msg.from);
        let to_x = self.participant_x(&msg.to);

        // A `create`d participant materializes at its first received message:
        // its head box is drawn here and the arrow stops at the box edge.
        let create_idx = self
            .participants
            .iter()
            .position(|p| p.name == msg.to && p.created && p.materialized_y.is_none());
        if let Some(idx) = create_idx {
            let (cx, w, h, fill, label_text) = {
                let p = &self.participants[idx];
                let fill = p
                    .color
                    .as_deref()
                    .map(crate::theme::resolve_color)
                    .unwrap_or_else(|| self.theme.participant_bg_color().to_string());
                (
                    p.x_center,
                    p.box_width,
                    self.participant_box_height(),
                    fill,
                    p.label.clone(),
                )
            };
            let box_top = y - 21.3;
            self.fg_primitives.push(Primitive::Rect(Rect {
                x: cx - w / 2.0,
                y: box_top,
                width: w,
                height: h,
                fill,
                stroke: self.theme.participant_border_color().to_string(),
                stroke_width: 0.5,
                rx: 2.5,
                ry: 2.5,
            }));
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: cx,
                y: box_top + h / 2.0 + self.title_measurer.line_height() * 0.32,
                content: label_text,
                font_size: self.theme.participant_font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
            self.participants[idx].materialized_y = Some(box_top + h);
        }

        // Endpoints stop at activation bar edges: the source line starts at
        // the bar's side, the arrow tip stops 2px short of the target bar
        // (1px short of the bare lifeline).
        let from_edge = if self.is_active(&msg.from) {
            ACTIVATION_HALF_W
        } else {
            0.0
        };
        let to_edge = if create_idx.is_some() {
            // stop at the new head box's edge
            self.participants
                .iter()
                .find(|p| p.name == msg.to)
                .map(|p| p.box_width / 2.0 + 2.0)
                .unwrap_or(1.0)
        } else if self.is_active(&msg.to) {
            ACTIVATION_HALF_W + 2.0
        } else {
            1.0
        };
        let (x1, x2) = if from_x <= to_x {
            (from_x + from_edge, to_x - to_edge)
        } else {
            (from_x - from_edge, to_x + to_edge)
        };

        let dashed = msg.arrow.line == LineStyle::Dashed;
        let head = match msg.arrow.head {
            ArrowHead::Filled => ArrowHeadStyle::Filled,
            ArrowHead::Open => ArrowHeadStyle::Open,
        };
        let stroke = msg
            .color
            .as_deref()
            .map(crate::theme::resolve_color)
            .unwrap_or_else(|| self.theme.arrow_color().to_string());

        // Arrow line
        self.fg_primitives.push(Primitive::Arrow(Arrow {
            x1,
            y1: y,
            x2,
            y2: y,
            stroke,
            stroke_width: 1.0,
            head,
            dashed,
        }));

        // Label above the arrow, anchored near the arrow's left end
        // (PlantUML: 7px right of the source going right, 16px right of the
        // arrowhead going left).
        if !label.is_empty() {
            let label_x = if from_x <= to_x { x1 + 7.0 } else { x2 + 16.0 };
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: label_x,
                y: y - 5.0,
                content: label.to_string(),
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
        }
    }

    fn layout_self_message(&mut self, msg: &Message, label: &str, y: f32) {
        let x = self.participant_x(&msg.from);
        let dashed = msg.arrow.line == LineStyle::Dashed;

        // PlantUML shape: out to the right from the bar/lifeline edge,
        // a short drop, and back with a left-pointing arrowhead.
        let edge = if self.is_active(&msg.from) {
            ACTIVATION_HALF_W
        } else {
            0.0
        };
        let x0 = x + edge;
        let x_right = x0 + SELF_MSG_WIDTH;
        let y_bottom = y + SELF_MSG_HEIGHT;
        let stroke = msg
            .color
            .as_deref()
            .map(crate::theme::resolve_color)
            .unwrap_or_else(|| self.theme.arrow_color().to_string());

        self.fg_primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{}",
                x0,
                y,
                x_right,
                y,
                x_right,
                y_bottom,
                x0 + 1.0,
                y_bottom,
            ),
            fill: "none".into(),
            stroke: stroke.clone(),
            stroke_width: 1.0,
            dashed,
        }));

        // Left-pointing arrowhead back into the lifeline/bar
        self.fg_primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(x0 + 1.0, y_bottom, -1.0, 0.0),
            fill: stroke,
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        // Label above the top line
        if !label.is_empty() {
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: x0 + 7.0,
                y: y - 5.0,
                content: label.to_string(),
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
            self.max_right = self
                .max_right
                .max(x0 + 7.0 + self.measurer.measure_width(label));
        }
        self.max_right = self.max_right.max(x_right);
    }

    /// `ref over A, B : text` — reference fragment frame with a "ref" tab.
    fn layout_ref_over(&mut self, r: &RefOver) {
        let members: Vec<&ParticipantInfo> = self
            .participants
            .iter()
            .filter(|p| r.participants.contains(&p.name))
            .collect();
        if members.is_empty() {
            return;
        }
        let left = members
            .iter()
            .map(|p| p.x_center - p.box_width / 2.0)
            .fold(f32::MAX, f32::min)
            - 3.0;
        let right = members
            .iter()
            .map(|p| p.x_center + p.box_width / 2.0)
            .fold(f32::MIN, f32::max)
            + 3.0;

        let lines: Vec<&str> = r.text.lines().collect();
        let tab_h = 17.0;
        let height = tab_h + lines.len() as f32 * 14.0 + 8.0;
        let top = self.y_cursor + 8.0;

        self.fg_primitives.push(Primitive::Rect(Rect {
            x: left,
            y: top,
            width: right - left,
            height,
            fill: "none".into(),
            stroke: "#000000".into(),
            stroke_width: 1.5,
            rx: 0.0,
            ry: 0.0,
        }));

        let tab_w = self.measurer.measure_width("ref") + 26.0;
        self.fg_primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (left, top),
                (left + tab_w, top),
                (left + tab_w, top + tab_h - 10.0),
                (left + tab_w - 10.0, top + tab_h),
                (left, top + tab_h),
            ],
            fill: "#EEEEEE".into(),
            stroke: "#000000".into(),
            stroke_width: 2.0,
        }));
        self.fg_primitives.push(Primitive::Text(Text {
            bold: true,
            italic: false,
            x: left + 13.0,
            y: top + 13.5,
            content: "ref".into(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));

        for (i, line) in lines.iter().enumerate() {
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: (left + right) / 2.0,
                y: top + tab_h + 13.0 + i as f32 * 14.0,
                content: line.to_string(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
        }

        self.max_right = self.max_right.max(right + 2.0);
        self.y_cursor = top + height;
    }

    /// Horizontal placement of a note: (left x, width). Depends only on the
    /// participant positions, so it can be evaluated before the y-layout pass.
    fn note_x_width(&self, note: &Note) -> (f32, f32) {
        let mut note_width =
            self.measurer.measure_multiline_width(&note.text) + NOTE_PADDING * 2.0 + NOTE_FOLD;
        let x = match &note.position {
            NotePosition::RightOf(name) => self.participant_x(name) + NOTE_MARGIN,
            NotePosition::LeftOf(name) => self.participant_x(name) - NOTE_MARGIN - note_width,
            NotePosition::Over(names) => {
                if names.len() == 1 {
                    self.participant_x(&names[0]) - note_width / 2.0
                } else {
                    // Span all named lifelines, extending 10px past each side
                    let min_x = names
                        .iter()
                        .map(|n| self.participant_x(n))
                        .fold(f32::MAX, f32::min);
                    let max_x = names
                        .iter()
                        .map(|n| self.participant_x(n))
                        .fold(f32::MIN, f32::max);
                    note_width = note_width.max(max_x - min_x + 20.0);
                    (min_x + max_x) / 2.0 - note_width / 2.0
                }
            }
        };
        (x, note_width)
    }

    /// Leftmost x any note reaches, for reserving left margin.
    fn min_note_x(&self, elements: &[SequenceElement]) -> f32 {
        let mut min_x = f32::MAX;
        for element in elements {
            match element {
                SequenceElement::Note(note) => {
                    min_x = min_x.min(self.note_x_width(note).0);
                }
                SequenceElement::Group(group) => {
                    min_x = min_x.min(self.min_note_x(&group.elements));
                    for else_block in &group.else_blocks {
                        min_x = min_x.min(self.min_note_x(&else_block.elements));
                    }
                }
                _ => {}
            }
        }
        min_x
    }

    fn layout_note(&mut self, note: &Note) {
        let note_fill = note
            .color
            .as_deref()
            .map(crate::theme::resolve_color)
            .unwrap_or_else(|| self.theme.note_bg_color().to_string());
        let (x, note_width) = self.note_x_width(note);
        let note_height = self.measurer.measure_multiline_height(&note.text) + NOTE_PADDING * 2.0;
        let y = self.y_cursor + NOTE_TOP_GAP;

        // Note body with a folded top-right corner (PlantUML shape):
        // outline with the corner cut off, plus the fold triangle.
        let right = x + note_width;
        let bottom = y + note_height;
        self.fg_primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{} L {},{} L {},{}",
                x,
                y,
                x,
                bottom,
                right,
                bottom,
                right,
                y + NOTE_FOLD,
                right - NOTE_FOLD,
                y,
                x,
                y,
            ),
            fill: note_fill.clone(),
            stroke: self.theme.note_border_color().to_string(),
            stroke_width: 0.5,
            dashed: false,
        }));
        self.fg_primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} Z",
                right - NOTE_FOLD,
                y,
                right - NOTE_FOLD,
                y + NOTE_FOLD,
                right,
                y + NOTE_FOLD,
            ),
            fill: note_fill.clone(),
            stroke: self.theme.note_border_color().to_string(),
            stroke_width: 0.5,
            dashed: false,
        }));

        // Note text
        self.fg_primitives.push(Primitive::Text(Text {
            bold: false,
            italic: false,
            x: x + NOTE_PADDING + 1.0,
            y: y + NOTE_PADDING + self.measurer.line_height() * 0.8,
            content: note.text.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));

        self.y_cursor = y + note_height;
        self.max_right = self.max_right.max(x + note_width);
    }

    fn layout_group(&mut self, group: &Group) {
        self.y_cursor += GROUP_TOP_MARGIN;
        let group_start_y = self.y_cursor;

        // The header tab overlaps the first message row's whitespace,
        // so only a small padding is added here.
        self.y_cursor += GROUP_PADDING;

        // Layout main elements
        self.layout_elements(&group.elements);

        // Layout else blocks
        let mut else_divider_ys = Vec::new();
        for else_block in &group.else_blocks {
            let divider_y = self.y_cursor + 10.0;
            else_divider_ys.push(divider_y);
            self.y_cursor = divider_y + 10.0;
            self.layout_elements(&else_block.elements);
        }

        self.y_cursor += GROUP_PADDING;

        let group_end_y = self.y_cursor;

        // The frame spans only the participants involved in the group,
        // extended by 10px beyond their boxes on each side.
        let mut involved = std::collections::HashSet::new();
        collect_group_participants(group, &mut involved);
        let span: Vec<&ParticipantInfo> = self
            .participants
            .iter()
            .filter(|p| involved.contains(&p.name))
            .collect();
        let (min_x, max_x) = if span.is_empty() {
            (
                DIAGRAM_MARGIN,
                self.calculate_total_width() - DIAGRAM_MARGIN,
            )
        } else {
            (
                span.iter()
                    .map(|p| p.x_center - p.box_width / 2.0)
                    .fold(f32::MAX, f32::min)
                    - 10.0,
                span.iter()
                    .map(|p| p.x_center + p.box_width / 2.0)
                    .fold(f32::MIN, f32::max)
                    + 10.0,
            )
        };

        self.max_right = self.max_right.max(max_x + 2.0);

        // Group frame
        self.group_primitives.push(Primitive::Rect(Rect {
            x: min_x,
            y: group_start_y,
            width: max_x - min_x,
            height: group_end_y - group_start_y,
            fill: "none".into(),
            stroke: self.theme.group_border_color().to_string(),
            stroke_width: 1.5,
            rx: 0.0,
            ry: 0.0,
        }));

        // Header tab: pentagon with a notched bottom-right corner
        let kind = group_kind_label(group.kind);
        let tab_width = self.measurer.measure_width(kind) + 30.0;
        let tab_height = GROUP_LABEL_HEIGHT;
        self.group_primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (min_x, group_start_y),
                (min_x + tab_width, group_start_y),
                (min_x + tab_width, group_start_y + tab_height - 10.0),
                (min_x + tab_width - 10.0, group_start_y + tab_height),
                (min_x, group_start_y + tab_height),
            ],
            fill: self.theme.group_label_bg_color().to_string(),
            stroke: self.theme.group_border_color().to_string(),
            stroke_width: 1.5,
        }));

        // Group kind (bold, inside the tab)
        self.group_primitives.push(Primitive::Text(Text {
            bold: true,
            italic: false,
            x: min_x + 15.0,
            y: group_start_y + 13.5,
            content: kind.to_string(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));

        // Condition label (small bold, right of the tab)
        if !group.label.is_empty() {
            self.group_primitives.push(Primitive::Text(Text {
                bold: true,
                italic: false,
                x: min_x + tab_width + 15.0,
                y: group_start_y + 12.6,
                content: format!("[{}]", group.label),
                font_size: self.theme.font_size() - 2.0,
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
        }

        // Else dividers
        for (i, &divider_y) in else_divider_ys.iter().enumerate() {
            self.group_primitives
                .push(Primitive::DashedLine(DashedLine {
                    x1: min_x,
                    y1: divider_y,
                    x2: max_x,
                    y2: divider_y,
                    stroke: self.theme.group_border_color().to_string(),
                    stroke_width: 1.0,
                    dash_array: "2,2".into(),
                }));

            let else_label = if i < group.else_blocks.len() {
                let lbl = &group.else_blocks[i].label;
                if lbl.is_empty() {
                    "[else]".to_string()
                } else {
                    format!("[{}]", lbl)
                }
            } else {
                "[else]".to_string()
            };

            self.group_primitives.push(Primitive::Text(Text {
                bold: true,
                italic: false,
                x: min_x + 5.0,
                y: divider_y + 10.6,
                content: else_label,
                font_size: self.theme.font_size() - 2.0,
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
        }
    }

    fn layout_separator(&mut self, sep: &Separator) {
        let total_width = self.calculate_total_width();
        // Center line of the separator band
        let y = self.y_cursor + MESSAGE_SPACING;

        // Double line across the full diagram width
        for dy in [-1.5, 1.5] {
            self.fg_primitives.push(Primitive::Line(Line {
                x1: 0.0,
                y1: y + dy,
                x2: total_width,
                y2: y + dy,
                stroke: self.theme.separator_color().to_string(),
                stroke_width: 1.0,
            }));
        }

        // Label box over the lines
        let label_width = self.measurer.measure_width(&sep.label) + 12.0;
        let label_height = self.measurer.line_height() + 8.0;
        let center_x = total_width / 2.0;

        self.fg_primitives.push(Primitive::Rect(Rect {
            x: center_x - label_width / 2.0,
            y: y - label_height / 2.0,
            width: label_width,
            height: label_height,
            fill: self.theme.group_bg_color().to_string(),
            stroke: self.theme.separator_color().to_string(),
            stroke_width: 2.0,
            rx: 0.0,
            ry: 0.0,
        }));

        // Label text (bold, centered)
        self.fg_primitives.push(Primitive::Text(Text {
            bold: true,
            italic: false,
            x: center_x,
            y: y + 4.5,
            content: sep.label.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Middle,
        }));

        self.y_cursor = y + label_height / 2.0;
    }

    fn layout_delay(&mut self, label: &Option<String>) {
        self.y_cursor += 10.0;

        if let Some(text) = label {
            let total_width = self.calculate_total_width();
            self.fg_primitives.push(Primitive::Text(Text {
                bold: false,
                italic: false,
                x: total_width / 2.0,
                y: self.y_cursor + self.measurer.line_height() * 0.7,
                content: text.clone(),
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "#888888".into(),
                anchor: TextAnchor::Middle,
            }));
            self.y_cursor += self.measurer.line_height();
        }

        self.y_cursor += 10.0;
    }

    fn calculate_total_width(&self) -> f32 {
        if self.participants.is_empty() {
            return 200.0;
        }
        let last = self.participants.last().unwrap();
        last.x_center + last.box_width / 2.0 + DIAGRAM_MARGIN
    }
}

/// Circle as an SVG path (two arcs).
fn circle_path_at(cx: f32, cy: f32, r: f32) -> String {
    format!(
        "M {},{} a {},{} 0 1,0 {},0 a {},{} 0 1,0 {},0",
        cx - r,
        cy,
        r,
        r,
        r * 2.0,
        r,
        r,
        -(r * 2.0),
    )
}

/// Database cylinder body centered at `cx`, top edge at `top`.
fn database_cylinder(cx: f32, top: f32, fill: &str, stroke: &str) -> Primitive {
    let l = cx - DB_CYL_W / 2.0;
    let r = cx + DB_CYL_W / 2.0;
    let bottom = top + DB_CYL_H;
    Primitive::Path(Path {
        d: format!(
            "M {},{} C {},{} {},{} {},{} C {},{} {},{} {},{} L {},{} C {},{} {},{} {},{} C {},{} {},{} {},{} Z",
            l, top + DB_CYL_CAP,
            l, top, cx, top, cx, top,
            cx, top, r, top, r, top + DB_CYL_CAP,
            r, bottom - DB_CYL_CAP,
            r, bottom, cx, bottom, cx, bottom,
            cx, bottom, l, bottom, l, bottom - DB_CYL_CAP,
        ),
        fill: fill.to_string(),
        stroke: stroke.to_string(),
        stroke_width: 0.5,
        dashed: false,
    })
}

/// The lens curve under the cylinder's top cap.
fn database_cylinder_lens(cx: f32, top: f32, stroke: &str) -> Primitive {
    let l = cx - DB_CYL_W / 2.0;
    let r = cx + DB_CYL_W / 2.0;
    let y = top + DB_CYL_CAP;
    Primitive::Path(Path {
        d: format!(
            "M {},{} C {},{} {},{} {},{} C {},{} {},{} {},{}",
            l,
            y,
            l,
            y + DB_CYL_CAP,
            cx,
            y + DB_CYL_CAP,
            cx,
            y + DB_CYL_CAP,
            cx,
            y + DB_CYL_CAP,
            r,
            y + DB_CYL_CAP,
            r,
            y,
        ),
        fill: "none".into(),
        stroke: stroke.to_string(),
        stroke_width: 0.5,
        dashed: false,
    })
}

/// Collect the names of all participants that exchange messages inside a group
/// (including its else blocks and nested groups).
fn collect_group_participants(group: &Group, names: &mut std::collections::HashSet<String>) {
    fn walk(elements: &[SequenceElement], names: &mut std::collections::HashSet<String>) {
        for element in elements {
            match element {
                SequenceElement::Message(msg) => {
                    names.insert(msg.from.clone());
                    names.insert(msg.to.clone());
                }
                SequenceElement::Group(g) => collect_group_participants(g, names),
                _ => {}
            }
        }
    }
    walk(&group.elements, names);
    for else_block in &group.else_blocks {
        walk(&else_block.elements, names);
    }
}

fn group_kind_label(kind: GroupKind) -> &'static str {
    match kind {
        GroupKind::Alt => "alt",
        GroupKind::Else => "else",
        GroupKind::Loop => "loop",
        GroupKind::Opt => "opt",
        GroupKind::Break => "break",
        GroupKind::Par => "par",
        GroupKind::Critical => "critical",
        GroupKind::Group => "group",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::sequence::parse;

    fn layout_from_text(input: &str) -> LaidOutDiagram {
        let diagram = parse(input).unwrap();
        layout(&diagram)
    }

    #[test]
    fn test_simple_message_layout() {
        let laid_out = layout_from_text("Alice -> Bob : Hello");
        assert!(laid_out.width > 0.0);
        assert!(laid_out.height > 0.0);
        assert!(!laid_out.primitives.is_empty());
    }

    #[test]
    fn test_layout_has_participant_boxes() {
        let laid_out = layout_from_text("Alice -> Bob : Hello");
        let rect_count = laid_out
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(_)))
            .count();
        // Background + 2 participants top + 2 participants bottom = 4 participant rects minimum
        assert!(
            rect_count >= 4,
            "expected at least 4 rects, got {}",
            rect_count
        );
    }

    #[test]
    fn test_layout_has_lifelines() {
        let laid_out = layout_from_text("Alice -> Bob : Hello");
        let dashed_count = laid_out
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::DashedLine(_)))
            .count();
        assert!(dashed_count >= 2, "expected lifelines for 2 participants");
    }

    #[test]
    fn test_layout_has_arrow() {
        let laid_out = layout_from_text("Alice -> Bob : Hello");
        let arrow_count = laid_out
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow(_)))
            .count();
        assert_eq!(arrow_count, 1);
    }

    #[test]
    fn test_layout_with_group() {
        let input = "alt success\nAlice -> Bob : OK\nelse failure\nAlice -> Bob : Error\nend";
        let laid_out = layout_from_text(input);
        assert!(laid_out.width > 0.0);
        assert!(laid_out.height > 0.0);
        // Should have group frame rect
        let has_group_frame = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Rect(r) if r.fill == "none"));
        assert!(has_group_frame, "expected group frame rectangle");
    }

    #[test]
    fn test_layout_with_note() {
        let input = "Alice -> Bob : Hello\nnote right of Alice : This is a note";
        let laid_out = layout_from_text(input);
        // Note should add a rect with note background color
        let has_note = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Path(path) if path.fill == "#FEFFDD"));
        assert!(has_note, "expected note body path");
    }

    #[test]
    fn test_layout_with_separator() {
        let input = "Alice -> Bob : Hello\n== Phase 2 ==\nBob -> Alice : World";
        let laid_out = layout_from_text(input);
        // Separator should include the label box on the double line
        let has_sep_rect = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Rect(r) if r.fill == "#EEEEEE"));
        assert!(has_sep_rect, "expected separator label rect");
    }

    #[test]
    fn test_layout_self_message() {
        let input = "Alice -> Alice : Think";
        let laid_out = layout_from_text(input);
        // Self-message uses Path for the U-shape
        let has_path = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Path(_)));
        assert!(has_path, "expected path for self-message");
    }

    #[test]
    fn test_layout_three_participants() {
        let input = "Alice -> Bob : Hello\nBob -> Charlie : Forward\nCharlie --> Alice : Reply";
        let laid_out = layout_from_text(input);
        let arrow_count = laid_out
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Arrow(_)))
            .count();
        assert_eq!(arrow_count, 3);
    }

    #[test]
    fn test_full_diagram_renders_to_svg() {
        let input = r#"title Auth Flow
participant Client
participant Server

Client -> Server : POST /login
activate Server
alt success
    Server --> Client : 200 OK
else failure
    Server --> Client : 401
end
deactivate Server"#;

        let laid_out = layout_from_text(input);
        let svg = crate::render::svg::render(&laid_out);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("Auth Flow"));
        assert!(svg.contains("Client"));
        assert!(svg.contains("Server"));
        assert!(svg.contains("POST /login"));
    }

    // Regression: else label and message content must not overlap vertically.
    // The else divider needs GROUP_LABEL_HEIGHT spacing, not just GROUP_PADDING.
    #[test]
    fn test_else_block_has_enough_vertical_space() {
        let input = "alt ok\nA -> B : yes\nelse fail\nA -> B : no\nend";
        let laid_out = layout_from_text(input);

        // Find the else label text and the "no" message text
        let texts: Vec<&Text> = laid_out
            .primitives
            .iter()
            .filter_map(|p| {
                if let Primitive::Text(t) = p {
                    Some(t)
                } else {
                    None
                }
            })
            .collect();

        let else_text = texts.iter().find(|t| t.content == "[fail]").unwrap();
        let no_text = texts.iter().find(|t| t.content == "no").unwrap();

        // The "no" message label must be below the else label, not overlapping
        assert!(
            no_text.y > else_text.y + 5.0,
            "else label (y={}) and message 'no' (y={}) overlap",
            else_text.y,
            no_text.y,
        );
    }
}
