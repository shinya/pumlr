use pumlr::{detect_diagram_type, render_svg, DiagramType, PlantUmlError};

#[test]
fn test_render_simple_sequence() {
    let input = r#"@startuml
Alice -> Bob : Hello
Bob --> Alice : Hi there
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("Alice"));
    assert!(svg.contains("Bob"));
    assert!(svg.contains("Hello"));
}

#[test]
fn test_render_sequence_with_participants() {
    let input = r#"@startuml
participant "Frontend App" as frontend
participant "API Gateway" as gateway
participant "Auth Service" as auth

frontend -> gateway : POST /login
gateway -> auth : validate
auth --> gateway : token
gateway --> frontend : 200 OK
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Frontend App"));
    assert!(svg.contains("API Gateway"));
    assert!(svg.contains("Auth Service"));
}

#[test]
fn test_render_sequence_with_groups() {
    let input = r#"@startuml
Alice -> Bob : request
alt success
    Bob --> Alice : response
else error
    Bob --> Alice : error
end
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("alt"));
    // Like PlantUML, the else section shows its condition in brackets
    assert!(svg.contains("[error]"));
}

#[test]
fn test_render_sequence_with_notes() {
    let input = r#"@startuml
Alice -> Bob : Hello
note right of Bob : This is a note
Bob --> Alice : Reply
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("This is a note"));
}

#[test]
fn test_render_unsupported_diagram() {
    let input = r#"@startmindmap
* root
** child
@endmindmap"#;

    let result = render_svg(input);
    assert!(matches!(result, Err(PlantUmlError::UnsupportedDiagram(_))));
}

#[test]
fn test_render_no_start_directive() {
    let input = "Alice -> Bob : Hello";
    let result = render_svg(input);
    assert!(matches!(result, Err(PlantUmlError::PreprocessError(_))));
}

#[test]
fn test_detect_diagram_types() {
    assert_eq!(
        detect_diagram_type("@startuml\n@enduml"),
        Some(DiagramType::Sequence)
    );
    assert_eq!(
        detect_diagram_type("@startmindmap\n@endmindmap"),
        Some(DiagramType::MindMap)
    );
    assert_eq!(
        detect_diagram_type("@startgantt\n@endgantt"),
        Some(DiagramType::Gantt)
    );
    assert_eq!(detect_diagram_type("hello world"), None);
}

#[test]
fn test_render_with_comments() {
    let input = r#"@startuml
' This is a comment
Alice -> Bob : Hello
/' Block comment '/
Bob --> Alice : Reply
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Hello"));
    assert!(svg.contains("Reply"));
    assert!(!svg.contains("This is a comment"));
    assert!(!svg.contains("Block comment"));
}

#[test]
fn test_render_with_title() {
    let input = r#"@startuml
title My Diagram
Alice -> Bob : Hello
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("My Diagram"));
}

#[test]
fn test_render_self_referencing_message() {
    let input = r#"@startuml
Alice -> Alice : Think
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Think"));
}

#[test]
fn test_render_sequence_with_separator() {
    let input = r#"@startuml
Alice -> Bob : Phase 1
== Initialization ==
Bob -> Alice : Phase 2
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Initialization"));
}

#[test]
fn test_render_complex_sequence() {
    let input = r#"@startuml
title Order Processing

actor User
participant "Web UI" as ui
participant "Order Service" as order
database "Order DB" as db

User -> ui : Place Order
activate ui
ui -> order : createOrder()
activate order
order -> db : INSERT order
activate db
db --> order : ok
deactivate db

alt payment success
    order --> ui : Order confirmed
    note right of ui : Show confirmation page
else payment failure
    order --> ui : Payment failed
    note right of ui : Show error message
end

deactivate order

== Notifications ==

ui --> User : Email confirmation
deactivate ui
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Order Processing"));
    assert!(svg.contains("Web UI"));
    assert!(svg.contains("createOrder()"));
    assert!(svg.contains("Notifications"));
}

#[test]
fn test_render_fixture_simple() {
    let input = std::fs::read_to_string("tests/fixtures/simple_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Alice"));
    assert!(svg.contains("Bob"));
}

#[test]
fn test_render_fixture_full() {
    let input = std::fs::read_to_string("tests/fixtures/full_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Authentication Flow"));
    assert!(svg.contains("Client"));
    assert!(svg.contains("Server"));
    assert!(svg.contains("Database"));
    // Regression: "Database --> Server : user data" must render as a message, not a participant
    assert!(svg.contains("user data"));
    assert!(
        !svg.contains("--&gt;"),
        "phantom participant '-->' should not appear"
    );
}

#[test]
fn test_svg_is_valid_xml_structure() {
    let input = r#"@startuml
Alice -> Bob : Test <special> & "chars"
@enduml"#;

    let svg = render_svg(input).unwrap();
    // Special characters should be escaped
    assert!(svg.contains("&lt;special&gt;"));
    assert!(svg.contains("&amp;"));
    assert!(svg.contains("&quot;chars&quot;"));
}

