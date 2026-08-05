/// Integration tests for state diagram extensions:
/// concurrent regions (`--` separator) and shallow history (`[H]`).
use pumlr::render_svg;

#[test]
fn test_render_concurrent_regions() {
    let input = r#"@startuml
[*] --> Active

state Active {
  [*] --> NumLockOff
  NumLockOff --> NumLockOn : EvNumLockPressed
  --
  [*] --> CapsLockOff
  CapsLockOff --> CapsLockOn : EvCapsLockPressed
}

Active --> [*]
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    for name in ["Active", "NumLockOff", "NumLockOn", "CapsLockOff", "CapsLockOn"] {
        assert!(svg.contains(name), "missing {name}");
    }
    // Dashed region separator (PlantUML uses dasharray 8,10 at width 1.5).
    assert!(svg.contains(r#"stroke-dasharray="8,10""#));
    // Synthetic region names must never leak into the output.
    assert!(!svg.contains("Active$"));
}

#[test]
fn test_render_history_state() {
    let input = r#"@startuml
[*] --> Working

state Working {
  [*] --> Editing
  Editing --> Saving : save
}

Working --> Suspended : pause
Suspended --> Working[H] : resume
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    // History circle label.
    assert!(svg.contains(">H</text>"));
    // The raw endpoint text must not appear as a state.
    assert!(!svg.contains("Working[H]"));
    assert!(svg.contains("resume"));
}

#[test]
fn test_render_state_ext_fixtures() {
    for name in ["concurrent_state", "history_state"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}
