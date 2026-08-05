/// Integration tests for the Gantt chart extensions:
/// closed weekdays, milestones, task colors and resources.
use pumlr::render_svg;

const CLOSED_FIXTURE: &str = include_str!("fixtures/closed_gantt.puml");
const STYLED_FIXTURE: &str = include_str!("fixtures/styled_gantt.puml");

#[test]
fn test_render_closed_gantt_fixture() {
    let svg = render_svg(CLOSED_FIXTURE).unwrap();
    assert!(svg.starts_with("<svg"));
    // Weekend shading rects.
    assert!(svg.contains("#F1E5E5"));
    // Closed-day header labels are dimmed.
    assert!(svg.contains("#989898"));
    // Task and milestone labels.
    for label in ["Design", "Build", "Kickoff", "Release"] {
        assert!(svg.contains(label), "missing label: {}", label);
    }
    // The split Build bar bridges the weekend with dashed lines.
    assert!(svg.contains("stroke-dasharray=\"2,3\""));
}

#[test]
fn test_closed_gantt_working_day_geometry() {
    let svg = render_svg(CLOSED_FIXTURE).unwrap();
    // Build (10 working days) starts Mon 8/10 (x = 7*16 + 2 = 114), pauses
    // over the 8/15-16 weekend and resumes at 8/17 (x = 14*16 = 224).
    assert!(svg.contains(r#"x="114""#), "first segment at x=114");
    assert!(svg.contains(r#"x="224""#), "second segment at x=224");
    // First weekend shading spans 8/8-8/9: x = 5*16 = 80, width 32.
    assert!(svg.contains(r#"x="80""#));
    assert!(svg.contains(r#"width="32""#));
}

#[test]
fn test_closed_gantt_milestones() {
    let svg = render_svg(CLOSED_FIXTURE).unwrap();
    // Kickoff diamond on 8/10: centered at x = 7*16 + 8 = 120 on row 2.
    assert!(svg.contains("120,74.9102"), "Kickoff diamond top vertex");
    // Release diamond on Build's last working day, Fri 8/21 (x = 18*16 + 8).
    assert!(svg.contains("296,91.8652") || svg.contains("296,91.8653"), "Release diamond top vertex");
}

#[test]
fn test_render_styled_gantt_fixture() {
    let svg = render_svg(STYLED_FIXTURE).unwrap();
    // Fill/border pair Lavender/LightBlue and single color Coral
    // (named colors are emitted lowercased).
    assert!(svg.contains("lavender"));
    assert!(svg.contains("lightblue"));
    assert!(svg.contains("coral"));
    // Resource names are appended to the bar labels...
    assert!(svg.contains("Prototype {Alice}"));
    assert!(svg.contains("Testing {Bob}"));
    // ...and listed in the resource section with daily load cells.
    assert!(svg.contains(">Alice<"));
    assert!(svg.contains(">Bob<"));
    assert!(svg.contains(">100<"));
}

#[test]
fn test_gantt_without_extensions_unchanged() {
    // The plain fixture must not gain shading, dashes or resource rows.
    let svg = render_svg(include_str!("fixtures/simple_gantt.puml")).unwrap();
    assert!(!svg.contains("#F1E5E5"));
    assert!(!svg.contains("#989898"));
    assert!(!svg.contains("stroke-dasharray=\"2,3\""));
    assert!(!svg.contains(">100<"));
}
