/// Parser for Gantt charts (`@startgantt`).
///
/// Supported statements:
/// - `Project starts 2026-08-03`
/// - `saturday are closed` (any weekday; `is closed` also accepted)
/// - `[Task] starts 2026-08-03 and lasts 5 days`
/// - `[Task] starts at [Other]'s end and lasts 10 days`
/// - `[Task] lasts 3 days` (starts at the project start)
/// - `[Task] on {Alice} lasts 3 days` (resource assignment)
/// - `[Task] is colored in Lavender/LightBlue` (fill or fill/border)
/// - `[M] happens at 2026-08-10` / `[M] happens at [Task]'s end`
///
/// A `[Task]` line whose name matches an already declared task updates
/// that task instead of creating a new row.
use crate::ast::gantt::*;
use crate::error::PlantUmlError;

const WEEKDAY_KEYWORDS: [&str; 7] = [
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
    "sunday",
];

pub fn parse(body: &str) -> Result<GanttDiagram, PlantUmlError> {
    let mut start: Option<Date> = None;
    let mut tasks: Vec<Task> = Vec::new();
    let mut title = None;
    let mut closed_weekdays = [false; 7];

    for (line_no, raw) in body.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('\'') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("title ") {
            title = Some(rest.trim().to_string());
            continue;
        }
        if line.starts_with("Project starts ") || line.starts_with("project starts ") {
            let date_str = line["Project starts ".len()..].trim();
            start = Some(parse_date(date_str, line_no)?);
            continue;
        }
        if let Some(day) = parse_closed_statement(line) {
            closed_weekdays[day] = true;
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                continue;
            };
            let name = rest[..end].trim().to_string();
            let spec = rest[end + 1..].trim();
            // A repeated `[Name]` line refines the already declared task.
            if let Some(pos) = tasks.iter().position(|t| t.name == name) {
                let mut task = tasks[pos].clone();
                apply_task_spec(&mut task, spec, &tasks, start, line_no)?;
                tasks[pos] = task;
            } else {
                let mut task = Task {
                    name,
                    start_offset: 0,
                    duration: 1,
                    after: None,
                    color: None,
                    border_color: None,
                    resource: None,
                    is_milestone: false,
                };
                apply_task_spec(&mut task, spec, &tasks, start, line_no)?;
                tasks.push(task);
            }
            continue;
        }
        // Other statements are ignored.
    }

    let start = start.unwrap_or(Date {
        year: 2026,
        month: 1,
        day: 1,
    });

    // Resolve `after` chains into start offsets, expanding durations over
    // closed days: a task chained after `[X]` starts on the first open day
    // following X's (working-day) end, while a milestone at `[X]'s end`
    // sits on X's last working day.
    let mut diagram = GanttDiagram {
        title,
        start,
        tasks: Vec::with_capacity(tasks.len()),
        closed_weekdays,
    };
    for mut task in tasks {
        if let Some(after) = task.after.filter(|&i| i < diagram.tasks.len()) {
            let end = diagram.task_end_offset(&diagram.tasks[after]);
            task.start_offset = if task.is_milestone {
                end - 1
            } else {
                diagram.next_open_offset(end)
            };
        } else if !task.is_milestone {
            task.start_offset = diagram.next_open_offset(task.start_offset);
        }
        diagram.tasks.push(task);
    }

    Ok(diagram)
}

/// Parses `saturday are closed` / `sunday is closed`; returns the weekday
/// index (0 = Monday ... 6 = Sunday).
fn parse_closed_statement(line: &str) -> Option<usize> {
    let words: Vec<String> = line
        .split_whitespace()
        .map(|w| w.to_ascii_lowercase())
        .collect();
    if words.len() == 3 && (words[1] == "are" || words[1] == "is") && words[2] == "closed" {
        return WEEKDAY_KEYWORDS.iter().position(|&w| w == words[0]);
    }
    None
}

