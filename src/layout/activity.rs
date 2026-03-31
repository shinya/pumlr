use crate::ast::activity::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{DefaultTheme, Theme};

const DIAGRAM_MARGIN: f32 = 20.0;
const ACTION_PADDING_H: f32 = 16.0;
const ACTION_PADDING_V: f32 = 10.0;
const ACTION_RADIUS: f32 = 10.0;
const ARROW_SPACING: f32 = 20.0;
// Used as default by draw_diamond (which is kept for tests/future use)
#[allow(dead_code)]
const DIAMOND_SIZE: f32 = 14.0;
const MIN_DIAMOND_SIZE: f32 = 16.0;
/// Small diamond for merge points (no text)
const MERGE_DIAMOND_SIZE: f32 = 14.0;
const START_STOP_RADIUS: f32 = 12.0;
const MIN_ACTION_WIDTH: f32 = 80.0;
const BRANCH_GAP: f32 = 30.0;
const NOTE_PADDING: f32 = 8.0;
const NOTE_MARGIN: f32 = 10.0;
const PARTITION_PADDING: f32 = 12.0;
const TITLE_MARGIN: f32 = 10.0;
const FORK_BAR_HEIGHT: f32 = 4.0;
const FORK_BAR_EXTEND: f32 = 20.0;

/// Layout a parsed activity diagram into primitives.
pub fn layout(diagram: &ActivityDiagram) -> LaidOutDiagram {
    let theme = DefaultTheme;
    let measurer = TextMeasurer::new(theme.font_size());

    let mut ctx = ActivityLayoutContext::new(&theme, &measurer);
    ctx.layout(diagram)
}

/// Represents the bounding box of a laid-out subtree.
#[derive(Debug, Clone, Copy)]
struct SubtreeBox {
    width: f32,
    height: f32,
    /// X offset of the connection point (center of flow) from the left edge
    center_x: f32,
}

struct ActivityLayoutContext<'a> {
    theme: &'a dyn Theme,
    measurer: &'a TextMeasurer,
    primitives: Vec<Primitive>,
}

impl<'a> ActivityLayoutContext<'a> {
    fn new(theme: &'a dyn Theme, measurer: &'a TextMeasurer) -> Self {
        Self {
            theme,
            measurer,
            primitives: Vec::new(),
        }
    }

    fn layout(&mut self, diagram: &ActivityDiagram) -> LaidOutDiagram {
        // First pass: measure total size
        let subtree = self.measure_elements(&diagram.elements);

        let title_height = if diagram.title.is_some() {
            self.measurer.line_height() + TITLE_MARGIN
        } else {
            0.0
        };

        let total_height = subtree.height + DIAGRAM_MARGIN * 2.0 + title_height;

        // Second pass: draw at center_x from measure (symmetric diamond placement)
        let start_x = DIAGRAM_MARGIN + subtree.center_x;
        let mut y = DIAGRAM_MARGIN;

        if let Some(title) = &diagram.title {
            self.primitives.push(Primitive::Text(Text {
                x: start_x, // will be re-centered by post-process shift
                y: y + self.measurer.line_height(),
                content: title.clone(),
                font_size: self.theme.participant_font_size() + 2.0,
                font_family: self.theme.font_family().to_string(),
                fill: "black".into(),
                anchor: TextAnchor::Middle,
            }));
            y += self.measurer.line_height() + TITLE_MARGIN;
        }

        self.draw_elements(&diagram.elements, start_x, y);

        // Post-process: find actual bounding box and shift to prevent clipping
        let (min_x, max_x) = self.find_x_bounds();
        let shift = if min_x < DIAGRAM_MARGIN {
            DIAGRAM_MARGIN - min_x
        } else {
            0.0
        };

        if shift > 0.0 {
            self.shift_all_x(shift);
        }

        let actual_width = (max_x + shift + DIAGRAM_MARGIN).max(subtree.width + DIAGRAM_MARGIN * 2.0);

        LaidOutDiagram {
            width: actual_width,
            height: total_height,
            primitives: std::mem::take(&mut self.primitives),
        }
    }

    // --- Measure pass ---

