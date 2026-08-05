/// Layout for Gantt charts, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::gantt::*;
use crate::layout::text_measure::TextMeasurer;
use crate::render::primitives::*;
use crate::render::svg::LaidOutDiagram;
use crate::theme::{resolve_color, Theme};

const DAY_W: f32 = 16.0;
const GRID_TOP: f32 = 39.0;
const BAR_H: f32 = 12.9551;
const ROW_PITCH: f32 = 16.9551;
const BAR_LABEL_BASELINE: f32 = 10.6348;
const GRID_COLOR: &str = "#C0C0C0";
const BAR_FILL: &str = "#E2E2F0";
const BAR_STROKE: &str = "#181818";
const TEXT_COLOR: &str = "#000000";
// Closed-day shading and the dimmed header/footer labels on closed days.
const CLOSED_FILL: &str = "#F1E5E5";
const CLOSED_TEXT: &str = "#989898";
// Milestone diamond: 10x10, centered on the day column, flush with row top.
const MILESTONE_HALF: f32 = 5.0;
// Dashed bridge across a closed gap stops 3px short of the bar segments.
const GAP_DASH_INSET: f32 = 3.0;
const GAP_DASH_ARRAY: &str = "2,3";
// Resource section (measured): name in Serif 13 above a full-width rule,
// one Serif-9 "load" cell per occupied day, 32px per resource row.
const RES_FONT_FAMILY: &str = "Serif";
const RES_NAME_SIZE: f32 = 13.0;
const RES_CELL_SIZE: f32 = 9.0;
const RES_NAME_BASELINE: f32 = 14.1367;
const RES_LINE_OFFSET: f32 = 16.9487;
const RES_CELL_BASELINE: f32 = 26.4023;
const RES_ROW_PITCH: f32 = 32.0;
const RES_BOTTOM_PAD: f32 = 17.0513;

const MONTH_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAY_NAMES: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];

