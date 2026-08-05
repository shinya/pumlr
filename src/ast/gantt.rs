/// AST for Gantt charts.

#[derive(Debug, Clone)]
pub struct GanttDiagram {
    pub title: Option<String>,
    /// Project start date (year, month, day).
    pub start: Date,
    pub tasks: Vec<Task>,
    /// Closed weekdays (index 0 = Monday ... 6 = Sunday), from
    /// `saturday are closed` statements.
    pub closed_weekdays: [bool; 7],
}

impl GanttDiagram {
    /// True if the day at `offset` (days from the project start) is closed.
    /// If every weekday is closed the schedule would never advance, so the
    /// closure set is ignored in that degenerate case.
    pub fn is_closed_offset(&self, offset: i64) -> bool {
        self.closed_weekdays.iter().any(|c| !c)
            && self.closed_weekdays[self.start.plus_days(offset).weekday() as usize]
    }

    /// First open (non-closed) day at or after `offset`.
    pub fn next_open_offset(&self, mut offset: i64) -> i64 {
        while self.is_closed_offset(offset) {
            offset += 1;
        }
        offset
    }

    /// Exclusive calendar end offset of a task: `duration` counts working
    /// days, so closed days inside the span stretch the bar.
    pub fn task_end_offset(&self, task: &Task) -> i64 {
        if task.is_milestone {
            return task.start_offset + 1;
        }
        let mut day = task.start_offset;
        let mut remaining = task.duration.max(1);
        loop {
            if !self.is_closed_offset(day) {
                remaining -= 1;
                if remaining == 0 {
                    return day + 1;
                }
            }
            day += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// Days since civil epoch (1970-01-01), Howard Hinnant's algorithm.
    pub fn to_epoch_days(self) -> i64 {
        let y = if self.month <= 2 {
            self.year - 1
        } else {
            self.year
        } as i64;
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let m = self.month as i64;
        let d = self.day as i64;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    pub fn from_epoch_days(days: i64) -> Self {
        let z = days + 719468;
        let era = if z >= 0 { z } else { z - 146096 } / 146097;
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
        Date {
            year: (if m <= 2 { y + 1 } else { y }) as i32,
            month: m,
            day: d,
        }
    }

    /// 0 = Monday ... 6 = Sunday.
    pub fn weekday(self) -> u32 {
        ((self.to_epoch_days() + 3).rem_euclid(7)) as u32
    }

    pub fn plus_days(self, n: i64) -> Self {
        Self::from_epoch_days(self.to_epoch_days() + n)
    }
}

#[derive(Debug, Clone)]
pub struct Task {
    pub name: String,
    /// Day offset from the project start (inclusive).
    pub start_offset: i64,
    /// Duration in working days (>= 1).
    pub duration: i64,
    /// Index of the task this one starts after, if declared via
    /// `starts at [X]'s end` / `happens at [X]'s end`.
    pub after: Option<usize>,
    /// Bar fill color (`is colored in Fill/Border` or inline `#hex`).
    pub color: Option<String>,
    /// Bar border color (second color of `is colored in Fill/Border`).
    pub border_color: Option<String>,
    /// Assigned resource name, from `on {Name}`.
    pub resource: Option<String>,
    /// True for `[M] happens at ...` milestones (rendered as a diamond).
    pub is_milestone: bool,
}
