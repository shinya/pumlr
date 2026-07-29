use crate::ast::activity::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{DefaultTheme, Theme};

// --- Layout metrics (matched against PlantUML 1.2026 default style, see SPEC.md) ---

const DIAGRAM_MARGIN: f32 = 20.0;
const ACTION_PADDING_H: f32 = 10.0;
const ACTION_PADDING_V: f32 = 10.0;
const ACTION_RADIUS: f32 = 12.5;
const ARROW_SPACING: f32 = 20.0;
/// Condition hexagon: horizontal extension of the side points beyond the text box.
const HEX_EXTEND: f32 = 12.0;
/// Condition hexagon: half height.
const HEX_HALF_H: f32 = 12.0;
/// Merge diamond: half width/height.
const MERGE_HALF: f32 = 12.0;
/// Gap between the condition hexagon bottom and the top of a branch.
const BRANCH_TOP_GAP: f32 = 10.0;
/// Gap between a branch bottom and the top of the merge diamond.
const MERGE_TOP_GAP: f32 = 6.0;
const START_RADIUS: f32 = 10.0;
const STOP_OUTER_RADIUS: f32 = 11.0;
const STOP_INNER_RADIUS: f32 = 6.0;
const BRANCH_GAP: f32 = 30.0;
const NOTE_PADDING: f32 = 8.0;
const NOTE_MARGIN: f32 = 10.0;
/// Partition frame: name tab height, side padding, inner-top gap, bottom padding.
const PARTITION_TAB_H: f32 = 19.5;
const PARTITION_PADDING_H: f32 = 20.0;
const PARTITION_INNER_TOP: f32 = 17.0;
const PARTITION_PADDING_B: f32 = 12.0;
const PARTITION_FONT_SIZE: f32 = 14.0;
const TITLE_MARGIN: f32 = 10.0;
const FORK_BAR_HEIGHT: f32 = 6.0;
const FORK_BAR_EXTEND: f32 = 20.0;
/// While loop: rails run this far outside the widest content.
const WHILE_RAIL_MARGIN: f32 = 12.0;
/// While loop: gap between hexagon bottom and body top (room for the is-label).
const WHILE_BODY_GAP: f32 = 32.0;
/// While loop: drop below the body before the loop-back rail turns.
const WHILE_LOOP_DROP: f32 = 10.0;
/// While loop: drop from the loop-back turn to the exit junction.
const WHILE_EXIT_DROP: f32 = 12.0;
/// If/elseif chain: gap between branch columns.
const CHAIN_COL_GAP: f32 = 15.0;
/// Repeat loop: gap between the body's right edge and the backward action.
const REPEAT_RAIL_GAP: f32 = 24.0;
/// If/elseif chain: gap between hexagon bottom and branch tops.
const CHAIN_BRANCH_GAP: f32 = 33.0;

// --- Text metrics (colors live in the Theme, see src/theme) ---

const FONT_FAMILY: &str = "sans-serif";
const FONT_SIZE: f32 = 12.0;
const LABEL_FONT_SIZE: f32 = 11.0;
const TITLE_FONT_SIZE: f32 = 14.0;

/// Layout a parsed activity diagram into primitives with the default theme.
pub fn layout(diagram: &ActivityDiagram) -> LaidOutDiagram {
    layout_with_theme(diagram, &DefaultTheme)
}

/// Layout a parsed activity diagram into primitives with the given theme.
pub fn layout_with_theme(diagram: &ActivityDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let measurer = TextMeasurer::new(FONT_SIZE);
    let label_measurer = TextMeasurer::new(LABEL_FONT_SIZE);

    let mut ctx = ActivityLayoutContext::new(theme, &measurer, &label_measurer);
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
    label_measurer: &'a TextMeasurer,
    primitives: Vec<Primitive>,
}

