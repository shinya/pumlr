//! Integration tests for activity-diagram extensions: swimlane switches
//! inside if/while/repeat blocks.

use pumlr::render_svg;

#[test]
fn test_braswim_fixture_renders() {
    let input = std::fs::read_to_string("tests/fixtures/braswim_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    // Lane headers
    assert!(svg.contains("Customer"));
    assert!(svg.contains("Shop"));
    // Branch actions placed via lane switches inside the if block
    assert!(svg.contains("Prepare Item"));
    assert!(svg.contains("Notify Unavailable"));
    assert!(svg.contains("Pay"));
    // Loop body actions placed via lane switches inside the while block
    assert!(svg.contains("Restock"));
    assert!(svg.contains("Check Item"));
}

#[test]
fn test_swimlane_fixture_still_renders() {
    // Regression guard: top-level-only lane switches keep working
    let input = std::fs::read_to_string("tests/fixtures/swimlane_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Customer"));
    assert!(svg.contains("Shop"));
    assert!(svg.contains("Delivery"));
    assert!(svg.contains("Ship Package"));
}

#[test]
fn test_repeat_with_lane_switch_renders() {
    let input = r#"@startuml
|A|
start
repeat
  |B|
  :Fetch Page;
  |A|
  :Store Rows;
repeat while (more pages?) is (yes)
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Fetch Page"));
    assert!(svg.contains("Store Rows"));
    assert!(svg.contains("more pages?"));
}

#[test]
fn test_repeat_with_lane_switch_and_backward_renders() {
    let input = r#"@startuml
|A|
start
repeat
  |B|
  :Process;
  backward :Retry;
repeat while (failed?) is (yes)
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Process"));
    assert!(svg.contains("Retry"));
}

#[test]
fn test_lane_switch_in_one_branch_only() {
    let input = r#"@startuml
|A|
start
if (ok?) then (yes)
  :stay here;
else (no)
  |B|
  :handle elsewhere;
endif
|A|
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("stay here"));
    assert!(svg.contains("handle elsewhere"));
}

#[test]
fn test_elseif_chain_with_lane_switch_falls_back_gracefully() {
    // Elseif chains keep the regular chain layout (switches inside are
    // ignored) — this must render without panicking.
    let input = r#"@startuml
|A|
start
if (x?) then (1)
  |B|
  :one;
elseif (y?) then (2)
  :two;
else (3)
  :three;
endif
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("one"));
    assert!(svg.contains("two"));
    assert!(svg.contains("three"));
}

#[test]
fn test_nested_if_inside_while_with_lane_switch() {
    let input = r#"@startuml
|A|
start
while (running?) is (yes)
  if (batch?) then (yes)
    |B|
    :bulk write;
    |A|
  else (no)
    :single write;
  endif
endwhile (no)
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("bulk write"));
    assert!(svg.contains("single write"));
}