    fn measure_elements(&self, elements: &[ActivityElement]) -> SubtreeBox {
        let mut total_height = 0.0;
        let mut max_left = 0.0f32; // max extent to the left of center
        let mut max_right = 0.0f32; // max extent to the right of center

        for element in elements {
            let sub = self.measure_element(element);
            total_height += sub.height + ARROW_SPACING;
            max_left = max_left.max(sub.center_x);
            max_right = max_right.max(sub.width - sub.center_x);
        }

        // Remove trailing spacing
        if !elements.is_empty() {
            total_height -= ARROW_SPACING;
        }

        SubtreeBox {
            width: max_left + max_right,
            height: total_height,
            center_x: max_left,
        }
    }

    fn measure_element(&self, element: &ActivityElement) -> SubtreeBox {
        match element {
            ActivityElement::Start | ActivityElement::Stop | ActivityElement::End => SubtreeBox {
                width: START_STOP_RADIUS * 2.0,
                height: START_STOP_RADIUS * 2.0,
                center_x: START_STOP_RADIUS,
            },
            ActivityElement::Detach => SubtreeBox {
                width: 30.0,
                height: 20.0,
                center_x: 15.0,
            },
            ActivityElement::Action(action) => {
                let text_w = self.measurer.measure_multiline_width(&action.label);
                let text_h = self.measurer.measure_multiline_height(&action.label);
                let w = (text_w + ACTION_PADDING_H * 2.0).max(MIN_ACTION_WIDTH);
                let h = text_h + ACTION_PADDING_V * 2.0;
                SubtreeBox {
                    width: w,
                    height: h,
                    center_x: w / 2.0,
                }
            }
            ActivityElement::If(block) => self.measure_if(block),
            ActivityElement::While(block) => self.measure_while(block),
            ActivityElement::Fork(block) => self.measure_fork(block),
            ActivityElement::Switch(block) => self.measure_switch(block),
            ActivityElement::Partition(partition) => {
                let inner = self.measure_elements(&partition.elements);
                let label_w = self.measurer.measure_width(&partition.name) + PARTITION_PADDING * 2.0;
                let w = inner.width.max(label_w) + PARTITION_PADDING * 2.0;
                let h = inner.height + PARTITION_PADDING * 2.0 + self.measurer.line_height();
                SubtreeBox {
                    width: w,
                    height: h,
                    center_x: w / 2.0,
                }
            }
            ActivityElement::Note(note) => {
                let text_w = self.measurer.measure_multiline_width(&note.text);
                let text_h = self.measurer.measure_multiline_height(&note.text);
                SubtreeBox {
                    width: text_w + NOTE_PADDING * 2.0 + NOTE_MARGIN,
                    height: text_h + NOTE_PADDING * 2.0,
                    center_x: 0.0,
                }
            }
            ActivityElement::Arrow(arrow) => {
                let w = self.measurer.measure_width(&arrow.label) + 20.0;
                SubtreeBox {
                    width: w,
                    height: 10.0,
                    center_x: w / 2.0,
                }
            }
        }
    }

    fn measure_if(&self, block: &IfBlock) -> SubtreeBox {
        let then_box = self.measure_elements(&block.then_elements);
        let else_box = self.measure_elements(&block.else_elements);

        let diamond_sz = self.diamond_size_for_text(&block.condition);

        // Branches width = then + gap + else (+ elseif branches)
        let mut branches_width = then_box.width + BRANCH_GAP + else_box.width;
        let mut max_branch_height = then_box.height.max(else_box.height);

        for elseif in &block.elseif_blocks {
            let b = self.measure_elements(&elseif.elements);
            branches_width += BRANCH_GAP + b.width;
            max_branch_height = max_branch_height.max(b.height);
        }

        // Diamond at visual center of branches (symmetric placement)
        let total_width = branches_width.max(diamond_sz * 2.0);
        let center_x = total_width / 2.0;
        let h = diamond_sz * 2.0 + ARROW_SPACING * 2.0 + max_branch_height + MERGE_DIAMOND_SIZE * 2.0;
        SubtreeBox {
            width: total_width,
            height: h,
            center_x,
        }
    }

    fn measure_while(&self, block: &WhileBlock) -> SubtreeBox {
        let dsz = self.diamond_size_for_text(&block.condition);
        let inner = self.measure_elements(&block.elements);
        let w = inner.width + BRANCH_GAP;
        let h = dsz * 2.0 + ARROW_SPACING + inner.height + ARROW_SPACING;
        SubtreeBox {
            width: w,
            height: h,
            center_x: w / 2.0,
        }
    }