// Regression: database keyword followed by arrow should produce a message, not a participant
#[test]
fn test_no_phantom_participant_from_database_message() {
    let input = r#"@startuml
database DB
Alice -> DB : query
DB --> Alice : result
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("query"));
    assert!(svg.contains("result"));

    // Count participant boxes: should be exactly 2 (Alice, DB) x 2 (top+bottom) = 4
    let participant_fill_count = svg.matches(r##"fill="#E2E2F0""##).count();
    assert_eq!(
        participant_fill_count, 4,
        "expected 4 participant boxes (2 participants x top/bottom), got {}",
        participant_fill_count
    );
}

// Regression: complex diagram with database should not create phantom participants
#[test]
fn test_complex_diagram_no_phantom() {
    let input = r#"@startuml
participant App
database PostgreSQL

App -> PostgreSQL : INSERT
PostgreSQL --> App : ok
App -> PostgreSQL : SELECT
PostgreSQL --> App : rows
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("INSERT"));
    assert!(svg.contains("SELECT"));
    assert!(svg.contains("rows"));
    // No phantom "-->" participant
    assert!(!svg.contains("--&gt;"));
}

// --- Activity diagram tests ---

#[test]
fn test_render_simple_activity() {
    let input = r#"@startuml
start
:Hello World;
stop
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.ends_with("</svg>"));
    assert!(svg.contains("Hello World"));
}

#[test]
fn test_render_activity_with_if() {
    let input = r#"@startuml
start
if (condition?) then (yes)
  :Do A;
else (no)
  :Do B;
endif
stop
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Do A"));
    assert!(svg.contains("Do B"));
    assert!(svg.contains("condition?"));
}

#[test]
fn test_render_activity_with_while() {
    let input = r#"@startuml
start
while (more data?) is (yes)
  :Read;
endwhile (done)
stop
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("more data?"));
    assert!(svg.contains("Read"));
}

#[test]
fn test_render_activity_with_fork() {
    let input = r#"@startuml
start
fork
  :Task 1;
fork again
  :Task 2;
end fork
stop
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Task 1"));
    assert!(svg.contains("Task 2"));
}

#[test]
fn test_render_fixture_simple_activity() {
    let input = std::fs::read_to_string("tests/fixtures/simple_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Initialize"));
    assert!(svg.contains("Process Data"));
    assert!(svg.contains("Valid?"));
}

#[test]
fn test_render_fixture_complex_activity() {
    let input = std::fs::read_to_string("tests/fixtures/complex_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Order Processing"));
    assert!(svg.contains("Receive Order"));
    assert!(svg.contains("Ship Order"));
}

#[test]
fn test_activity_auto_detection() {
    // This should be detected as activity, not sequence
    let input = r#"@startuml
start
:action;
stop
@enduml"#;
    let svg = render_svg(input).unwrap();
    // Activity diagrams have rounded rects for actions
    assert!(svg.contains("rx="));
}

#[test]
fn test_sequence_not_misdetected_as_activity() {
    // Sequence diagram should still work
    let input = r#"@startuml
Alice -> Bob : Hello
@enduml"#;
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Alice"));
    assert!(svg.contains("Bob"));
}

#[test]
fn test_render_repeat_fixture() {
    let input = std::fs::read_to_string("tests/fixtures/repeat_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.contains("Process Row"));
    assert!(svg.contains("Next Row"));
    assert!(svg.contains("More rows?"));
    assert!(svg.contains("no"));
}

#[test]
fn test_render_return_fixture() {
    let input = std::fs::read_to_string("tests/fixtures/return_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    // return draws numbered replies; hide footbox drops the bottom row
    assert!(svg.contains("30 ok"));
    assert!(svg.contains("40 saved"));
    let client_count = svg.matches(">Client</text>").count();
    assert_eq!(
        client_count, 1,
        "hide footbox should leave only the top row"
    );
}

#[test]
fn test_render_edge_label_fixture() {
    let input = std::fs::read_to_string("tests/fixtures/label_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.contains("valid"));
    assert!(svg.contains("done"));
}

#[test]
fn test_classic_theme() {
    use pumlr::RenderOptions;
    let options = RenderOptions {
        theme: Some("classic".to_string()),
        ..Default::default()
    };

    let seq =
        pumlr::render_svg_with_options("@startuml\nAlice -> Bob : Hi\n@enduml", &options).unwrap();
    assert!(seq.contains("#FEFECE"), "classic participant fill");
    assert!(seq.contains("#A80036"), "classic stroke color");

    let act = pumlr::render_svg_with_options("@startuml\nstart\n:Work;\nstop\n@enduml", &options)
        .unwrap();
    assert!(act.contains("#FEFECE"), "classic action fill");
    assert!(act.contains("#A80036"), "classic edge color");

    // Default stays on the modern style
    let modern = pumlr::render_svg("@startuml\nstart\n:Work;\nstop\n@enduml").unwrap();
    assert!(modern.contains("#F1F1F1"));
    assert!(!modern.contains("#FEFECE"));
}
