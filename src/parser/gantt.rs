/// Parser for Gantt charts (`@startgantt`).
///
/// Supported statements:
/// - `Project starts 2026-08-03`
/// - `[Task] starts 2026-08-03 and lasts 5 days`
/// - `[Task] starts at [Other]'s end and lasts 10 days`
/// - `[Task] lasts 3 days` (starts at the project start)
use crate::ast::gantt::*;
use crate::error::PlantUmlError;

pub fn parse(body: &str) -> Result<GanttDiagram, PlantUmlError> {
    let mut start: Option<Date> = None;
    let mut tasks: Vec<Task> = Vec::new();
    let mut title = None;

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
        if let Some(rest) = line.strip_prefix('[') {
            let Some(end) = rest.find(']') else {
                continue;
            };
            let name = rest[..end].trim().to_string();
            let spec = rest[end + 1..].trim();
            let task = parse_task_spec(&name, spec, &tasks, start, line_no)?;
            tasks.push(task);
            continue;
        }
        // Other statements (colors, weekends, milestones…) are ignored.
    }

    let start = start.unwrap_or(Date {
        year: 2026,
        month: 1,
        day: 1,
    });

    // Resolve `after` chains into start offsets.
    let mut resolved: Vec<Task> = Vec::with_capacity(tasks.len());
    for mut task in tasks {
        if let Some(after) = task.after {
            let prev: &Task = &resolved[after];
            task.start_offset = prev.start_offset + prev.duration;
        }
        resolved.push(task);
    }

    Ok(GanttDiagram {
        title,
        start,
        tasks: resolved,
    })
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

fn parse_task_spec(
    name: &str,
    spec: &str,
    tasks: &[Task],
    project_start: Option<Date>,
    line_no: usize,
) -> Result<Task, PlantUmlError> {
    let mut task = Task {
        name: name.to_string(),
        start_offset: 0,
        duration: 1,
        after: None,
        color: None,
    };

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
        if let Some(end) = rest.find(']') {
            let ref_name = rest[..end].trim();
            let Some(pos) = tasks.iter().position(|t| t.name == ref_name) else {
                return Err(PlantUmlError::ParseError {
                    line: line_no + 1,
                    message: format!("unknown task reference: {}", ref_name),
                });
            };
            task.after = Some(pos);
        }
    } else if let Some(idx) = spec.find("starts ") {
        let rest = spec[idx + "starts ".len()..].trim();
        let date_str: String = rest
            .chars()
            .take_while(|&c| c.is_ascii_digit() || c == '-')
            .collect();
        let date = parse_date(&date_str, line_no)?;
        if let Some(ps) = project_start {
            task.start_offset = date.to_epoch_days() - ps.to_epoch_days();
        }
    }

    if let Some(color) = spec.split_whitespace().find(|t| t.starts_with('#')) {
        task.color = Some(color.to_string());
    }

    Ok(task)
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
}