fn parse_date(s: &str, line_no: usize) -> Result<Date, PlantUmlError> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() == 3 {
        if let (Ok(y), Ok(m), Ok(d)) = (
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            if (1..=12).contains(&m) && (1..=31).contains(&d) {
                return Ok(Date {
                    year: y,
                    month: m,
                    day: d,
                });
            }
        }
    }
    Err(PlantUmlError::ParseError {
        line: line_no + 1,
        message: format!("invalid date: {}", s),
    })
}

/// Offset of `date` relative to the project start (0 when unknown).
fn date_offset(date: Date, project_start: Option<Date>) -> i64 {
    project_start
        .map(|ps| date.to_epoch_days() - ps.to_epoch_days())
        .unwrap_or(0)
}

/// Resolves a `[X]` reference used in `... at [X]'s end`.
fn task_ref(rest: &str, tasks: &[Task], line_no: usize) -> Result<Option<usize>, PlantUmlError> {
    let Some(end) = rest.find(']') else {
        return Ok(None);
    };
    let ref_name = rest[..end].trim();
    match tasks.iter().position(|t| t.name == ref_name) {
        Some(pos) => Ok(Some(pos)),
        None => Err(PlantUmlError::ParseError {
            line: line_no + 1,
            message: format!("unknown task reference: {}", ref_name),
        }),
    }
}