impl<'a> ActivityLayoutContext<'a> {
    fn new(
        theme: &'a dyn Theme,
        measurer: &'a TextMeasurer,
        label_measurer: &'a TextMeasurer,
    ) -> Self {
        Self {
            theme,
            measurer,
            label_measurer,
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

        let title_index = diagram.title.as_ref().map(|_| self.primitives.len());
        if let Some(title) = &diagram.title {
            self.primitives.push(Primitive::Text(Text {
                x: start_x, // re-centered on the final diagram width below
                y: y + self.measurer.line_height(),
                content: title.clone(),
                font_size: TITLE_FONT_SIZE,
                font_family: FONT_FAMILY.into(),
                fill: self.theme.activity_text_color().into(),
                anchor: TextAnchor::Middle,
                bold: true,
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

        let actual_width =
            (max_x + shift + DIAGRAM_MARGIN).max(subtree.width + DIAGRAM_MARGIN * 2.0);

        // Center the title on the final diagram width (like PlantUML does).
        if let Some(i) = title_index {
            if let Primitive::Text(t) = &mut self.primitives[i] {
                t.x = actual_width / 2.0;
            }
        }

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
            ActivityElement::Start => SubtreeBox {
                width: START_RADIUS * 2.0,
                height: START_RADIUS * 2.0,
                center_x: START_RADIUS,
            },
            ActivityElement::Stop | ActivityElement::End => SubtreeBox {
                width: STOP_OUTER_RADIUS * 2.0,
                height: STOP_OUTER_RADIUS * 2.0,
                center_x: STOP_OUTER_RADIUS,
            },
            ActivityElement::Detach => SubtreeBox {
                width: 30.0,
                height: 20.0,
                center_x: 15.0,
            },
            ActivityElement::Action(action) => {
                let text_w = self.measurer.measure_multiline_width(&action.label);
                let text_h = self.measurer.measure_multiline_height(&action.label);
                let w = text_w + ACTION_PADDING_H * 2.0;
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
                let tab_w = self.partition_tab_width(&partition.name);
                let w = inner.width.max(tab_w) + PARTITION_PADDING_H * 2.0;
                let h = PARTITION_TAB_H + PARTITION_INNER_TOP + inner.height + PARTITION_PADDING_B;
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
            ActivityElement::Repeat(block) => self.measure_repeat(block),
            // An edge label contributes no height of its own: the surrounding
            // connector is drawn as one longer arrow with the label beside it.
            ActivityElement::Arrow(arrow) => {
                let w = self.measurer.measure_width(&arrow.label) + 10.0;
                SubtreeBox {
                    width: w,
                    height: 0.0,
                    center_x: 5.0,
                }
            }
        }
    }

    /// Full width of the condition hexagon for the given text.
    fn hex_width(&self, text: &str) -> f32 {
        self.label_measurer.measure_width(text) + HEX_EXTEND * 2.0
    }

    fn measure_if(&self, block: &IfBlock) -> SubtreeBox {
        if !block.elseif_blocks.is_empty() {
            return self.measure_if_chain(block);
        }
        let then_box = self.measure_elements(&block.then_elements);
        let else_box = self.measure_elements(&block.else_elements);

        let hex_w = self.hex_width(&block.condition);

        // Branches width = then + gap + else
        let branches_width = then_box.width + BRANCH_GAP + else_box.width;
        let max_branch_height = then_box.height.max(else_box.height);

        let total_width = branches_width.max(hex_w).max(MERGE_HALF * 2.0);
        let center_x = total_width / 2.0;
        let h = HEX_HALF_H * 2.0
            + BRANCH_TOP_GAP
            + max_branch_height
            + MERGE_TOP_GAP
            + MERGE_HALF * 2.0;
        SubtreeBox {
            width: total_width,
            height: h,
            center_x,
        }
    }

    /// Column layout for an if/elseif chain: one column per branch
    /// (then, each elseif, else). Returns (widths, center offsets from the
    /// block's left edge).
    fn chain_columns(&self, block: &IfBlock) -> (Vec<f32>, Vec<f32>) {
        let mut widths = Vec::new();
        widths.push(
            self.measure_elements(&block.then_elements)
                .width
                .max(self.hex_width(&block.condition)),
        );
        for elseif in &block.elseif_blocks {
            widths.push(
                self.measure_elements(&elseif.elements)
                    .width
                    .max(self.hex_width(&elseif.condition)),
            );
        }
        widths.push(self.measure_elements(&block.else_elements).width.max(20.0));

        let mut centers = Vec::new();
        let mut x = 0.0;
        for w in &widths {
            centers.push(x + w / 2.0);
            x += w + CHAIN_COL_GAP;
        }
        (widths, centers)
    }

    fn measure_if_chain(&self, block: &IfBlock) -> SubtreeBox {
        let (widths, centers) = self.chain_columns(block);
        let total_width: f32 =
            widths.iter().sum::<f32>() + CHAIN_COL_GAP * (widths.len() - 1) as f32;

        let mut max_branch_height = self
            .measure_elements(&block.then_elements)
            .height
            .max(self.measure_elements(&block.else_elements).height);
        for elseif in &block.elseif_blocks {
            max_branch_height =
                max_branch_height.max(self.measure_elements(&elseif.elements).height);
        }

        // Flow enters/leaves at the midpoint between the outer columns.
        let center_x = (centers[0] + centers[centers.len() - 1]) / 2.0;
        let h = HEX_HALF_H * 2.0 + CHAIN_BRANCH_GAP + max_branch_height + ARROW_SPACING;
        SubtreeBox {
            width: total_width,
            height: h,
            center_x,
        }
    }

    fn measure_while(&self, block: &WhileBlock) -> SubtreeBox {
        let hex_w = self.hex_width(&block.condition);
        let inner = self.measure_elements(&block.elements);
        // Rails run down both sides, 12px outside the widest content.
        let w = inner.width.max(hex_w) + WHILE_RAIL_MARGIN * 2.0;
        let h =
            HEX_HALF_H * 2.0 + WHILE_BODY_GAP + inner.height + WHILE_LOOP_DROP + WHILE_EXIT_DROP;
        SubtreeBox {
            width: w,
            height: h,
            center_x: w / 2.0,
        }
    }

    /// Size of the backward action box on a repeat loop's rail.
    fn backward_size(&self, label: &str) -> (f32, f32) {
        let w = self.measurer.measure_multiline_width(label) + ACTION_PADDING_H * 2.0;
        let h = self.measurer.measure_multiline_height(label) + ACTION_PADDING_V * 2.0;
        (w, h)
    }

    fn measure_repeat(&self, block: &RepeatBlock) -> SubtreeBox {
        let inner = self.measure_elements(&block.elements);
        let hex_w = self.hex_width(&block.condition);
        let content_half = inner.width.max(hex_w) / 2.0;

        // Rail to the right, with the backward action box on it (if any)
        let rail_extent = match &block.backward {
            Some(label) => {
                let (bw, _) = self.backward_size(label);
                content_half + REPEAT_RAIL_GAP + bw
            }
            None => content_half + WHILE_RAIL_MARGIN,
        };

        let h = MERGE_HALF * 2.0 + ARROW_SPACING + inner.height + ARROW_SPACING + HEX_HALF_H * 2.0;
        SubtreeBox {
            width: content_half + rail_extent,
            height: h,
            center_x: content_half,
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

    /// Case column centers relative to the block's left edge.
    fn switch_columns(&self, block: &SwitchBlock) -> Vec<f32> {
        let mut centers = Vec::new();
        let mut x = 0.0;
        for case in &block.cases {
            let w = self.measure_elements(&case.elements).width.max(60.0);
            centers.push(x + w / 2.0);
            x += w + BRANCH_GAP;
        }
        centers
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

        // Like PlantUML, the hexagon (and the flow) aligns with the middle
        // case column, not the geometric center of all columns.
        let centers = self.switch_columns(block);
        let center_x = if centers.is_empty() {
            total_width / 2.0
        } else if centers.len() % 2 == 1 {
            centers[centers.len() / 2]
        } else {
            (centers[centers.len() / 2 - 1] + centers[centers.len() / 2]) / 2.0
        };

        let h = HEX_HALF_H * 2.0 + CHAIN_BRANCH_GAP + max_height + MERGE_TOP_GAP + MERGE_HALF * 2.0;
        SubtreeBox {
            width: total_width.max(self.hex_width(&block.condition)),
            height: h,
            center_x,
        }
    }

    // --- Draw pass ---

    fn draw_elements(&mut self, elements: &[ActivityElement], center_x: f32, mut y: f32) -> f32 {
        let mut first = true;
        let mut pending_label: Option<String> = None;
        for element in elements {
            // `-> label;` labels the next connector instead of being drawn
            // as an element of its own.
            if let ActivityElement::Arrow(arrow) = element {
                pending_label = Some(arrow.label.clone());
                continue;
            }

            // Draw downward arrow from previous element (except before the
            // first). An if/elseif chain draws its own entry elbow, and a
            // partition is entered with a plain line (the arrowhead is drawn
            // inside, on the first inner element).
            if !first {
                // A labeled connector is one spacing longer to fit the label.
                let gap = if pending_label.is_some() {
                    ARROW_SPACING * 2.0
                } else {
                    ARROW_SPACING
                };
                let chain_entry =
                    matches!(element, ActivityElement::If(b) if !b.elseif_blocks.is_empty());
                if chain_entry {
                    // the chain draws its own elbow
                } else if matches!(element, ActivityElement::Partition(_)) {
                    self.primitives.push(Primitive::Line(Line {
                        x1: center_x,
                        y1: y,
                        x2: center_x,
                        y2: y + gap,
                        stroke: self.theme.activity_edge_color().into(),
                        stroke_width: 1.0,
                    }));
                } else {
                    self.draw_down_arrow(center_x, y, y + gap, pending_label.as_deref());
                }
                y += gap;
                pending_label = None;
            }

            y = self.draw_element(element, center_x, y);
            first = false;
        }
        y
    }

    fn draw_element(&mut self, element: &ActivityElement, center_x: f32, y: f32) -> f32 {
        match element {
            ActivityElement::Start => {
                self.draw_filled_circle(center_x, y + START_RADIUS, START_RADIUS);
                y + START_RADIUS * 2.0
            }
            ActivityElement::Stop | ActivityElement::End => {
                self.draw_stop_circle(center_x, y + STOP_OUTER_RADIUS);
                y + STOP_OUTER_RADIUS * 2.0
            }
            ActivityElement::Detach => {
                self.draw_detach(center_x, y);
                y + 20.0
            }
            ActivityElement::Action(action) => self.draw_action(action, center_x, y),
            ActivityElement::If(block) => self.draw_if(block, center_x, y),
            ActivityElement::While(block) => self.draw_while(block, center_x, y),
            ActivityElement::Fork(block) => self.draw_fork(block, center_x, y),
            ActivityElement::Switch(block) => self.draw_switch(block, center_x, y),
            ActivityElement::Repeat(block) => self.draw_repeat(block, center_x, y),
            ActivityElement::Partition(partition) => self.draw_partition(partition, center_x, y),
            ActivityElement::Note(note) => {
                self.draw_note(note, center_x, y);
                let sub = self.measure_element(element);
                y + sub.height
            }
            // Edge labels are folded into the connectors by draw_elements
            ActivityElement::Arrow(_) => y,
        }
    }

    fn draw_action(&mut self, action: &Action, center_x: f32, y: f32) -> f32 {
        let text_w = self.measurer.measure_multiline_width(&action.label);
        let text_h = self.measurer.measure_multiline_height(&action.label);
        let w = text_w + ACTION_PADDING_H * 2.0;
        let h = text_h + ACTION_PADDING_V * 2.0;

        self.primitives.push(Primitive::Rect(Rect {
            x: center_x - w / 2.0,
            y,
            width: w,
            height: h,
            fill: self.theme.activity_shape_fill().into(),
            stroke: self.theme.activity_shape_stroke().into(),
            stroke_width: self.theme.activity_shape_stroke_width(),
            rx: ACTION_RADIUS,
            ry: ACTION_RADIUS,
        }));

        // Vertically center the text block: place the first baseline so that
        // n lines (at 1.2em tspan spacing) sit around the rect middle.
        let line_count = action.label.lines().count().max(1) as f32;
        let tspan_step = FONT_SIZE * 1.2;
        let first_baseline = y + h / 2.0 - (line_count - 1.0) * tspan_step / 2.0 + FONT_SIZE * 0.35;
        self.primitives.push(Primitive::Text(Text {
            x: center_x,
            y: first_baseline,
            content: action.label.clone(),
            font_size: FONT_SIZE,
            font_family: FONT_FAMILY.into(),
            fill: self.theme.activity_text_color().into(),
            anchor: TextAnchor::Middle,
            bold: false,
        }));

        y + h
    }

    fn draw_if(&mut self, block: &IfBlock, center_x: f32, y: f32) -> f32 {
        if !block.elseif_blocks.is_empty() {
            return self.draw_if_chain(block, center_x, y);
        }
        let total_box = self.measure_if(block);
        let then_box = self.measure_elements(&block.then_elements);
        let else_box = self.measure_elements(&block.else_elements);

        // Condition hexagon
        let cy = y + HEX_HALF_H;
        self.draw_condition_hexagon(center_x, cy, &block.condition);
        let hex_half_w = self.hex_width(&block.condition) / 2.0;
        let hex_left = center_x - hex_half_w;
        let hex_right = center_x + hex_half_w;

        let branch_top = y + HEX_HALF_H * 2.0 + BRANCH_TOP_GAP;

        // Then branch (left)
        let then_cx = center_x - BRANCH_GAP / 2.0 - then_box.width / 2.0;
        self.draw_branch_edge_out(center_x, cy, hex_half_w, then_cx, branch_top);
        if !block.then_label.is_empty() {
            self.draw_side_label(hex_left, cy, &block.then_label, TextAnchor::End);
        }
        let then_end_y = self.draw_elements(&block.then_elements, then_cx, branch_top);

        // Else branch (right)
        let else_cx = center_x + BRANCH_GAP / 2.0 + else_box.width / 2.0;
        self.draw_branch_edge_out(center_x, cy, hex_half_w, else_cx, branch_top);
        if !block.else_label.is_empty() {
            self.draw_side_label(hex_right, cy, &block.else_label, TextAnchor::Start);
        }
        let else_end_y = self.draw_elements(&block.else_elements, else_cx, branch_top);

        // Bottom merge diamond (small, no text)
        let merge_cy = y + total_box.height - MERGE_HALF;
        self.draw_merge_diamond(center_x, merge_cy);

        // Edges from branches into the merge diamond
        self.draw_branch_edge_in(then_cx, then_end_y, center_x, merge_cy);
        self.draw_branch_edge_in(else_cx, else_end_y, center_x, merge_cy);

        y + total_box.height
    }

    /// If/elseif chain, wired like PlantUML: hexagons connected left to right,
    /// each with its branch column below; the flow enters via an elbow into
    /// the first hexagon and all branches merge on a horizontal junction line
    /// (no merge diamond).
    fn draw_if_chain(&mut self, block: &IfBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_if_chain(block);
        let (_, centers) = self.chain_columns(block);
        let left = center_x - total_box.center_x;
        let cols: Vec<f32> = centers.iter().map(|c| left + c).collect();
        let n_conds = cols.len() - 1; // last column is the else branch

        let cy = y + HEX_HALF_H;
        let hex_bottom = y + HEX_HALF_H * 2.0;
        let branch_top = hex_bottom + CHAIN_BRANCH_GAP;
        let junction_y = y + total_box.height;

        // Entry elbow: from the flow center above, over to the first hexagon
        let prev_bottom = y - ARROW_SPACING;
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{}",
                center_x,
                prev_bottom,
                center_x,
                prev_bottom + 5.0,
                cols[0],
                prev_bottom + 5.0,
                cols[0],
                y,
            ),
            fill: "none".into(),
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
            dashed: false,
        }));
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(cols[0], y, 0.0, 1.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        // Conditions with their branch columns
        let mut conds: Vec<(&str, &str, &[ActivityElement])> = vec![(
            block.condition.as_str(),
            block.then_label.as_str(),
            &block.then_elements[..],
        )];
        for elseif in &block.elseif_blocks {
            conds.push((
                elseif.condition.as_str(),
                elseif.then_label.as_str(),
                &elseif.elements[..],
            ));
        }

        let mut branch_ends = Vec::new();
        for (i, (condition, then_label, elements)) in conds.iter().enumerate() {
            let cx = cols[i];
            let hex_half_w = self.hex_width(condition) / 2.0;
            self.draw_condition_hexagon(cx, cy, condition);

            // then-label below the hexagon, next to the down edge
            if !then_label.is_empty() {
                self.primitives.push(Primitive::Text(Text {
                    x: cx + 4.0,
                    y: hex_bottom + 10.6,
                    content: then_label.to_string(),
                    font_size: LABEL_FONT_SIZE,
                    font_family: FONT_FAMILY.into(),
                    fill: self.theme.activity_text_color().into(),
                    anchor: TextAnchor::Start,
                    bold: false,
                }));
            }

            self.draw_down_arrow(cx, hex_bottom, branch_top, None);
            branch_ends.push(self.draw_elements(elements, cx, branch_top));

            let hex_right = cx + hex_half_w;
            if i + 1 < n_conds {
                // "no" edge into the next condition's left vertex
                let next_left = cols[i + 1] - self.hex_width(conds[i + 1].0) / 2.0;
                self.primitives.push(Primitive::Line(Line {
                    x1: hex_right,
                    y1: cy,
                    x2: next_left,
                    y2: cy,
                    stroke: self.theme.activity_edge_color().into(),
                    stroke_width: 1.0,
                }));
                self.primitives.push(Primitive::Polygon(Polygon {
                    points: concave_head(next_left, cy, 1.0, 0.0),
                    fill: self.theme.activity_edge_color().into(),
                    stroke: "none".into(),
                    stroke_width: 0.0,
                }));
            } else {
                // Final "no" edge over and down into the else branch
                let else_cx = cols[n_conds];
                if !block.else_label.is_empty() {
                    self.draw_side_label(hex_right, cy, &block.else_label, TextAnchor::Start);
                }
                self.primitives.push(Primitive::Path(Path {
                    d: format!(
                        "M {},{} L {},{} L {},{}",
                        hex_right, cy, else_cx, cy, else_cx, branch_top,
                    ),
                    fill: "none".into(),
                    stroke: self.theme.activity_edge_color().into(),
                    stroke_width: 1.0,
                    dashed: false,
                }));
                self.primitives.push(Primitive::Polygon(Polygon {
                    points: concave_head(else_cx, branch_top, 0.0, 1.0),
                    fill: self.theme.activity_edge_color().into(),
                    stroke: "none".into(),
                    stroke_width: 0.0,
                }));
            }
        }

        // Else branch
        branch_ends.push(self.draw_elements(&block.else_elements, cols[n_conds], branch_top));

        // Junction line all branches drop onto
        self.primitives.push(Primitive::Line(Line {
            x1: cols[0],
            y1: junction_y,
            x2: cols[n_conds],
            y2: junction_y,
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
        }));
        for (cx, end_y) in cols.iter().zip(&branch_ends) {
            self.draw_down_arrow(*cx, *end_y, junction_y, None);
        }

        junction_y
    }

    /// Repeat loop, wired like PlantUML: entry merge diamond on top, body,
    /// condition hexagon at the bottom; the loop-back rail runs up the right
    /// side (through the backward action, if any) into the diamond's right
    /// vertex. The flow exits from the hexagon's bottom vertex.
    fn draw_repeat(&mut self, block: &RepeatBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_repeat(block);
        let inner = self.measure_elements(&block.elements);
        let hex_w = self.hex_width(&block.condition);
        let content_half = inner.width.max(hex_w) / 2.0;

        // Entry merge diamond
        let diamond_cy = y + MERGE_HALF;
        self.draw_merge_diamond(center_x, diamond_cy);

        // Body
        let body_top = y + MERGE_HALF * 2.0 + ARROW_SPACING;
        self.draw_down_arrow(center_x, y + MERGE_HALF * 2.0, body_top, None);
        let body_end_y = self.draw_elements(&block.elements, center_x, body_top);

        // Condition hexagon at the bottom
        let hex_top = body_end_y + ARROW_SPACING;
        self.draw_down_arrow(center_x, body_end_y, hex_top, None);
        let hex_cy = hex_top + HEX_HALF_H;
        self.draw_condition_hexagon(center_x, hex_cy, &block.condition);
        let hex_right = center_x + hex_w / 2.0;

        // is-label ("yes") outside the right vertex
        if !block.is_label.is_empty() {
            self.draw_side_label(hex_right, hex_cy, &block.is_label, TextAnchor::Start);
        }

        // Loop-back rail on the right, up into the diamond's right vertex
        let rail_x = match &block.backward {
            Some(label) => {
                let (bw, _) = self.backward_size(label);
                center_x + content_half + REPEAT_RAIL_GAP + bw / 2.0
            }
            None => center_x + content_half + WHILE_RAIL_MARGIN,
        };

        if let Some(label) = &block.backward {
            let (_, bh) = self.backward_size(label);
            let backward_top = body_top + (inner.height - bh) / 2.0;
            let backward_bottom = backward_top + bh;

            // Up from the hexagon's right vertex into the backward action
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{}",
                    hex_right, hex_cy, rail_x, hex_cy, rail_x, backward_bottom,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
            self.primitives.push(Primitive::Polygon(Polygon {
                points: concave_head(rail_x, backward_bottom, 0.0, -1.0),
                fill: self.theme.activity_edge_color().into(),
                stroke: "none".into(),
                stroke_width: 0.0,
            }));

            self.draw_action(
                &Action {
                    label: label.clone(),
                    shape: ActionShape::Action,
                },
                rail_x,
                backward_top,
            );

            // From the backward action up into the diamond's right vertex
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{}",
                    rail_x,
                    backward_top,
                    rail_x,
                    diamond_cy,
                    center_x + MERGE_HALF,
                    diamond_cy,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
        } else {
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{} L {},{}",
                    hex_right,
                    hex_cy,
                    rail_x,
                    hex_cy,
                    rail_x,
                    diamond_cy,
                    center_x + MERGE_HALF,
                    diamond_cy,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
        }
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(center_x + MERGE_HALF, diamond_cy, -1.0, 0.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        y + total_box.height
    }

    /// While loop, wired like PlantUML: body below the hexagon, loop-back rail
    /// on the right (into the hexagon's right vertex), exit rail on the left
    /// (from the left vertex down to a junction the flow continues from).
    fn draw_while(&mut self, block: &WhileBlock, center_x: f32, y: f32) -> f32 {
        let total_box = self.measure_while(block);
        let inner = self.measure_elements(&block.elements);

        // Condition hexagon
        let cy = y + HEX_HALF_H;
        self.draw_condition_hexagon(center_x, cy, &block.condition);
        let hex_half_w = self.hex_width(&block.condition) / 2.0;
        let hex_left = center_x - hex_half_w;
        let hex_right = center_x + hex_half_w;

        // "is" label (loop-continue side, below the hexagon)
        if !block.is_label.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: center_x + 4.0,
                y: y + HEX_HALF_H * 2.0 + 10.6,
                content: block.is_label.clone(),
                font_size: LABEL_FONT_SIZE,
                font_family: FONT_FAMILY.into(),
                fill: self.theme.activity_text_color().into(),
                anchor: TextAnchor::Start,
                bold: false,
            }));
        }

        // Body
        let body_top = y + HEX_HALF_H * 2.0 + WHILE_BODY_GAP;
        self.draw_down_arrow(center_x, y + HEX_HALF_H * 2.0, body_top, None);
        let body_end_y = self.draw_elements(&block.elements, center_x, body_top);

        let content_half = inner.width.max(hex_half_w * 2.0) / 2.0;
        let rail_r = center_x + content_half + WHILE_RAIL_MARGIN;
        let rail_l = center_x - content_half - WHILE_RAIL_MARGIN;
        let loop_turn_y = body_end_y + WHILE_LOOP_DROP;
        let junction_y = loop_turn_y + WHILE_EXIT_DROP;

        // Loop-back: down from the body, right rail up, into the right vertex
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{} L {},{}",
                center_x,
                body_end_y,
                center_x,
                loop_turn_y,
                rail_r,
                loop_turn_y,
                rail_r,
                cy,
                hex_right,
                cy,
            ),
            fill: "none".into(),
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
            dashed: false,
        }));
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(rail_r, (cy + loop_turn_y) / 2.0, 0.0, -1.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(hex_right, cy, -1.0, 0.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        // Exit: from the left vertex, left rail down to the junction, back to
        // the flow center (the connector to the next element starts there)
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{}",
                hex_left, cy, rail_l, cy, rail_l, junction_y, center_x, junction_y,
            ),
            fill: "none".into(),
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
            dashed: false,
        }));
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(rail_l, (cy + junction_y) / 2.0, 0.0, 1.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));

        // "endwhile" label (loop-exit side)
        if !block.end_label.is_empty() {
            self.draw_side_label(hex_left, cy, &block.end_label, TextAnchor::End);
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
            fill: self.theme.activity_start_stop_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            rx: 2.5,
            ry: 2.5,
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
            fill: self.theme.activity_start_stop_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            rx: 2.5,
            ry: 2.5,
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

        // Condition hexagon
        let cy = y + HEX_HALF_H;
        self.draw_condition_hexagon(center_x, cy, &block.condition);
        let hex_half_w = self.hex_width(&block.condition) / 2.0;

        // Columns positioned so the middle one aligns with the hexagon center
        let left_x = center_x - total_box.center_x;
        let branch_top_y = y + HEX_HALF_H * 2.0 + CHAIN_BRANCH_GAP;

        let merge_cy = y + total_box.height - MERGE_HALF;

        let mut cx = left_x;
        for (case, cbox) in block.cases.iter().zip(&case_boxes) {
            let case_w = cbox.width.max(60.0);
            let case_cx = cx + case_w / 2.0;

            self.draw_branch_edge_out(center_x, cy, hex_half_w, case_cx, branch_top_y);

            // Case label sits beside the vertical entry edge, halfway down
            if !case.label.is_empty() {
                self.primitives.push(Primitive::Text(Text {
                    x: case_cx + 2.0,
                    y: (cy + branch_top_y) / 2.0 + 4.0,
                    content: case.label.clone(),
                    font_size: LABEL_FONT_SIZE,
                    font_family: FONT_FAMILY.into(),
                    fill: self.theme.activity_text_color().into(),
                    anchor: TextAnchor::Start,
                    bold: false,
                }));
            }

            let end_y = self.draw_elements(&case.elements, case_cx, branch_top_y);
            self.draw_branch_edge_in(case_cx, end_y, center_x, merge_cy);

            cx += case_w + BRANCH_GAP;
        }

        // Bottom merge diamond (small)
        self.draw_merge_diamond(center_x, merge_cy);

        y + total_box.height
    }

    /// Tab width for a partition label (14px text + side margins).
    fn partition_tab_width(&self, name: &str) -> f32 {
        self.measurer.measure_width(name) * (PARTITION_FONT_SIZE / FONT_SIZE) + 26.0
    }

    fn draw_partition(&mut self, partition: &Partition, center_x: f32, y: f32) -> f32 {
        let inner = self.measure_elements(&partition.elements);
        let tab_w = self.partition_tab_width(&partition.name);
        let w = inner.width.max(tab_w) + PARTITION_PADDING_H * 2.0;
        let h = PARTITION_TAB_H + PARTITION_INNER_TOP + inner.height + PARTITION_PADDING_B;
        let left = center_x - w / 2.0;

        // Partition frame
        self.primitives.push(Primitive::Rect(Rect {
            x: left,
            y,
            width: w,
            height: h,
            fill: "none".into(),
            stroke: self.theme.partition_border_color().into(),
            stroke_width: 1.5,
            rx: 0.0,
            ry: 0.0,
        }));

        // Name tab in the top-left corner (open path with a notched corner)
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{} L {},{}",
                left + tab_w,
                y,
                left + tab_w,
                y + PARTITION_TAB_H - 10.0,
                left + tab_w - 10.0,
                y + PARTITION_TAB_H,
                left,
                y + PARTITION_TAB_H,
            ),
            fill: "none".into(),
            stroke: self.theme.partition_border_color().into(),
            stroke_width: 1.5,
            dashed: false,
        }));

        // Partition label inside the tab
        self.primitives.push(Primitive::Text(Text {
            x: left + 3.0,
            y: y + 14.5,
            content: partition.name.clone(),
            font_size: PARTITION_FONT_SIZE,
            font_family: FONT_FAMILY.into(),
            fill: self.theme.activity_text_color().into(),
            anchor: TextAnchor::Start,
            bold: false,
        }));

        // Entry: the connector from outside stops at the frame; continue it
        // through the frame with the arrowhead on the first inner element.
        let inner_y = y + PARTITION_TAB_H + PARTITION_INNER_TOP;
        self.draw_down_arrow(center_x, y, inner_y, None);
        let inner_end = self.draw_elements(&partition.elements, center_x, inner_y);

        // Exit: plain line from the last inner element to the frame bottom,
        // where the outer connector picks up.
        self.primitives.push(Primitive::Line(Line {
            x1: center_x,
            y1: inner_end,
            x2: center_x,
            y2: y + h,
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
        }));

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
            fill: self.theme.activity_note_fill().into(),
            stroke: self.theme.activity_shape_stroke().into(),
            stroke_width: self.theme.activity_shape_stroke_width(),
            rx: 0.0,
            ry: 0.0,
        }));

        self.primitives.push(Primitive::Text(Text {
            x: x + NOTE_PADDING,
            y: y + NOTE_PADDING + self.measurer.line_height() * 0.7,
            content: note.text.clone(),
            font_size: FONT_SIZE,
            font_family: FONT_FAMILY.into(),
            fill: self.theme.activity_text_color().into(),
            anchor: TextAnchor::Start,
            bold: false,
        }));
    }

    // --- Drawing helpers ---

    fn draw_filled_circle(&mut self, cx: f32, cy: f32, r: f32) {
        self.primitives.push(Primitive::Path(Path {
            d: circle_path(cx, cy, r),
            fill: self.theme.activity_start_stop_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            dashed: false,
        }));
    }

    fn draw_stop_circle(&mut self, cx: f32, cy: f32) {
        // Outer circle
        self.primitives.push(Primitive::Path(Path {
            d: circle_path(cx, cy, STOP_OUTER_RADIUS),
            fill: "none".into(),
            stroke: self.theme.activity_start_stop_color().into(),
            stroke_width: 1.0,
            dashed: false,
        }));
        // Inner filled circle
        self.primitives.push(Primitive::Path(Path {
            d: circle_path(cx, cy, STOP_INNER_RADIUS),
            fill: self.theme.activity_start_stop_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
            dashed: false,
        }));
    }

    /// Condition hexagon centered at (cx, cy) with the condition text inside.
    fn draw_condition_hexagon(&mut self, cx: f32, cy: f32, text: &str) {
        let half_w = self.label_measurer.measure_width(text) / 2.0;
        self.primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (cx - half_w, cy - HEX_HALF_H),
                (cx + half_w, cy - HEX_HALF_H),
                (cx + half_w + HEX_EXTEND, cy),
                (cx + half_w, cy + HEX_HALF_H),
                (cx - half_w, cy + HEX_HALF_H),
                (cx - half_w - HEX_EXTEND, cy),
            ],
            fill: self.theme.activity_shape_fill().into(),
            stroke: self.theme.activity_shape_stroke().into(),
            stroke_width: self.theme.activity_shape_stroke_width(),
        }));

        if !text.is_empty() {
            self.primitives.push(Primitive::Text(Text {
                x: cx,
                y: cy + LABEL_FONT_SIZE * 0.38,
                content: text.to_string(),
                font_size: LABEL_FONT_SIZE,
                font_family: FONT_FAMILY.into(),
                fill: self.theme.activity_text_color().into(),
                anchor: TextAnchor::Middle,
                bold: false,
            }));
        }
    }

    /// Small merge diamond centered at (cx, cy).
    fn draw_merge_diamond(&mut self, cx: f32, cy: f32) {
        self.primitives.push(Primitive::Polygon(Polygon {
            points: vec![
                (cx, cy - MERGE_HALF),
                (cx + MERGE_HALF, cy),
                (cx, cy + MERGE_HALF),
                (cx - MERGE_HALF, cy),
            ],
            fill: self.theme.activity_shape_fill().into(),
            stroke: self.theme.activity_shape_stroke().into(),
            stroke_width: self.theme.activity_shape_stroke_width(),
        }));
    }

    /// yes/no label placed beside a hexagon side vertex, just above the branch edge.
    fn draw_side_label(&mut self, vertex_x: f32, cy: f32, text: &str, anchor: TextAnchor) {
        self.primitives.push(Primitive::Text(Text {
            x: vertex_x,
            y: cy - 2.3,
            content: text.to_string(),
            font_size: LABEL_FONT_SIZE,
            font_family: FONT_FAMILY.into(),
            fill: self.theme.activity_text_color().into(),
            anchor,
            bold: false,
        }));
    }

    fn draw_down_arrow(&mut self, x: f32, y1: f32, y2: f32, label: Option<&str>) {
        self.primitives.push(Primitive::Arrow(Arrow {
            x1: x,
            y1,
            x2: x,
            y2,
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
            head: ArrowHeadStyle::Filled,
            dashed: false,
        }));

        if let Some(text) = label {
            self.primitives.push(Primitive::Text(Text {
                x: x + 5.0,
                y: (y1 + y2) / 2.0 + 4.0,
                content: text.to_string(),
                font_size: LABEL_FONT_SIZE,
                font_family: FONT_FAMILY.into(),
                fill: self.theme.activity_text_color().into(),
                anchor: TextAnchor::Start,
                bold: false,
            }));
        }
    }

    /// Edge leaving a condition hexagon at (hex_cx, hex_cy) with half width
    /// `hex_half_w`, into the branch top at (target_x, target_y).
    ///
    /// Aligned branches leave from the bottom vertex; branches beyond the
    /// side vertices leave horizontally from them. Targets that fall inside
    /// the hexagon's width take a small elbow below the bottom vertex so the
    /// line never crosses the shape.
    fn draw_branch_edge_out(
        &mut self,
        hex_cx: f32,
        hex_cy: f32,
        hex_half_w: f32,
        target_x: f32,
        target_y: f32,
    ) {
        let dx = target_x - hex_cx;
        let hex_bottom = hex_cy + HEX_HALF_H;
        if dx.abs() < 0.5 {
            self.draw_down_arrow(hex_cx, hex_bottom, target_y, None);
            return;
        }
        if dx.abs() <= hex_half_w + 12.0 {
            let elbow_y = hex_bottom + 6.0;
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{} L {},{}",
                    hex_cx, hex_bottom, hex_cx, elbow_y, target_x, elbow_y, target_x, target_y,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
        } else {
            let vertex_x = if dx < 0.0 {
                hex_cx - hex_half_w
            } else {
                hex_cx + hex_half_w
            };
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{}",
                    vertex_x, hex_cy, target_x, hex_cy, target_x, target_y,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
        }
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(target_x, target_y, 0.0, 1.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));
    }

    /// Edge entering a merge diamond: vertical from the branch bottom (x1, y1),
    /// then into the nearest vertex. Sources close to the diamond's center
    /// take a small elbow into the top vertex so the line never crosses the
    /// shape; farther sources enter a side vertex horizontally.
    fn draw_branch_edge_in(&mut self, x1: f32, y1: f32, merge_cx: f32, merge_cy: f32) {
        let dx = x1 - merge_cx;
        let merge_top = merge_cy - MERGE_HALF;
        if dx.abs() < 0.5 {
            self.draw_down_arrow(x1, y1, merge_top, None);
            return;
        }
        if dx.abs() <= MERGE_HALF + 12.0 {
            let elbow_y = merge_top - 6.0;
            self.primitives.push(Primitive::Path(Path {
                d: format!(
                    "M {},{} L {},{} L {},{} L {},{}",
                    x1, y1, x1, elbow_y, merge_cx, elbow_y, merge_cx, merge_top,
                ),
                fill: "none".into(),
                stroke: self.theme.activity_edge_color().into(),
                stroke_width: 1.0,
                dashed: false,
            }));
            self.primitives.push(Primitive::Polygon(Polygon {
                points: concave_head(merge_cx, merge_top, 0.0, 1.0),
                fill: self.theme.activity_edge_color().into(),
                stroke: "none".into(),
                stroke_width: 0.0,
            }));
            return;
        }
        let (target_x, dir) = if dx < 0.0 {
            (merge_cx - MERGE_HALF, 1.0)
        } else {
            (merge_cx + MERGE_HALF, -1.0)
        };
        self.primitives.push(Primitive::Path(Path {
            d: format!(
                "M {},{} L {},{} L {},{}",
                x1, y1, x1, merge_cy, target_x, merge_cy
            ),
            fill: "none".into(),
            stroke: self.theme.activity_edge_color().into(),
            stroke_width: 1.0,
            dashed: false,
        }));
        self.primitives.push(Primitive::Polygon(Polygon {
            points: concave_head(target_x, merge_cy, dir, 0.0),
            fill: self.theme.activity_edge_color().into(),
            stroke: "none".into(),
            stroke_width: 0.0,
        }));
    }

    fn draw_detach(&mut self, cx: f32, y: f32) {
        self.primitives.push(Primitive::Text(Text {
            x: cx,
            y: y + 14.0,
            content: "X".to_string(),
            font_size: 16.0,
            font_family: FONT_FAMILY.into(),
            fill: self.theme.activity_edge_color().into(),
            anchor: TextAnchor::Middle,
            bold: false,
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
                Primitive::Arrow(a) => {
                    a.x1 += dx;
                    a.x2 += dx;
                }
                Primitive::Polygon(p) => {
                    for pt in &mut p.points {
                        pt.0 += dx;
                    }
                }
                Primitive::Path(p) => {
                    // Shift path by rewriting M/L coordinates
                    p.d = shift_path_d(&p.d, dx);
                }
                Primitive::Line(l) => {
                    l.x1 += dx;
                    l.x2 += dx;
                }
                Primitive::DashedLine(dl) => {
                    dl.x1 += dx;
                    dl.x2 += dx;
                }
            }
        }
    }
}