    fn measure_fork(&self, block: &ForkBlock) -> SubtreeBox {
        let branch_boxes: Vec<SubtreeBox> = block
            .branches
            .iter()
            .map(|b| self.measure_elements(b))
            .collect();

        let total_width: f32 = branch_boxes.iter().map(|b| b.width).sum::<f32>()
            + BRANCH_GAP * (branch_boxes.len().saturating_sub(1) as f32);
        let max_height = branch_boxes.iter().map(|b| b.height).fold(0.0f32, f32::max);

        let h = FORK_BAR_HEIGHT + ARROW_SPACING + max_height + ARROW_SPACING + FORK_BAR_HEIGHT;
        SubtreeBox {
            width: total_width,
            height: h,
            center_x: total_width / 2.0,
        }
    }

    fn measure_switch(&self, block: &SwitchBlock) -> SubtreeBox {
        let case_boxes: Vec<SubtreeBox> = block
            .cases
            .iter()
            .map(|c| self.measure_elements(&c.elements))
            .collect();

        let total_width: f32 = case_boxes.iter().map(|b| b.width.max(60.0)).sum::<f32>()
            + BRANCH_GAP * (case_boxes.len().saturating_sub(1) as f32);
        let max_height = case_boxes.iter().map(|b| b.height).fold(0.0f32, f32::max);

        let dsz = self.diamond_size_for_text(&block.condition);
        let h = dsz * 2.0 + ARROW_SPACING + max_height + ARROW_SPACING + MERGE_DIAMOND_SIZE * 2.0;
        SubtreeBox {
            width: total_width,
            height: h,
            center_x: total_width / 2.0,
        }
    }

    // --- Draw pass ---

    fn draw_elements(&mut self, elements: &[ActivityElement], center_x: f32, mut y: f32) -> f32 {
        for (i, element) in elements.iter().enumerate() {
            // Draw downward arrow from previous element (except before the first)
            if i > 0 {
                self.draw_down_arrow(center_x, y, y + ARROW_SPACING, None);
                y += ARROW_SPACING;
            }

            y = self.draw_element(element, center_x, y);
        }
        y
    }

    fn draw_element(&mut self, element: &ActivityElement, center_x: f32, y: f32) -> f32 {
        match element {
            ActivityElement::Start => {
                self.draw_filled_circle(center_x, y + START_STOP_RADIUS, START_STOP_RADIUS);
                y + START_STOP_RADIUS * 2.0
            }
            ActivityElement::Stop => {
                self.draw_stop_circle(center_x, y + START_STOP_RADIUS, START_STOP_RADIUS);
                y + START_STOP_RADIUS * 2.0
            }
            ActivityElement::End => {
                self.draw_stop_circle(center_x, y + START_STOP_RADIUS, START_STOP_RADIUS);
                y + START_STOP_RADIUS * 2.0
            }
            ActivityElement::Detach => {
                self.draw_detach(center_x, y);
                y + 20.0
            }
            ActivityElement::Action(action) => {
                self.draw_action(action, center_x, y)
            }
            ActivityElement::If(block) => self.draw_if(block, center_x, y),
            ActivityElement::While(block) => self.draw_while(block, center_x, y),
            ActivityElement::Fork(block) => self.draw_fork(block, center_x, y),
            ActivityElement::Switch(block) => self.draw_switch(block, center_x, y),
            ActivityElement::Partition(partition) => self.draw_partition(partition, center_x, y),
            ActivityElement::Note(note) => {
                self.draw_note(note, center_x, y);
                let sub = self.measure_element(element);
                y + sub.height
            }
            ActivityElement::Arrow(_) => {
                // Arrow label is drawn as part of the connecting arrow
                // Just reserve space
                y + 10.0
            }
        }
    }

    fn draw_action(&mut self, action: &Action, center_x: f32, y: f32) -> f32 {
        let text_w = self.measurer.measure_multiline_width(&action.label);
        let text_h = self.measurer.measure_multiline_height(&action.label);
        let w = (text_w + ACTION_PADDING_H * 2.0).max(MIN_ACTION_WIDTH);
        let h = text_h + ACTION_PADDING_V * 2.0;

        self.primitives.push(Primitive::Rect(Rect {
            x: center_x - w / 2.0,
            y,
            width: w,
            height: h,
            fill: "#FEFECE".into(),
            stroke: "#A80036".into(),
            stroke_width: 1.5,
            rx: ACTION_RADIUS,
            ry: ACTION_RADIUS,
        }));

        self.primitives.push(Primitive::Text(Text {
            x: center_x,
            y: y + h / 2.0 + self.measurer.line_height() * 0.3,
            content: action.label.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "#333333".into(),
            anchor: TextAnchor::Middle,
        }));

        y + h
    }