fn apply_task_spec(
    task: &mut Task,
    spec: &str,
    tasks: &[Task],
    project_start: Option<Date>,
    line_no: usize,
) -> Result<(), PlantUmlError> {
    // Colors: `is colored in Fill` or `is colored in Fill/Border`.
    if let Some(idx) = spec.find("is colored in ") {
        let rest = spec[idx + "is colored in ".len()..].trim();
        let token = rest.split_whitespace().next().unwrap_or("");
        let mut parts = token.splitn(2, '/');
        if let Some(fill) = parts.next().filter(|s| !s.is_empty()) {
            task.color = Some(fill.to_string());
        }
        if let Some(border) = parts.next().filter(|s| !s.is_empty()) {
            task.border_color = Some(border.to_string());
        }
        return Ok(());
    }

    // Resource: `on {Name}`.
    if let Some(idx) = spec.find("on {") {
        let rest = &spec[idx + "on {".len()..];
        if let Some(end) = rest.find('}') {
            task.resource = Some(rest[..end].trim().to_string());
        }
    }

    // Milestone: `happens at [X]'s end` or `happens [at] <date>`.
    if let Some(idx) = spec.find("happens ") {
        task.is_milestone = true;
        let rest = spec[idx + "happens ".len()..].trim();
        let rest = rest.strip_prefix("at ").unwrap_or(rest).trim();
        if let Some(refname) = rest.strip_prefix('[') {
            task.after = task_ref(refname, tasks, line_no)?;
        } else {
            let date_str: String = rest
                .chars()
                .take_while(|&c| c.is_ascii_digit() || c == '-')
                .collect();
            let date = parse_date(&date_str, line_no)?;
            task.start_offset = date_offset(date, project_start);
        }
        return Ok(());
    }

    // Duration: `lasts N day(s)`.
    if let Some(idx) = spec.find("lasts ") {
        let after_lasts = &spec[idx + "lasts ".len()..];
        let num: String = after_lasts
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        task.duration = num.parse::<i64>().unwrap_or(1).max(1);
    }

    // Start: `starts at [X]'s end` or `starts <date>`.
    if let Some(idx) = spec.find("starts at [") {
        let rest = &spec[idx + "starts at [".len()..];
        task.after = task_ref(rest, tasks, line_no)?;
    } else if let Some(idx) = spec.find("starts ") {
        let rest = spec[idx + "starts ".len()..].trim();
        let date_str: String = rest
            .chars()
            .take_while(|&c| c.is_ascii_digit() || c == '-')
            .collect();
        let date = parse_date(&date_str, line_no)?;
        task.start_offset = date_offset(date, project_start);
    }

    if let Some(color) = spec.split_whitespace().find(|t| t.starts_with('#')) {
        task.color = Some(color.to_string());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let d = parse(
            "Project starts 2026-08-03\n[Design] starts 2026-08-03 and lasts 5 days\n[Build] starts at [Design]'s end and lasts 10 days\n",
        )
        .unwrap();
        assert_eq!(d.start.day, 3);
        assert_eq!(d.tasks.len(), 2);
        assert_eq!(d.tasks[0].start_offset, 0);
        assert_eq!(d.tasks[0].duration, 5);
        assert_eq!(d.tasks[1].start_offset, 5);
        assert_eq!(d.tasks[1].duration, 10);
    }

    #[test]
    fn test_start_at_date_offset() {
        let d = parse(
            "Project starts 2026-08-03\n[Docs] starts 2026-08-10 and lasts 6 days\n",
        )
        .unwrap();
        assert_eq!(d.tasks[0].start_offset, 7);
    }

    #[test]
    fn test_weekday() {
        // 2026-08-03 is a Monday.
        let date = Date {
            year: 2026,
            month: 8,
            day: 3,
        };
        assert_eq!(date.weekday(), 0);
        // 2026-08-08 is a Saturday.
        assert_eq!(date.plus_days(5).weekday(), 5);
    }

    #[test]
    fn test_date_roundtrip() {
        let date = Date {
            year: 2026,
            month: 8,
            day: 31,
        };
        let back = Date::from_epoch_days(date.to_epoch_days());
        assert_eq!(date, back);
        assert_eq!(date.plus_days(1).month, 9);
    }

    #[test]
    fn test_closed_weekdays() {
        let d = parse(
            "Project starts 2026-08-03\nsaturday are closed\nsunday are closed\n[Design] lasts 5 days\n[Build] starts at [Design]'s end and lasts 10 days\n",
        )
        .unwrap();
        assert!(d.closed_weekdays[5] && d.closed_weekdays[6]);
        assert!(!d.closed_weekdays[0]);
        // Design: Mon 8/3 .. Fri 8/7 (5 working days, no closed inside).
        assert_eq!(d.task_end_offset(&d.tasks[0]), 5);
        // Build starts Mon 8/10 (offset 7), skipping the weekend.
        assert_eq!(d.tasks[1].start_offset, 7);
        // 10 working days from 8/10 span one weekend → ends after 8/21.
        assert_eq!(d.task_end_offset(&d.tasks[1]), 19);
    }

    #[test]
    fn test_milestones() {
        let d = parse(
            "Project starts 2026-08-03\nsaturday are closed\nsunday are closed\n[Design] lasts 5 days\n[Kickoff] happens at 2026-08-10\n[Release] happens at [Design]'s end\n",
        )
        .unwrap();
        assert!(d.tasks[1].is_milestone);
        assert_eq!(d.tasks[1].start_offset, 7);
        assert!(d.tasks[2].is_milestone);
        // Design's last working day is Fri 8/7 (offset 4).
        assert_eq!(d.tasks[2].start_offset, 4);
    }

    #[test]
    fn test_colors_and_resources() {
        let d = parse(
            "Project starts 2026-08-03\n[Prototype] on {Alice} lasts 7 days\n[Prototype] is colored in Lavender/LightBlue\n[Testing] on {Bob} lasts 5 days\n[Testing] starts at [Prototype]'s end\n[Testing] is colored in Coral\n",
        )
        .unwrap();
        assert_eq!(d.tasks.len(), 2);
        assert_eq!(d.tasks[0].color.as_deref(), Some("Lavender"));
        assert_eq!(d.tasks[0].border_color.as_deref(), Some("LightBlue"));
        assert_eq!(d.tasks[0].resource.as_deref(), Some("Alice"));
        assert_eq!(d.tasks[1].color.as_deref(), Some("Coral"));
        assert_eq!(d.tasks[1].border_color, None);
        assert_eq!(d.tasks[1].resource.as_deref(), Some("Bob"));
        assert_eq!(d.tasks[1].start_offset, 7);
    }
}
