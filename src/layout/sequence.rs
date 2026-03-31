use crate::ast::sequence::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{DefaultTheme, Theme};

// Layout constants
const PADDING: f32 = 10.0;
const PARTICIPANT_PADDING_H: f32 = 16.0;
const PARTICIPANT_PADDING_V: f32 = 10.0;
const PARTICIPANT_MARGIN: f32 = 30.0;
const MESSAGE_SPACING: f32 = 40.0;
const NOTE_PADDING: f32 = 8.0;
const NOTE_MARGIN: f32 = 10.0;
const GROUP_PADDING: f32 = 8.0;
const GROUP_LABEL_HEIGHT: f32 = 20.0;
// Reserved for future activation bar rendering
// const ACTIVATION_WIDTH: f32 = 10.0;
const SELF_MSG_WIDTH: f32 = 40.0;
const SELF_MSG_HEIGHT: f32 = 30.0;
const SEPARATOR_MARGIN: f32 = 10.0;
const DIAGRAM_MARGIN: f32 = 20.0;
const TITLE_MARGIN: f32 = 10.0;

/// Layout a parsed sequence diagram into primitives with computed positions.
pub fn layout(diagram: &SequenceDiagram) -> LaidOutDiagram {
    let theme = DefaultTheme;
    let measurer = TextMeasurer::new(theme.font_size());
    let title_measurer = TextMeasurer::new(theme.participant_font_size() + 2.0);

    let mut ctx = LayoutContext::new(&theme, &measurer, &title_measurer);
    ctx.layout(diagram)
}

struct ParticipantInfo {
    name: String,
    label: String,
    #[allow(dead_code)]
    kind: ParticipantKind,
    x_center: f32,
    box_width: f32,
    box_height: f32,
}

