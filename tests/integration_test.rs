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
fn test_render_mindmap_diagram() {
    let input = r#"@startmindmap
* root
** child
@endmindmap"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("root"));
    assert!(svg.contains("child"));
    // Mind map boxes use the 12.5px corner radius.
    assert!(svg.contains(r#"rx="12.5""#));
}

#[test]
fn test_render_wbs_diagram() {
    let input = r#"@startwbs
* root
** child
@endwbs"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("root"));
    assert!(svg.contains("child"));
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

#[test]
fn test_box_and_activation_shorthand() {
    let input = std::fs::read_to_string("tests/fixtures/box_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    // Box panels with titles; #LightBlue resolves to a CSS color name
    assert!(svg.contains("Frontend"));
    assert!(svg.contains("Backend"));
    assert!(svg.contains(r#"fill="lightblue""#));
    assert!(svg.contains(r##"fill="#DDDDDD""##));
    // ++/-- shorthand creates activation bars (white 10px-wide rects)
    let bar_count = svg.matches(r##"fill="#FFFFFF" stroke="#181818""##).count();
    assert_eq!(bar_count, 2, "expected activation bars for Api and Db");
}

#[test]
fn test_color_overrides() {
    let seq = std::fs::read_to_string("tests/fixtures/color_diagrams.puml").unwrap();
    let svg = render_svg(&seq).unwrap();
    assert!(
        svg.contains(r#"fill="lightgreen""#),
        "participant color name"
    );
    assert!(svg.contains(r##"fill="#FFAAAA""##), "participant hex color");
    assert!(svg.contains(r#"fill="lightblue""#), "note color");

    let act = std::fs::read_to_string("tests/fixtures/color_activity.puml").unwrap();
    let svg = render_svg(&act).unwrap();
    assert!(
        svg.contains(r#"fill="lightblue""#),
        "action stereotype color"
    );
    assert!(
        svg.contains(r#"fill="palegreen""#),
        "action color in branch"
    );
    assert!(svg.contains(r##"fill="#FFAAAA""##), "action hex color");
}

#[test]
fn test_arrow_colors_and_page_decorations() {
    let input = std::fs::read_to_string("tests/fixtures/arrow_color_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.contains(r##"stroke="#FF0000""##) || svg.contains(r#"stroke="red""#));
    assert!(svg.contains(r##"stroke="#0000FF""##));
    assert!(svg.contains("Internal API"), "header");
    assert!(svg.contains("Page 1 of 1"), "footer");
    assert!(svg.contains("Figure 1: error handling"), "caption");
}

#[test]
fn test_autonumber_format() {
    let input = std::fs::read_to_string("tests/fixtures/numfmt_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    assert!(svg.contains("[010] first"));
    assert!(svg.contains("[020] second"));
    assert!(svg.contains("[030] third"));
}

#[test]
fn test_create_and_ref_over() {
    let input = std::fs::read_to_string("tests/fixtures/create_ref_sequence.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    // Worker's head appears once (created mid-diagram) plus the tail row
    let worker_count = svg.matches(">Worker</text>").count();
    assert_eq!(
        worker_count, 2,
        "created participant: one mid-diagram head + one tail"
    );
    assert!(svg.contains(">ref</text>"));
    assert!(svg.contains("shared setup"));
    assert!(svg.contains("see diagram 2"));
}

#[test]
fn test_swimlanes() {
    let input = std::fs::read_to_string("tests/fixtures/swimlane_activity.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    for lane in ["Customer", "Shop", "Delivery"] {
        assert!(svg.contains(lane), "lane title {lane}");
    }
    // 4 boundary lines for 3 lanes, 1.5px black
    let boundary_count = svg
        .matches(r##"stroke="#000000" stroke-width="1.5""##)
        .count();
    assert_eq!(boundary_count, 4, "lane boundary lines");
    assert!(svg.contains("Receive Package"));
}

#[test]
fn test_render_class_diagram() {
    let input = r#"@startuml
class Animal {
  +name: String
  +makeSound(): void
}
class Dog
Animal <|-- Dog
@enduml"#;

    assert_eq!(
        pumlr::detect_diagram_type(input),
        Some(DiagramType::Sequence)
    ); // refined at render time
    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Animal"));
    assert!(svg.contains("makeSound(): void"));
    // Class boxes use rx=2.5 rounded corners.
    assert!(svg.contains(r#"rx="2.5""#));
}

#[test]
fn test_render_class_fixtures() {
    for name in ["simple_class", "relations_class", "package_class"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}

#[test]
fn test_render_state_diagram() {
    let input = r#"@startuml
[*] --> Idle
Idle --> Running : start
Running --> [*]
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Idle"));
    assert!(svg.contains("Running"));
    assert!(svg.contains("start"));
    // State boxes use rx=12.5 rounded corners.
    assert!(svg.contains(r#"rx="12.5""#));
}

#[test]
fn test_render_state_fixtures() {
    for name in ["simple_state", "nested_state"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}

#[test]
fn test_render_usecase_diagram() {
    let input = r#"@startuml
actor Customer
usecase (Browse items) as UC1
Customer --> UC1
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Customer"));
    assert!(svg.contains("Browse items"));
    assert!(svg.contains("<ellipse"));
}

#[test]
fn test_render_usecase_fixtures() {
    for name in ["simple_usecase", "rect_usecase"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}

#[test]
fn test_render_component_diagram() {
    let input = r#"@startuml
[Web UI] --> [API Server] : HTTPS
@enduml"#;

    let svg = render_svg(input).unwrap();
    assert!(svg.contains("Web UI"));
    assert!(svg.contains("API Server"));
    assert!(svg.contains("HTTPS"));
}

#[test]
fn test_render_component_fixtures() {
    for name in ["simple_component", "package_component"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}

#[test]
fn test_render_gantt_json_yaml_fixtures() {
    for name in ["simple_gantt", "simple_json", "simple_yaml"] {
        let path = format!("tests/fixtures/{}.puml", name);
        let input = std::fs::read_to_string(&path).unwrap();
        let svg = render_svg(&input).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(svg.starts_with("<svg"), "{name}");
    }
}

#[test]
fn test_render_json_checkbox_bool() {
    let svg = render_svg("@startjson\n{\"ok\": true}\n@endjson").unwrap();
    assert!(svg.contains('\u{2611}'));
}

#[test]
fn test_render_yaml_plain_bool() {
    let svg = render_svg("@startyaml\nok: true\n@endyaml").unwrap();
    assert!(svg.contains(">true<"));
}