/// Circle as an SVG path (two arcs).
fn circle_path(cx: f32, cy: f32, r: f32) -> String {
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
            if ch == 'Z' {
                continue;
            }
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
            matches!(p, Primitive::Path(path) if path.fill == "#222222" && path.d.contains('a'))
        });
        assert!(has_filled_circle, "expected start circle");
    }

    #[test]
    fn test_activity_has_action_rect() {
        let laid_out = layout_from_text("start\n:Hello;\nstop");
        let has_rounded_rect = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Rect(r) if r.rx > 0.0 && r.fill == "#F1F1F1"));
        assert!(has_rounded_rect, "expected rounded action rect");
    }

    #[test]
    fn test_action_rect_uses_plantuml_radius() {
        let laid_out = layout_from_text("start\n:Hello;\nstop");
        let has_radius = laid_out
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Rect(r) if (r.rx - 12.5).abs() < 0.01));
        assert!(has_radius, "expected action rect with rx=12.5");
    }

    #[test]
    fn test_activity_with_if() {
        let laid_out =
            layout_from_text("start\nif (ok?) then (yes)\n:A;\nelse (no)\n:B;\nendif\nstop");
        // Condition hexagon (6 points) and merge diamond (4 points)
        let hexagon_count = laid_out.primitives.iter().filter(|p| {
            matches!(p, Primitive::Polygon(poly) if poly.points.len() == 6 && poly.fill == "#F1F1F1")
        }).count();
        let diamond_count = laid_out.primitives.iter().filter(|p| {
            matches!(p, Primitive::Polygon(poly) if poly.points.len() == 4 && poly.fill == "#F1F1F1")
        }).count();
        assert_eq!(hexagon_count, 1, "expected 1 condition hexagon");
        assert_eq!(diamond_count, 1, "expected 1 merge diamond");
    }

    #[test]
    fn test_activity_with_while() {
        let laid_out =
            layout_from_text("start\nwhile (running?) is (yes)\n:process;\nendwhile (done)\nstop");
        assert!(laid_out.width > 0.0);
        assert!(laid_out.height > 0.0);
    }

    #[test]
    fn test_activity_with_fork() {
        let laid_out =
            layout_from_text("start\nfork\n:task1;\nfork again\n:task2;\nend fork\nstop");
        // Fork bars should be dark rects
        let bar_count = laid_out
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Rect(r) if r.fill == "#222222"))
            .count();
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