struct LayoutContext<'a> {
    theme: &'a dyn Theme,
    measurer: &'a TextMeasurer,
    title_measurer: &'a TextMeasurer,
    participants: Vec<ParticipantInfo>,
    /// Background layer: lifelines (drawn behind everything)
    bg_primitives: Vec<Primitive>,
    /// Middle layer: group frames (behind messages but above lifelines)
    group_primitives: Vec<Primitive>,
    /// Foreground layer: participant boxes, messages, notes, text
    fg_primitives: Vec<Primitive>,
    y_cursor: f32,
    auto_number: Option<u32>,
    active_participants: Vec<String>,
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
            bg_primitives: Vec::new(),
            group_primitives: Vec::new(),
            fg_primitives: Vec::new(),
            y_cursor: DIAGRAM_MARGIN,
            auto_number: None,
            active_participants: Vec::new(),
        }
    }

    fn layout(&mut self, diagram: &SequenceDiagram) -> LaidOutDiagram {
        // Collect all participants (declared + implicit from messages)
        self.collect_participants(diagram);

        // Position participants horizontally
        self.position_participants();

        // Draw title if present
        if let Some(title) = &diagram.title {
            self.draw_title(title);
        }

        // Draw participant boxes (top)
        let participant_top_y = self.y_cursor;
        self.draw_participant_boxes(participant_top_y);
        self.y_cursor += self.participant_box_height() + PADDING;

        let lifeline_start_y = self.y_cursor;

        // Layout elements
        self.layout_elements(&diagram.elements);

        self.y_cursor += PADDING;

        // Draw participant boxes (bottom)
        let bottom_box_y = self.y_cursor;
        self.draw_participant_boxes(bottom_box_y);
        self.y_cursor += self.participant_box_height() + DIAGRAM_MARGIN;

        // Draw lifelines (from bottom of top box to top of bottom box)
        self.draw_lifelines(lifeline_start_y, bottom_box_y);

        // Calculate total dimensions
        let total_width = self.calculate_total_width();
        let total_height = self.y_cursor;

        // Merge layers: background (lifelines) → groups → foreground (boxes, messages, notes)
        let mut primitives = Vec::new();
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
                    let box_width = self.measurer.measure_width(&label) + PARTICIPANT_PADDING_H * 2.0;
                    self.participants.push(ParticipantInfo {
                        name: p.name.clone(),
                        label,
                        kind: p.kind,
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
                            let box_width =
                                self.measurer.measure_width(name) + PARTICIPANT_PADDING_H * 2.0;
                            self.participants.push(ParticipantInfo {
                                name: name.clone(),
                                label: name.clone(),
                                kind: ParticipantKind::Participant,
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

    fn position_participants(&mut self) {
        let box_height = self.participant_box_height();
        let mut x = DIAGRAM_MARGIN;

        for p in &mut self.participants {
            p.box_height = box_height;
            p.x_center = x + p.box_width / 2.0;
            x += p.box_width + PARTICIPANT_MARGIN;
        }
    }

    fn participant_box_height(&self) -> f32 {
        self.measurer.line_height() + PARTICIPANT_PADDING_V * 2.0
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
            x: center_x,
            y: self.y_cursor + self.title_measurer.line_height(),
            content: title.to_string(),
            font_size: self.theme.participant_font_size() + 2.0,
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Middle,
        }));
        self.y_cursor += self.title_measurer.line_height() + TITLE_MARGIN;
    }

    fn draw_participant_boxes(&mut self, y: f32) {
        for p in &self.participants {
            // Draw box
            self.fg_primitives.push(Primitive::Rect(Rect {
                x: p.x_center - p.box_width / 2.0,
                y,
                width: p.box_width,
                height: p.box_height,
                fill: self.theme.participant_bg_color().to_string(),
                stroke: self.theme.participant_border_color().to_string(),
                stroke_width: 1.5,
                rx: 0.0,
                ry: 0.0,
            }));

            // Draw label
            self.fg_primitives.push(Primitive::Text(Text {
                x: p.x_center,
                y: y + p.box_height / 2.0 + self.measurer.line_height() * 0.3,
                content: p.label.clone(),
                font_size: self.theme.participant_font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
        }
    }

    fn draw_lifelines(&mut self, start_y: f32, end_y: f32) {
        for p in &self.participants {
            self.bg_primitives.push(Primitive::DashedLine(DashedLine {
                x1: p.x_center,
                y1: start_y,
                x2: p.x_center,
                y2: end_y,
                stroke: self.theme.lifeline_color().to_string(),
                stroke_width: 1.0,
                dash_array: "5,5".into(),
            }));
        }
    }

    fn layout_elements(&mut self, elements: &[SequenceElement]) {
        for element in elements {
            match element {
                SequenceElement::ParticipantDecl(_) => {
                    // Already handled
                }
                SequenceElement::Message(msg) => self.layout_message(msg),
                SequenceElement::Note(note) => self.layout_note(note),
                SequenceElement::Group(group) => self.layout_group(group),
                SequenceElement::Separator(sep) => self.layout_separator(sep),
                SequenceElement::Activate(name) => {
                    self.active_participants.push(name.clone());
                }
                SequenceElement::Deactivate(name) => {
                    if let Some(pos) = self.active_participants.iter().position(|n| n == name) {
                        self.active_participants.remove(pos);
                    }
                }
                SequenceElement::AutoNumber(config) => {
                    self.auto_number = Some(config.start.unwrap_or(1));
                }
                SequenceElement::Delay(label) => self.layout_delay(label),
                SequenceElement::Space(n) => {
                    self.y_cursor += n.unwrap_or(15) as f32;
                }
            }
        }
    }

    fn layout_message(&mut self, msg: &Message) {
        let y = self.y_cursor + MESSAGE_SPACING / 2.0;

        let mut label = msg.label.clone();
        if let Some(ref mut num) = self.auto_number {
            label = format!("{} {}", num, label);
            *num += 1;
        }

        if msg.is_self_referencing {
            self.layout_self_message(msg, &label, y);
        } else {
            self.layout_normal_message(msg, &label, y);
        }

        self.y_cursor += MESSAGE_SPACING;
    }

    fn layout_normal_message(&mut self, msg: &Message, label: &str, y: f32) {
        let from_x = self.participant_x(&msg.from);
        let to_x = self.participant_x(&msg.to);

        let dashed = msg.arrow.line == LineStyle::Dashed;
        let head = match msg.arrow.head {
            ArrowHead::Filled => ArrowHeadStyle::Filled,
            ArrowHead::Open => ArrowHeadStyle::Open,
        };

        // Arrow line
        self.fg_primitives.push(Primitive::Arrow(Arrow {
            x1: from_x,
            y1: y,
            x2: to_x,
            y2: y,
            stroke: self.theme.arrow_color().to_string(),
            stroke_width: 1.0,
            head,
            dashed,
        }));

        // Label above the arrow
        if !label.is_empty() {
            let label_x = (from_x + to_x) / 2.0;
            let label_y = y - 6.0;
            self.fg_primitives.push(Primitive::Text(Text {
                x: label_x,
                y: label_y,
                content: label.to_string(),
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
        }
    }

    fn layout_self_message(&mut self, msg: &Message, label: &str, y: f32) {
        let x = self.participant_x(&msg.from);
        let dashed = msg.arrow.line == LineStyle::Dashed;

        // Self-referencing message: draw a U-shape to the right
        let x_right = x + SELF_MSG_WIDTH;
        let y_bottom = y + SELF_MSG_HEIGHT;

        let d = format!(
            "M {},{} L {},{} L {},{} L {},{}",
            x, y, x_right, y, x_right, y_bottom, x, y_bottom,
        );

        self.fg_primitives.push(Primitive::Path(Path {
            d,
            fill: "none".into(),
            stroke: self.theme.arrow_color().to_string(),
            stroke_width: 1.0,
            dashed,
        }));

        // Arrowhead at end
        let head = match msg.arrow.head {
            ArrowHead::Filled => ArrowHeadStyle::Filled,
            ArrowHead::Open => ArrowHeadStyle::Open,
        };
        // Small arrowhead pointing left at (x, y_bottom)
        let head_size = 6.0;
        match head {
            ArrowHeadStyle::Filled => {
                self.fg_primitives.push(Primitive::Polygon(Polygon {
                    points: vec![
                        (x, y_bottom),
                        (x + head_size, y_bottom - head_size / 2.0),
                        (x + head_size, y_bottom + head_size / 2.0),
                    ],
                    fill: self.theme.arrow_color().to_string(),
                    stroke: "none".into(),
                    stroke_width: 0.0,
                }));
            }
            ArrowHeadStyle::Open => {
                self.fg_primitives.push(Primitive::Path(Path {
                    d: format!(
                        "M {},{} L {},{} M {},{} L {},{}",
                        x + head_size,
                        y_bottom - head_size / 2.0,
                        x,
                        y_bottom,
                        x,
                        y_bottom,
                        x + head_size,
                        y_bottom + head_size / 2.0,
                    ),
                    fill: "none".into(),
                    stroke: self.theme.arrow_color().to_string(),
                    stroke_width: 1.0,
                    dashed: false,
                }));
            }
        }

        // Label
        if !label.is_empty() {
            self.fg_primitives.push(Primitive::Text(Text {
                x: x + SELF_MSG_WIDTH + 4.0,
                y: y + SELF_MSG_HEIGHT / 2.0 + 4.0,
                content: label.to_string(),
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
        }

        self.y_cursor += SELF_MSG_HEIGHT;
    }

    fn layout_note(&mut self, note: &Note) {
        let note_width = self.measurer.measure_multiline_width(&note.text) + NOTE_PADDING * 2.0;
        let note_height =
            self.measurer.measure_multiline_height(&note.text) + NOTE_PADDING * 2.0;

        let (x, y) = match &note.position {
            NotePosition::RightOf(name) => {
                let px = self.participant_x(name);
                (px + NOTE_MARGIN, self.y_cursor)
            }
            NotePosition::LeftOf(name) => {
                let px = self.participant_x(name);
                (px - NOTE_MARGIN - note_width, self.y_cursor)
            }
            NotePosition::Over(names) => {
                if names.len() == 1 {
                    let px = self.participant_x(&names[0]);
                    (px - note_width / 2.0, self.y_cursor)
                } else {
                    let min_x = names.iter().map(|n| self.participant_x(n)).fold(f32::MAX, f32::min);
                    let max_x = names.iter().map(|n| self.participant_x(n)).fold(f32::MIN, f32::max);
                    let center = (min_x + max_x) / 2.0;
                    (center - note_width / 2.0, self.y_cursor)
                }
            }
        };

        // Note rectangle
        self.fg_primitives.push(Primitive::Rect(Rect {
            x,
            y,
            width: note_width,
            height: note_height,
            fill: self.theme.note_bg_color().to_string(),
            stroke: self.theme.note_border_color().to_string(),
            stroke_width: 1.0,
            rx: 0.0,
            ry: 0.0,
        }));

        // Folded corner
        let fold_size = 7.0;
        let fx = x + note_width - fold_size;
        let fy = y;
        self.fg_primitives.push(Primitive::Path(Path {
            d: format!("M {},{} L {},{} L {},{} Z", fx, fy, fx, fy + fold_size, x + note_width, fy + fold_size),
            fill: self.theme.note_border_color().to_string(),
            stroke: "none".into(),
            stroke_width: 0.0,
            dashed: false,
        }));

        // Note text
        self.fg_primitives.push(Primitive::Text(Text {
            x: x + NOTE_PADDING,
            y: y + NOTE_PADDING + self.measurer.line_height() * 0.7,
            content: note.text.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));

        self.y_cursor += note_height + PADDING;
    }

    fn layout_group(&mut self, group: &Group) {
        let group_start_y = self.y_cursor;
        let label = format!("{}{}", group_kind_label(group.kind), if group.label.is_empty() { String::new() } else { format!(" [{}]", group.label) });

        self.y_cursor += GROUP_LABEL_HEIGHT + GROUP_PADDING;

        // Layout main elements
        self.layout_elements(&group.elements);

        // Layout else blocks
        let mut else_divider_ys = Vec::new();
        for else_block in &group.else_blocks {
            else_divider_ys.push(self.y_cursor);
            // Space for else label text + padding before content
            self.y_cursor += GROUP_LABEL_HEIGHT + GROUP_PADDING;
            self.layout_elements(&else_block.elements);
        }

        self.y_cursor += GROUP_PADDING;

        let group_end_y = self.y_cursor;

        // Calculate group width (span all participants)
        let min_x = self
            .participants
            .iter()
            .map(|p| p.x_center - p.box_width / 2.0)
            .fold(f32::MAX, f32::min)
            - GROUP_PADDING;
        let max_x = self
            .participants
            .iter()
            .map(|p| p.x_center + p.box_width / 2.0)
            .fold(f32::MIN, f32::max)
            + GROUP_PADDING;

        // Group frame
        self.group_primitives.push(Primitive::Rect(Rect {
            x: min_x,
            y: group_start_y,
            width: max_x - min_x,
            height: group_end_y - group_start_y,
            fill: "none".into(),
            stroke: self.theme.group_border_color().to_string(),
            stroke_width: 1.0,
            rx: 0.0,
            ry: 0.0,
        }));

        // Group label background (pentagon-like tab)
        let label_width = self.measurer.measure_width(&label) + 20.0;
        let tab_height = GROUP_LABEL_HEIGHT;
        self.group_primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (min_x, group_start_y),
                (min_x + label_width, group_start_y),
                (min_x + label_width, group_start_y + tab_height - 5.0),
                (min_x + label_width - 5.0, group_start_y + tab_height),
                (min_x, group_start_y + tab_height),
            ],
            fill: self.theme.group_label_bg_color().to_string(),
            stroke: self.theme.group_border_color().to_string(),
            stroke_width: 1.0,
        }));

        // Group label text
        self.group_primitives.push(Primitive::Text(Text {
            x: min_x + 10.0,
            y: group_start_y + tab_height - 5.0,
            content: label,
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));

        // Else dividers
        for (i, &divider_y) in else_divider_ys.iter().enumerate() {
            self.group_primitives.push(Primitive::DashedLine(DashedLine {
                x1: min_x,
                y1: divider_y,
                x2: max_x,
                y2: divider_y,
                stroke: self.theme.group_border_color().to_string(),
                stroke_width: 1.0,
                dash_array: "5,5".into(),
            }));

            let else_label = if i < group.else_blocks.len() {
                let lbl = &group.else_blocks[i].label;
                if lbl.is_empty() {
                    "else".to_string()
                } else {
                    format!("else [{}]", lbl)
                }
            } else {
                "else".to_string()
            };

            self.group_primitives.push(Primitive::Text(Text {
                x: min_x + 10.0,
                y: divider_y + self.measurer.line_height(),
                content: else_label,
                font_size: self.theme.font_size(),
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Start,
            }));
        }
    }

    fn layout_separator(&mut self, sep: &Separator) {
        self.y_cursor += SEPARATOR_MARGIN;

        let total_width = self.calculate_total_width();
        let y = self.y_cursor;

        // Double line separator
        self.fg_primitives.push(Primitive::Line(Line {
            x1: 0.0,
            y1: y,
            x2: total_width,
            y2: y,
            stroke: self.theme.separator_color().to_string(),
            stroke_width: 1.0,
        }));

        // Background for label
        let label_width = self.measurer.measure_width(&sep.label) + 20.0;
        let label_height = self.measurer.line_height() + 4.0;
        let center_x = total_width / 2.0;

        self.fg_primitives.push(Primitive::Rect(Rect {
            x: center_x - label_width / 2.0,
            y: y - label_height / 2.0,
            width: label_width,
            height: label_height,
            fill: self.theme.group_bg_color().to_string(),
            stroke: self.theme.separator_color().to_string(),
            stroke_width: 1.0,
            rx: 3.0,
            ry: 3.0,
        }));

        // Label text
        self.fg_primitives.push(Primitive::Text(Text {
            x: center_x,
            y: y + self.measurer.line_height() * 0.3,
            content: sep.label.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Middle,
        }));

        self.y_cursor += label_height + SEPARATOR_MARGIN;
    }

    fn layout_delay(&mut self, label: &Option<String>) {
        self.y_cursor += 10.0;

        if let Some(text) = label {
            let total_width = self.calculate_total_width();
            self.fg_primitives.push(Primitive::Text(Text {
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
        assert!(rect_count >= 4, "expected at least 4 rects, got {}", rect_count);
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
        let has_group_frame = laid_out.primitives.iter().any(|p| {
            matches!(p, Primitive::Rect(r) if r.fill == "none")
        });
        assert!(has_group_frame, "expected group frame rectangle");
    }

    #[test]
    fn test_layout_with_note() {
        let input = "Alice -> Bob : Hello\nnote right of Alice : This is a note";
        let laid_out = layout_from_text(input);
        // Note should add a rect with note background color
        let has_note = laid_out.primitives.iter().any(|p| {
            matches!(p, Primitive::Rect(r) if r.fill == "#FBFB77")
        });
        assert!(has_note, "expected note rectangle");
    }

    #[test]
    fn test_layout_with_separator() {
        let input = "Alice -> Bob : Hello\n== Phase 2 ==\nBob -> Alice : World";
        let laid_out = layout_from_text(input);
        // Separator should include a rect with label background
        let has_sep_rect = laid_out.primitives.iter().any(|p| {
            matches!(p, Primitive::Rect(r) if r.rx > 0.0)
        });
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
        let texts: Vec<&Text> = laid_out.primitives.iter().filter_map(|p| {
            if let Primitive::Text(t) = p { Some(t) } else { None }
        }).collect();

        let else_text = texts.iter().find(|t| t.content.starts_with("else")).unwrap();
        let no_text = texts.iter().find(|t| t.content == "no").unwrap();

        // The "no" message label must be below the else label, not overlapping
        assert!(
            no_text.y > else_text.y + 5.0,
            "else label (y={}) and message 'no' (y={}) overlap",
            else_text.y, no_text.y,
        );
    }
}