pub fn layout_with_theme(diagram: &GanttDiagram, theme: &dyn Theme) -> LaidOutDiagram {
    let mut prims: Vec<Primitive> = Vec::new();

    // Chart span: from the earliest start to the latest end (inclusive).
    let min_offset = diagram
        .tasks
        .iter()
        .map(|t| t.start_offset)
        .min()
        .unwrap_or(0)
        .min(0);
    let max_end = diagram
        .tasks
        .iter()
        .map(|t| diagram.task_end_offset(t))
        .max()
        .unwrap_or(1);
    let num_days = (max_end - min_offset).max(1);
    let chart_w = num_days as f32 * DAY_W;

    let day_x = |offset: i64| (offset - min_offset) as f32 * DAY_W;
    let is_closed = |offset: i64| diagram.is_closed_offset(offset);

    // Resources in order of first appearance.
    let mut resources: Vec<&str> = Vec::new();
    for task in &diagram.tasks {
        if let Some(res) = task.resource.as_deref() {
            if !resources.contains(&res) {
                resources.push(res);
            }
        }
    }

    let n_rows = diagram.tasks.len();
    let task_area_bottom = GRID_TOP + 2.0 + n_rows as f32 * ROW_PITCH;
    let grid_bottom = if resources.is_empty() {
        task_area_bottom + 0.9
    } else {
        // Rows of the resource section hang below the last task bar.
        let base = task_area_bottom - (ROW_PITCH - BAR_H);
        base + RES_LINE_OFFSET + (resources.len() - 1) as f32 * RES_ROW_PITCH + RES_BOTTOM_PAD
    };

    let font = theme.font_family().to_string();
    let text = |x: f32, y: f32, content: String, size: f32, bold: bool, anchor: TextAnchor| {
        Primitive::Text(Text {
            x,
            y,
            content,
            font_size: size,
            font_family: font.clone(),
            fill: TEXT_COLOR.into(),
            anchor,
            bold,
            italic: false,
            underline: false,
        })
    };

    // --- Closed-day shading (behind everything else): merged runs of
    // closed days spanning the full grid height.
    let mut run_start: Option<i64> = None;
    for i in 0..=num_days {
        let closed = i < num_days && is_closed(min_offset + i);
        match (run_start, closed) {
            (None, true) => run_start = Some(i),
            (Some(s), false) => {
                prims.push(Primitive::Rect(Rect {
                    x: day_x(min_offset + s),
                    y: GRID_TOP,
                    width: (i - s) as f32 * DAY_W,
                    height: grid_bottom - GRID_TOP,
                    fill: CLOSED_FILL.into(),
                    stroke: "none".into(),
                    stroke_width: 1.0,
                    rx: 0.0,
                    ry: 0.0,
                }));
                run_start = None;
            }
            _ => {}
        }
    }

    // --- Header & footer timeline.
    // Month spans: group consecutive days by (year, month).
    let mut spans: Vec<(String, f32, f32)> = Vec::new(); // (label, x0, x1)
    let mut span_start = 0i64;
    let mut current = diagram.start.plus_days(min_offset);
    for i in 0..=num_days {
        let date = diagram.start.plus_days(min_offset + i);
        if i == num_days || (date.month != current.month && i > 0) {
            let label = format!("{} {}", MONTH_NAMES[(current.month - 1) as usize], current.year);
            spans.push((label, day_x(min_offset + span_start), day_x(min_offset + i)));
            span_start = i;
            current = date;
        }
    }
    let footer_top = grid_bottom;
    for (label, x0, x1) in &spans {
        prims.push(text(
            (x0 + x1) / 2.0,
            11.6016,
            label.clone(),
            12.0,
            true,
            TextAnchor::Middle,
        ));
        prims.push(text(
            (x0 + x1) / 2.0,
            footer_top + 38.6,
            label.clone(),
            12.0,
            true,
            TextAnchor::Middle,
        ));
    }
    for i in 0..num_days {
        let date = diagram.start.plus_days(min_offset + i);
        let cx = day_x(min_offset + i) + DAY_W / 2.0;
        let weekday = WEEKDAY_NAMES[date.weekday() as usize].to_string();
        let day_num = date.day.to_string();
        // Closed days are labeled in gray in both the header and footer.
        let fill = if is_closed(min_offset + i) {
            CLOSED_TEXT
        } else {
            TEXT_COLOR
        };
        for (y, content) in [
            (23.668, weekday.clone()),
            (35.668, day_num.clone()),
            (footer_top + 9.7, weekday),
            (footer_top + 23.7, day_num),
        ] {
            prims.push(Primitive::Text(Text {
                x: cx,
                y,
                content,
                font_size: 10.0,
                font_family: font.clone(),
                fill: fill.into(),
                anchor: TextAnchor::Middle,
                bold: false,
                italic: false,
                underline: false,
            }));
        }
    }

    // --- Grid: vertical line at every day boundary, top and bottom rules.
    for i in 0..=num_days {
        let x = day_x(min_offset + i);
        prims.push(Primitive::Line(Line {
            x1: x,
            y1: GRID_TOP,
            x2: x,
            y2: grid_bottom,
            stroke: GRID_COLOR.into(),
            stroke_width: 1.0,
        }));
    }
    for y in [GRID_TOP, grid_bottom] {
        prims.push(Primitive::Line(Line {
            x1: 0.0,
            y1: y,
            x2: chart_w,
            y2: y,
            stroke: GRID_COLOR.into(),
            stroke_width: 1.0,
        }));
    }

    // --- Dependency arrows (drawn before bars so bars sit on top).
    let bar_top = |row: usize| GRID_TOP + 2.0 + row as f32 * ROW_PITCH;
    for (row, task) in diagram.tasks.iter().enumerate() {
        if task.is_milestone {
            continue;
        }
        let Some(after) = task.after else { continue };
        let prev = &diagram.tasks[after];
        let px = day_x(diagram.task_end_offset(prev)) - 2.0 - 6.0;
        let py = bar_top(after) + BAR_H;
        let ty = bar_top(row) + BAR_H / 2.0;
        let tip_x = day_x(task.start_offset);
        prims.push(Primitive::Path(Path {
            d: format!("M{},{} L{},{} L{},{}", px, py, px, ty, tip_x - 4.0, ty),
            fill: "none".into(),
            stroke: BAR_STROKE.into(),
            stroke_width: 1.5,
            dashed: false,
        }));
        prims.push(Primitive::Polygon(Polygon {
            points: vec![
                (tip_x, ty),
                (tip_x - 8.0, ty - 4.0),
                (tip_x - 8.0, ty + 4.0),
            ],
            fill: BAR_STROKE.into(),
            stroke: BAR_STROKE.into(),
            stroke_width: 1.0,
        }));
    }

    // --- Task bars, milestones and labels.
    let label_measurer = TextMeasurer::new(11.0);
    let mut right_extent = chart_w;
    for (row, task) in diagram.tasks.iter().enumerate() {
        let y = bar_top(row);
        let label = match task.resource.as_deref() {
            Some(res) => format!("{} {{{}}}", task.name, res),
            None => task.name.clone(),
        };
        let label_w = label_measurer.measure_width(&label);

        if task.is_milestone {
            // Diamond centered on the day column, flush with the row top;
            // label to the right of the column.
            let cx = day_x(task.start_offset) + DAY_W / 2.0;
            let cy = y + MILESTONE_HALF;
            prims.push(Primitive::Polygon(Polygon {
                points: vec![
                    (cx, y),
                    (cx + MILESTONE_HALF, cy),
                    (cx, y + 2.0 * MILESTONE_HALF),
                    (cx - MILESTONE_HALF, cy),
                ],
                fill: TEXT_COLOR.into(),
                stroke: TEXT_COLOR.into(),
                stroke_width: 1.0,
            }));
            let lx = cx + DAY_W / 2.0;
            prims.push(text(
                lx,
                y + BAR_LABEL_BASELINE,
                label,
                11.0,
                false,
                TextAnchor::Start,
            ));
            right_extent = right_extent.max(lx + label_w);
            continue;
        }

        let end = diagram.task_end_offset(task);
        let fill = task
            .color
            .as_deref()
            .map(resolve_color)
            .unwrap_or_else(|| BAR_FILL.to_string());
        // A single color also colors the border; the default border is dark.
        let stroke = task
            .border_color
            .as_deref()
            .or(task.color.as_deref())
            .map(resolve_color)
            .unwrap_or_else(|| BAR_STROKE.to_string());

        // Split the bar into contiguous runs of open days.
        let mut segments: Vec<(i64, i64)> = Vec::new(); // (start, end) exclusive
        let mut seg_start: Option<i64> = None;
        for d in task.start_offset..=end {
            let open = d < end && !is_closed(d);
            match (seg_start, open) {
                (None, true) => seg_start = Some(d),
                (Some(s), false) => {
                    segments.push((s, d));
                    seg_start = None;
                }
                _ => {}
            }
        }

        let bot = y + BAR_H;
        let last_i = segments.len().saturating_sub(1);
        for (i, &(s, e)) in segments.iter().enumerate() {
            let first = i == 0;
            let last = i == last_i;
            // Outer segment ends are inset 2px like a plain bar; cut ends
            // sit on the day boundary with no side border.
            let x0 = if first { day_x(s) + 2.0 } else { day_x(s) };
            let x1 = if last { day_x(e) - 2.0 } else { day_x(e) };
            if first && last {
                prims.push(Primitive::Rect(Rect {
                    x: x0,
                    y,
                    width: x1 - x0,
                    height: BAR_H,
                    fill: fill.clone(),
                    stroke: stroke.clone(),
                    stroke_width: 1.0,
                    rx: 0.0,
                    ry: 0.0,
                }));
            } else {
                // Fill overshoots a right-side cut by 1px (measured).
                let fx1 = if last { x1 } else { x1 + 1.0 };
                prims.push(Primitive::Rect(Rect {
                    x: x0,
                    y,
                    width: fx1 - x0,
                    height: BAR_H,
                    fill: fill.clone(),
                    stroke: "none".into(),
                    stroke_width: 1.0,
                    rx: 0.0,
                    ry: 0.0,
                }));
                let d = if first {
                    // Left cap: bottom, left and top edges (open right).
                    format!("M{},{} L{},{} L{},{} L{},{}", x1, bot, x0, bot, x0, y, x1, y)
                } else if last {
                    // Right cap: top, right and bottom edges (open left).
                    format!("M{},{} L{},{} L{},{} L{},{}", x0, y, x1, y, x1, bot, x0, bot)
                } else {
                    // Middle segment: top and bottom edges only.
                    format!("M{},{} L{},{} M{},{} L{},{}", x0, y, x1, y, x0, bot, x1, bot)
                };
                prims.push(Primitive::Path(Path {
                    d,
                    fill: "none".into(),
                    stroke: stroke.clone(),
                    stroke_width: 1.0,
                    dashed: false,
                }));
                // Dashed bridge over the closed gap to the next segment.
                if !last {
                    let gx0 = day_x(e) + GAP_DASH_INSET;
                    let gx1 = day_x(segments[i + 1].0) - GAP_DASH_INSET;
                    for gy in [y, bot] {
                        prims.push(Primitive::DashedLine(DashedLine {
                            x1: gx0,
                            y1: gy,
                            x2: gx1,
                            y2: gy,
                            stroke: stroke.clone(),
                            stroke_width: 1.0,
                            dash_array: GAP_DASH_ARRAY.into(),
                        }));
                    }
                }
            }
        }

        // Label: inside the bar when it fits (4px padding each side),
        // otherwise just right of the bar.
        let bar_x = day_x(task.start_offset) + 2.0;
        let bar_right = day_x(end) - 2.0;
        let lx = if label_w + 8.0 <= bar_right - bar_x {
            bar_x + 4.0
        } else {
            bar_right + 4.0
        };
        prims.push(text(
            lx,
            y + BAR_LABEL_BASELINE,
            label,
            11.0,
            false,
            TextAnchor::Start,
        ));
        right_extent = right_extent.max(lx + label_w);
    }

    // --- Resource section: one row per resource with its daily load.
    if !resources.is_empty() {
        let base = task_area_bottom - (ROW_PITCH - BAR_H);
        let cell_measurer = TextMeasurer::new(RES_CELL_SIZE);
        let res_text = |x: f32, y: f32, content: String, size: f32| {
            Primitive::Text(Text {
                x,
                y,
                content,
                font_size: size,
                font_family: RES_FONT_FAMILY.into(),
                fill: TEXT_COLOR.into(),
                anchor: TextAnchor::Start,
                bold: false,
                italic: false,
                underline: false,
            })
        };
        for (i, res) in resources.iter().enumerate() {
            let row_base = base + i as f32 * RES_ROW_PITCH;
            prims.push(res_text(
                0.0,
                row_base + RES_NAME_BASELINE,
                res.to_string(),
                RES_NAME_SIZE,
            ));
            prims.push(Primitive::Line(Line {
                x1: 0.0,
                y1: row_base + RES_LINE_OFFSET,
                x2: chart_w,
                y2: row_base + RES_LINE_OFFSET,
                stroke: TEXT_COLOR.into(),
                stroke_width: 1.0,
            }));
            // Daily load: 100% per assigned task on each day of its span.
            for d in min_offset..max_end {
                let load: i64 = diagram
                    .tasks
                    .iter()
                    .filter(|t| {
                        !t.is_milestone
                            && t.resource.as_deref() == Some(res)
                            && d >= t.start_offset
                            && d < diagram.task_end_offset(t)
                    })
                    .count() as i64
                    * 100;
                if load > 0 {
                    let content = load.to_string();
                    let w = cell_measurer.measure_width(&content);
                    prims.push(res_text(
                        day_x(d) + (DAY_W - w) / 2.0,
                        row_base + RES_CELL_BASELINE,
                        content,
                        RES_CELL_SIZE,
                    ));
                }
            }
        }
    }

    let width = right_extent + 21.0;
    let height = footer_top + 41.0;

    LaidOutDiagram {
        width,
        height,
        primitives: prims,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::gantt::parse;
    use crate::theme::DefaultTheme;

    fn fixture() -> GanttDiagram {
        parse(
            "Project starts 2026-08-03\n[Design] starts 2026-08-03 and lasts 5 days\n[Build] starts at [Design]'s end and lasts 10 days\n",
        )
        .unwrap()
    }

    #[test]
    fn test_bars() {
        let laid = layout_with_theme(&fixture(), &DefaultTheme);
        let bars: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(bars.len(), 2);
        // Design: day 0, 5 days → x=2, w=76.
        assert!((bars[0].x - 2.0).abs() < 0.01);
        assert!((bars[0].width - 76.0).abs() < 0.01);
        // Build starts at day 5 → x = 5*16+2 = 82.
        assert!((bars[1].x - 82.0).abs() < 0.01);
    }

    #[test]
    fn test_dependency_arrow() {
        let laid = layout_with_theme(&fixture(), &DefaultTheme);
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Polygon(_))));
    }

    #[test]
    fn test_day_columns() {
        let laid = layout_with_theme(&fixture(), &DefaultTheme);
        // 15 days → 16 vertical lines + 2 horizontal rules.
        let lines = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Line(_)))
            .count();
        assert_eq!(lines, 16 + 2);
        // Weekday labels present, including the mirrored footer.
        let mo_count = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Text(t) if t.content == "Mo"))
            .count();
        // 15 days starting Mon 8/3 contain Mondays 3, 10, 17 → 3 in the
        // header + 3 in the mirrored footer.
        assert_eq!(mo_count, 6);
    }

    fn closed_fixture() -> GanttDiagram {
        parse(
            "Project starts 2026-08-03\nsaturday are closed\nsunday are closed\n[Design] starts 2026-08-03 and lasts 5 days\n[Build] starts at [Design]'s end and lasts 10 days\n[Kickoff] happens at 2026-08-10\n",
        )
        .unwrap()
    }

    #[test]
    fn test_closed_shading() {
        let laid = layout_with_theme(&closed_fixture(), &DefaultTheme);
        let shades: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) if r.fill == CLOSED_FILL => Some(r),
                _ => None,
            })
            .collect();
        // Two weekends inside the 8/3..8/21 span, each merged into one rect.
        assert_eq!(shades.len(), 2);
        assert!((shades[0].x - 80.0).abs() < 0.01);
        assert!((shades[0].width - 32.0).abs() < 0.01);
        assert!((shades[1].x - 192.0).abs() < 0.01);
        // Closed header labels are gray.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "Sa" && t.fill == CLOSED_TEXT)));
    }

    #[test]
    fn test_working_day_split_bar() {
        let laid = layout_with_theme(&closed_fixture(), &DefaultTheme);
        // Build (10 working days from Mon 8/10) is split around the 8/15-16
        // weekend: segments at x=114 (w=79, fill overshoot) and x=224 (w=78).
        let seg_fills: Vec<&Rect> = laid
            .primitives
            .iter()
            .filter_map(|p| match p {
                Primitive::Rect(r) if r.fill == BAR_FILL && r.stroke == "none" => Some(r),
                _ => None,
            })
            .collect();
        assert_eq!(seg_fills.len(), 2);
        assert!((seg_fills[0].x - 114.0).abs() < 0.01);
        assert!((seg_fills[0].width - 79.0).abs() < 0.01);
        assert!((seg_fills[1].x - 224.0).abs() < 0.01);
        assert!((seg_fills[1].width - 78.0).abs() < 0.01);
        // Dashed bridge (top and bottom) across the closed gap.
        let dashes = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::DashedLine(_)))
            .count();
        assert_eq!(dashes, 2);
    }

    #[test]
    fn test_milestone_diamond() {
        let laid = layout_with_theme(&closed_fixture(), &DefaultTheme);
        // Kickoff on 8/10 (offset 7, row 2): diamond centered at (120, 79.91).
        let diamond = laid
            .primitives
            .iter()
            .find_map(|p| match p {
                Primitive::Polygon(poly) if poly.points.len() == 4 => Some(poly),
                _ => None,
            })
            .expect("milestone diamond");
        assert!((diamond.points[0].0 - 120.0).abs() < 0.01);
        assert!((diamond.points[0].1 - 74.9102).abs() < 0.01);
        assert!((diamond.points[1].0 - 125.0).abs() < 0.01);
    }

    #[test]
    fn test_colored_bar_and_resources() {
        let d = parse(
            "Project starts 2026-08-03\n[Prototype] on {Alice} lasts 7 days\n[Prototype] is colored in Lavender/LightBlue\n[Testing] on {Bob} lasts 5 days\n[Testing] starts at [Prototype]'s end\n[Testing] is colored in Coral\n",
        )
        .unwrap();
        let laid = layout_with_theme(&d, &DefaultTheme);
        // Two-color: fill Lavender, border LightBlue; one-color: both Coral.
        assert!(laid.primitives.iter().any(
            |p| matches!(p, Primitive::Rect(r) if r.fill == "lavender" && r.stroke == "lightblue")
        ));
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Rect(r) if r.fill == "coral" && r.stroke == "coral")));
        // Resource labels are appended to the bar text.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "Prototype {Alice}")));
        // Resource rows: names in Serif plus per-day 100% load cells.
        assert!(laid
            .primitives
            .iter()
            .any(|p| matches!(p, Primitive::Text(t) if t.content == "Alice" && t.font_family == RES_FONT_FAMILY)));
        let loads = laid
            .primitives
            .iter()
            .filter(|p| matches!(p, Primitive::Text(t) if t.content == "100"))
            .count();
        // Alice 7 days + Bob 5 days.
        assert_eq!(loads, 12);
    }
}