    fn draw_if(&mut self, block: &IfBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_if(block);
        let then_box = self.measure_elements(&block.then_elements);
        let else_box = self.measure_elements(&block.else_elements);
        let dsz = self.diamond_size_for_text(&block.condition);

        // Top diamond
        let diamond_y = y + dsz;
        self.draw_diamond_sized(center_x, diamond_y, dsz);
        self.draw_text_near(center_x, diamond_y, &block.condition);

        let branch_top_y = y + dsz * 2.0 + ARROW_SPACING;

        // Then branch (left) — center at gap/2 + half-width left of diamond
        let then_center_x = center_x - BRANCH_GAP / 2.0 - then_box.width / 2.0;
        self.draw_angled_arrow(center_x, y + dsz * 2.0, then_center_x, branch_top_y);
        if !block.then_label.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: center_x - dsz - 3.0,
                y: y + dsz * 2.0 + 12.0,
                content: block.then_label.clone(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#666666".into(),
                anchor: TextAnchor::End,
            }));
        }
        let then_end_y = self.draw_elements(&block.then_elements, then_center_x, branch_top_y);

        // Else branch (right) — center at gap/2 + half-width right of diamond
        let else_center_x = center_x + BRANCH_GAP / 2.0 + else_box.width / 2.0;
        self.draw_angled_arrow(center_x, y + dsz * 2.0, else_center_x, branch_top_y);
        if !block.else_label.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: center_x + dsz + 3.0,
                y: y + dsz * 2.0 + 12.0,
                content: block.else_label.clone(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#666666".into(),
                anchor: TextAnchor::Start,
            }));
        }
        let else_end_y = self.draw_elements(&block.else_elements, else_center_x, branch_top_y);

        // Bottom merge diamond (small, no text)
        let msz = MERGE_DIAMOND_SIZE;
        let merge_y = y + total_box.height - msz * 2.0;
        self.draw_diamond_sized(center_x, merge_y + msz, msz);

        // Arrows from branches to merge
        self.draw_angled_arrow(then_center_x, then_end_y, center_x, merge_y);
        self.draw_angled_arrow(else_center_x, else_end_y, center_x, merge_y);

        y + total_box.height
    }

    fn draw_while(&mut self, block: &WhileBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_while(block);

        let dsz = self.diamond_size_for_text(&block.condition);

        // Top diamond
        let diamond_y = y + dsz;
        self.draw_diamond_sized(center_x, diamond_y, dsz);
        self.draw_text_near(center_x, diamond_y, &block.condition);

        // "is" label
        if !block.is_label.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: center_x + dsz + 5.0,
                y: diamond_y - 5.0,
                content: block.is_label.clone(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#666666".into(),
                anchor: TextAnchor::Start,
            }));
        }

        // Body
        let body_y = y + dsz * 2.0 + ARROW_SPACING;
        self.draw_down_arrow(center_x, y + dsz * 2.0, body_y, None);
        let body_end_y = self.draw_elements(&block.elements, center_x, body_y);

        // Loop-back arrow (left side)
        let loop_x = center_x - total_box.width / 2.0 - 10.0;
        let loop_top_y = y + dsz;
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{}",
                center_x, body_end_y,
                loop_x, body_end_y,
                loop_x, loop_top_y,
                center_x - dsz, loop_top_y,
            ),
            fill: "none".into(),
            stroke: self.theme.arrow_color().to_string(),
            stroke_width: 1.0,
            dashed: false,
        }));
        // Arrowhead pointing right
        self.primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (center_x - dsz, loop_top_y),
                (center_x - dsz - 6.0, loop_top_y - 3.0),
                (center_x - dsz - 6.0, loop_top_y + 3.0),
            ],
            fill: self.theme.arrow_color().to_string(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        // "end" label
        if !block.end_label.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: center_x - dsz - 5.0,
                y: diamond_y + dsz + 12.0,
                content: block.end_label.clone(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#666666".into(),
                anchor: TextAnchor::End,
            }));
        }

        y + total_box.height
    }

    fn draw_fork(&mut self, block: &ForkBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_fork(block);
        let branch_boxes: Vec<SubtreeBox> = block
            .branches
            .iter()
            .map(|b| self.measure_elements(b))
            .collect();

        let total_branches_width = total_box.width;
        let left_x = center_x - total_branches_width / 2.0;

        // Top fork bar
        self.primitives.push(Primitive::Rect(Rect {
            x: left_x - FORK_BAR_EXTEND,
            y,
            width: total_branches_width + FORK_BAR_EXTEND * 2.0,
            height: FORK_BAR_HEIGHT,
            fill: "#333333".into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            rx: 0.0,
            ry: 0.0,
        }));

        let branch_top_y = y + FORK_BAR_HEIGHT + ARROW_SPACING;
        let mut bx = left_x;
        for (branch, bbox) in block.branches.iter().zip(&branch_boxes) {
            let branch_cx = bx + bbox.width / 2.0;
            self.draw_down_arrow(branch_cx, y + FORK_BAR_HEIGHT, branch_top_y, None);
            let end_y = self.draw_elements(branch, branch_cx, branch_top_y);

            // Arrow down to bottom bar
            let bottom_bar_y = y + total_box.height - FORK_BAR_HEIGHT;
            self.draw_down_arrow(branch_cx, end_y, bottom_bar_y, None);

            bx += bbox.width + BRANCH_GAP;
        }

        // Bottom join bar
        let bottom_bar_y = y + total_box.height - FORK_BAR_HEIGHT;
        self.primitives.push(Primitive::Rect(Rect {
            x: left_x - FORK_BAR_EXTEND,
            y: bottom_bar_y,
            width: total_branches_width + FORK_BAR_EXTEND * 2.0,
            height: FORK_BAR_HEIGHT,
            fill: "#333333".into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            rx: 0.0,
            ry: 0.0,
        }));

        y + total_box.height
    }

    fn draw_switch(&mut self, block: &SwitchBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_switch(block);
        let case_boxes: Vec<SubtreeBox> = block
            .cases
            .iter()
            .map(|c| self.measure_elements(&c.elements))
            .collect();

        let dsz = self.diamond_size_for_text(&block.condition);

        // Top diamond
        let diamond_y = y + dsz;
        self.draw_diamond_sized(center_x, diamond_y, dsz);
        self.draw_text_near(center_x, diamond_y, &block.condition);

        let total_cases_width = total_box.width;
        let left_x = center_x - total_cases_width / 2.0;
        let branch_top_y = y + dsz * 2.0 + ARROW_SPACING;

        let mut cx = left_x;
        for (case, cbox) in block.cases.iter().zip(&case_boxes) {
            let case_w = cbox.width.max(60.0);
            let case_cx = cx + case_w / 2.0;

            self.draw_angled_arrow(center_x, y + dsz * 2.0, case_cx, branch_top_y);

            if !case.label.is_empty() {
                self.primitives.push(Primitive::Text(Text {
                    x: case_cx,
                    y: branch_top_y - 5.0,
                    content: case.label.clone(),
                    font_size: self.theme.font_size() - 1.0,
                    font_family: self.theme.font_family().to_string(),
                    fill: "#666666".into(),
                    anchor: TextAnchor::Middle,
                }));
            }

            let end_y = self.draw_elements(&case.elements, case_cx, branch_top_y);

            let merge_y = y + total_box.height - MERGE_DIAMOND_SIZE * 2.0;
            self.draw_angled_arrow(case_cx, end_y, center_x, merge_y);

            cx += case_w + BRANCH_GAP;
        }

        // Bottom merge diamond (small)
        let merge_y = y + total_box.height - MERGE_DIAMOND_SIZE;
        self.draw_diamond_sized(center_x, merge_y, MERGE_DIAMOND_SIZE);

        y + total_box.height
    }

    fn draw_partition(&mut self, partition: &Partition, center_x: f32, y: f32) -> f32 {
        let inner = self.measure_elements(&partition.elements);
        let label_w = self.measurer.measure_width(&partition.name) + PARTITION_PADDING * 2.0;
        let w = inner.width.max(label_w) + PARTITION_PADDING * 2.0;
        let label_h = self.measurer.line_height();
        let h = inner.height + PARTITION_PADDING * 2.0 + label_h;

        // Partition frame
        self.primitives.push(Primitive::Rect(Rect {
            x: center_x - w / 2.0,
            y,
            width: w,
            height: h,
            fill: "none".into(),
            stroke: "#333333".into(),
            stroke_width: 1.0,
            rx: 0.0,
            ry: 0.0,
        }));

        // Partition label
        self.primitives.push(Primitive::Text(Text {
            x: center_x,
            y: y + label_h * 0.8,
            content: partition.name.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "#333333".into(),
            anchor: TextAnchor::Middle,
        }));

        let inner_y = y + label_h + PARTITION_PADDING;
        self.draw_elements(&partition.elements, center_x, inner_y);

        y + h
    }

    fn draw_note(&mut self, note: &ActivityNote, center_x: f32, y: f32) {
        let text_w = self.measurer.measure_multiline_width(&note.text);
        let text_h = self.measurer.measure_multiline_height(&note.text);
        let w = text_w + NOTE_PADDING * 2.0;
        let h = text_h + NOTE_PADDING * 2.0;

        let x = match note.position {
            ActivityNotePosition::Right => center_x + NOTE_MARGIN,
            ActivityNotePosition::Left => center_x - NOTE_MARGIN - w,
        };

        self.primitives.push(Primitive::Rect(Rect {
            x,
            y,
            width: w,
            height: h,
            fill: self.theme.note_bg_color().to_string(),
            stroke: self.theme.note_border_color().to_string(),
            stroke_width: 1.0,
            rx: 0.0,
            ry: 0.0,
        }));

        self.primitives.push(Primitive::Text(Text {
            x: x + NOTE_PADDING,
            y: y + NOTE_PADDING + self.measurer.line_height() * 0.7,
            content: note.text.clone(),
            font_size: self.theme.font_size(),
            font_family: self.theme.font_family().to_string(),
            fill: "black".into(),
            anchor: TextAnchor::Start,
        }));
    }

    // --- Drawing helpers ---

    fn draw_filled_circle(&mut self, cx: f32, cy: f32, r: f32) {
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} a {},{} 0 1,0 {},0 a {},{} 0 1,0 {},0",
                cx - r, cy, r, r, r * 2.0, r, r, -(r * 2.0),
            ),
            fill: "#333333".into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            dashed: false,
        }));
    }

    fn draw_stop_circle(&mut self, cx: f32, cy: f32, r: f32) {
        // Outer circle
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} a {},{} 0 1,0 {},0 a {},{} 0 1,0 {},0",
                cx - r, cy, r, r, r * 2.0, r, r, -(r * 2.0),
            ),
            fill: "none".into(),
            stroke: "#333333".into(),
            stroke_width: 2.0,
            dashed: false,
        }));
        // Inner filled circle
        let ir = r * 0.6;
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} a {},{} 0 1,0 {},0 a {},{} 0 1,0 {},0",
                cx - ir, cy, ir, ir, ir * 2.0, ir, ir, -(ir * 2.0),
            ),
            fill: "#333333".into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            dashed: false,
        }));
    }

    #[allow(dead_code)]
    fn draw_diamond(&mut self, cx: f32, cy: f32) {
        self.draw_diamond_sized(cx, cy, DIAMOND_SIZE);
    }

    fn draw_diamond_sized(&mut self, cx: f32, cy: f32, d: f32) {
        self.primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (cx, cy - d),
                (cx + d, cy),
                (cx, cy + d),
                (cx - d, cy),
            ],
            fill: "#FEFECE".into(),
            stroke: "#A80036".into(),
            stroke_width: 1.5,
        }));
    }

    /// Calculate diamond size needed to fit the condition text inside.
    /// Diamond size for condition text. Compact like Java PlantUML —
    /// the diamond is a visual marker, text is rendered centered on it
    /// but may extend beyond the diamond edges.
    fn diamond_size_for_text(&self, text: &str) -> f32 {
        if text.is_empty() {
            return MERGE_DIAMOND_SIZE;
        }
        let text_w = self.measurer.measure_width(text);
        // Compact: diamond is about 40% of text width, clamped
        (text_w * 0.4 + 4.0).clamp(MIN_DIAMOND_SIZE, 30.0)
    }

    fn draw_text_near(&mut self, cx: f32, cy: f32, text: &str) {
        if text.is_empty() {
            return;
        }
        self.primitives.push(Primitive::Text(Text {
            x: cx,
            y: cy + self.measurer.line_height() * 0.3,
            content: text.to_string(),
            font_size: self.theme.font_size() - 1.0,
            font_family: self.theme.font_family().to_string(),
            fill: "#333333".into(),
            anchor: TextAnchor::Middle,
        }));
    }

    fn draw_down_arrow(&mut self, x: f32, y1: f32, y2: f32, label: Option<&str>) {
        self.primitives.push(Primitive::Arrow(Arrow {
            x1: x,
            y1,
            x2: x,
            y2,
            stroke: self.theme.arrow_color().to_string(),
            stroke_width: 1.0,
            head: ArrowHeadStyle::Filled,
            dashed: false,
        }));

        if let Some(text) = label {
            self.primitives.push(Primitive::Text(Text {
                x: x + 5.0,
                y: (y1 + y2) / 2.0 + 4.0,
                content: text.to_string(),
                font_size: self.theme.font_size() - 1.0,
                font_family: self.theme.font_family().to_string(),
                fill: "#666666".into(),
                anchor: TextAnchor::Start,
            }));
        }
    }

    fn draw_angled_arrow(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        // Draw an L-shaped or straight arrow
        if (x1 - x2).abs() < 1.0 {
            self.draw_down_arrow(x1, y1, y2, None);
        } else {
            // L-shape: go horizontal then vertical
            self.primitives.push(Primitive::Path(Path {
                d: format!("M {},{} L {},{} L {},{}", x1, y1, x2, y1, x2, y2),
                fill: "none".into(),
                stroke: self.theme.arrow_color().to_string(),
                stroke_width: 1.0,
                dashed: false,
            }));
            // Arrowhead
            self.primitives.push(Primitive::Polygon(Polygon {
                points: vec![
                    (x2, y2),
                    (x2 - 4.0, y2 - 8.0),
                    (x2 + 4.0, y2 - 8.0),
                ],
                fill: self.theme.arrow_color().to_string(),
                stroke: "none".into(),
                stroke_width: 0.0,
            }));
        }
    }

    fn draw_detach(&mut self, cx: f32, y: f32) {
        self.primitives.push(Primitive::Text(Text {
            x: cx,
            y: y + 14.0,
            content: "X".to_string(),
            font_size: 16.0,
            font_family: self.theme.font_family().to_string(),
            fill: "#A80036".into(),
            anchor: TextAnchor::Middle,
        }));
    }

    fn find_x_bounds(&self) -> (f32, f32) {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        for prim in &self.primitives {
            match prim {
                Primitive::Rect(r) => {
                    min_x = min_x.min(r.x);
                    max_x = max_x.max(r.x + r.width);
                }
                Primitive::Text(t) => {
                    let w = self.measurer.measure_width(&t.content);
                    let left = match t.anchor {
                        TextAnchor::Start => t.x,
                        TextAnchor::Middle => t.x - w / 2.0,
                        TextAnchor::End => t.x - w,
                    };
                    min_x = min_x.min(left);
                    max_x = max_x.max(left + w);
                }
                Primitive::Arrow(a) => {
                    min_x = min_x.min(a.x1.min(a.x2));
                    max_x = max_x.max(a.x1.max(a.x2));
                }
                Primitive::Polygon(p) => {
                    for &(x, _) in &p.points {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
                Primitive::Path(_) => {
                    // Path bounds are complex; skip for now
                }
                Primitive::Line(l) => {
                    min_x = min_x.min(l.x1.min(l.x2));
                    max_x = max_x.max(l.x1.max(l.x2));
                }
                Primitive::DashedLine(dl) => {
                    min_x = min_x.min(dl.x1.min(dl.x2));
                    max_x = max_x.max(dl.x1.max(dl.x2));
                }
            }
        }
        (min_x, max_x)
    }

    fn shift_all_x(&mut self, dx: f32) {
        for prim in &mut self.primitives {
            match prim {
                Primitive::Rect(r) => r.x += dx,
                Primitive::Text(t) => t.x += dx,
                Primitive::Arrow(a) => { a.x1 += dx; a.x2 += dx; }
                Primitive::Polygon(p) => {
                    for pt in &mut p.points { pt.0 += dx; }
                }
                Primitive::Path(p) => {
                    // Shift path by rewriting M/L coordinates
                    p.d = shift_path_d(&p.d, dx);
                }
                Primitive::Line(l) => { l.x1 += dx; l.x2 += dx; }
                Primitive::DashedLine(dl) => { dl.x1 += dx; dl.x2 += dx; }
            }
        }
    }
}

fn shift_path_d(d: &str, dx: f32) -> String {
    let mut result = String::new();
    let mut chars = d.chars().peekable();

    while let Some(&ch) = chars.peek() {
        if ch == 'M' || ch == 'L' || ch == 'Z' {
            result.push(ch);
            chars.next();
            // Skip whitespace
            while let Some(&' ') = chars.peek() {
                result.push(' ');
                chars.next();
            }
            if ch == 'Z' { continue; }
            // Read x number
            let mut num_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() || c == '.' || c == '-' {
                    num_str.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Ok(x) = num_str.parse::<f32>() {
                result.push_str(&format!("{}", x + dx));
            } else {
                result.push_str(&num_str);
            }
        } else if ch == 'a' {
            // Arc command — relative, don't shift
            result.push(ch);
            chars.next();
        } else {
            result.push(ch);
            chars.next();
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::activity::parse;

    fn layout_from_text(input: &str) -> LaidOutDiagram {
        let diagram = parse(input).unwrap();
        layout(&diagram)
    }

    #[test]
    fn test_simple_activity() {
        let laid_out = layout_from_text("start\n:Hello;\nstop");
        assert!(laid_out.width > 0.0);
        assert!(laid_out.height > 0.0);
        assert!(!laid_out.primitives.is_empty());
    }

    #[test]
    fn test_activity_has_start_circle() {
        let laid_out = layout_from_text("start\n:Hello;\nstop");
        let has_filled_circle = laid_out.primitives.iter().any(|p| {
            matches!(p, Primitive::Path(path) if path.fill == "#333333" && path.d.contains('a'))
        });
        assert!(has_filled_circle, "expected start circle");
    }

    #[test]
    fn test_activity_has_action_rect() {
        let laid_out = layout_from_text("start\n:Hello;\nstop");
        let has_rounded_rect = laid_out.primitives.iter().any(|p| {
            matches!(p, Primitive::Rect(r) if r.rx > 0.0 && r.fill == "#FEFECE")
        });
        assert!(has_rounded_rect, "expected rounded action rect");
    }

    #[test]
    fn test_activity_with_if() {
        let laid_out = layout_from_text(
            "start\nif (ok?) then (yes)\n:A;\nelse (no)\n:B;\nendif\nstop",
        );
        // Should have diamonds for if/merge
        let diamond_count = laid_out.primitives.iter().filter(|p| {
            matches!(p, Primitive::Polygon(poly) if poly.points.len() == 4 && poly.fill == "#FEFECE")
        }).count();
        assert!(diamond_count >= 2, "expected at least 2 diamonds, got {}", diamond_count);
    }

    #[test]
    fn test_activity_with_while() {
        let laid_out = layout_from_text(
            "start\nwhile (running?) is (yes)\n:process;\nendwhile (done)\nstop",
        );
        assert!(laid_out.width > 0.0);
        assert!(laid_out.height > 0.0);
    }

    #[test]
    fn test_activity_with_fork() {
        let laid_out = layout_from_text(
            "start\nfork\n:task1;\nfork again\n:task2;\nend fork\nstop",
        );
        // Fork bars should be dark rects
        let bar_count = laid_out.primitives.iter().filter(|p| {
            matches!(p, Primitive::Rect(r) if r.fill == "#333333")
        }).count();
        assert_eq!(bar_count, 2, "expected 2 fork bars");
    }

    #[test]
    fn test_activity_renders_to_svg() {
        let input = r#"start
:Initialize;
if (Valid?) then (yes)
  :Process;
else (no)
  :Error;
endif
stop"#;
        let laid_out = layout_from_text(input);
        let svg = crate::render::svg::render(&laid_out);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("Initialize"));
        assert!(svg.contains("Process"));
        assert!(svg.contains("Error"));
    }

    // Regression: nested if branches must not extend past the left edge of the viewBox.
    // The center_x calculation must account for asymmetric branch widths.
    #[test]
    fn test_nested_if_does_not_clip_left() {
        let input = r#"start
:Outer;
if (A?) then (yes)
  :Left1;
  if (B?) then (yes)
    :Inner Left;
  else (no)
    :Inner Right;
  endif
else (no)
  :Right1;
endif
stop"#;
        let laid_out = layout_from_text(input);

        // All rects and text must have x >= 0
        for prim in &laid_out.primitives {
            match prim {
                Primitive::Rect(r) => {
                    assert!(r.x >= 0.0, "rect clipped at x={}", r.x);
                }
                Primitive::Text(t) => {
                    // Text with anchor=Start must have x >= 0
                    if matches!(t.anchor, TextAnchor::Start) {
                        assert!(t.x >= 0.0, "text '{}' clipped at x={}", t.content, t.x);
                    }
                }
                _ => {}
            }
        }
    }
}
