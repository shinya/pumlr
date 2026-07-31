/// Layout for Gantt charts, matching the PlantUML 1.2026 default style
/// (measured values recorded in SPEC.md).
use crate::ast::gantt::*;
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
        .map(|t| t.start_offset + t.duration)
        .max()
        .unwrap_or(1);
    let num_days = (max_end - min_offset).max(1);
    let chart_w = num_days as f32 * DAY_W;

    let day_x = |offset: i64| (offset - min_offset) as f32 * DAY_W;

    let n_rows = diagram.tasks.len();
    let grid_bottom = GRID_TOP + 2.0 + n_rows as f32 * ROW_PITCH + 0.9;

    let font = theme.font_family().to_string();
    let text = |x: f32, y: f32, content: String, size: f32, bold: bool, anchor: TextAnchor| {
        Primitive::Text(Text {
            x,
            y,
            content,
            font_size: size,
            font_family: font.clone(),
            fill: "#000000".into(),
            anchor,
            bold,
            italic: false,
        })
    };

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
        prims.push(text(cx, 23.668, weekday.clone(), 10.0, false, TextAnchor::Middle));
        prims.push(text(cx, 35.668, day_num.clone(), 10.0, false, TextAnchor::Middle));
        prims.push(text(cx, footer_top + 9.7, weekday, 10.0, false, TextAnchor::Middle));
        prims.push(text(cx, footer_top + 23.7, day_num, 10.0, false, TextAnchor::Middle));
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
        let Some(after) = task.after else { continue };
        let prev = &diagram.tasks[after];
        let px = day_x(prev.start_offset + prev.duration) - 2.0 - 6.0;
        let py = bar_top(after) + BAR_H;
        let ty = bar_top(row) + BAR_H / 2.0;
        let tip_x = day_x(task.start_offset);
        prims.push(Primitive::Path(Path {
            d: format!("M{},{} L{},{} L{},{}", px, py, px, ty, tip_x - 4.0, ty),
            fill: "none".into(),
            stroke: "#181818".into(),
            stroke_width: 1.5,
            dashed: false,
        }));
        prims.push(Primitive::Polygon(Polygon {
            points: vec![
                (tip_x, ty),
                (tip_x - 8.0, ty - 4.0),
                (tip_x - 8.0, ty + 4.0),
            ],
            fill: "#181818".into(),
            stroke: "#181818".into(),
            stroke_width: 1.0,
        }));
    }

    // --- Task bars and labels.
    for (row, task) in diagram.tasks.iter().enumerate() {
        let x = day_x(task.start_offset) + 2.0;
        let w = task.duration as f32 * DAY_W - 4.0;
        let y = bar_top(row);
        let fill = task
            .color
            .as_deref()
            .map(resolve_color)
            .unwrap_or_else(|| BAR_FILL.to_string());
        prims.push(Primitive::Rect(Rect {
            x,
            y,
            width: w,
            height: BAR_H,
            fill,
            stroke: "#181818".into(),
            stroke_width: 1.0,
            rx: 0.0,
            ry: 0.0,
        }));
        prims.push(text(
            x + 4.0,
            y + BAR_LABEL_BASELINE,
            task.name.clone(),
            11.0,
            false,
            TextAnchor::Start,
        ));
    }

    let width = chart_w + 21.0;
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
}
