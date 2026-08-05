//! Integration tests for the YAML extensions: block scalars (`|` / `>`),
//! anchors/aliases (`&` / `*`), and list items that are nested maps.

use pumlr::render_svg;

fn render_fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{}.puml", env!("CARGO_MANIFEST_DIR"), name);
    let input = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path, e));
    render_svg(&input).unwrap_or_else(|e| panic!("render {}: {}", path, e))
}

#[test]
fn test_block_yaml_fixture_renders() {
    let svg = render_fixture("block_yaml");
    assert!(svg.starts_with("<svg"));
    // Literal block lines all appear; per the PlantUML reference the value is
    // kept on a single row, so lines are joined with spaces in the SVG text.
    assert!(svg.contains("first line second line third line"));
    // Rows following the block scalar are not swallowed by it.
    assert!(svg.contains("after"));
    assert!(svg.contains("done"));
    // No literal newline may survive inside a text element (single-row cell).
    assert!(!svg.contains("first line\n"));
}

#[test]
fn test_folded_block_renders_single_line() {
    let svg = render_svg("@startyaml\nfold: >\n  this text\n  is folded\nafter: x\n@endyaml\n")
        .unwrap();
    assert!(svg.contains("this text is folded"));
    assert!(svg.contains("after"));
}

#[test]
fn test_anchor_yaml_fixture_renders_literally() {
    let svg = render_fixture("anchor_yaml");
    assert!(svg.starts_with("<svg"));
    // The PlantUML reference does not resolve anchors: the raw tokens show up.
    assert!(svg.contains("&amp;n pumlr"));
    assert!(svg.contains("*n"));
    assert!(svg.contains("&amp;v 0.1.0"));
    assert!(svg.contains("*v"));
}

#[test]
fn test_listmap_yaml_fixture_renders_child_boxes() {
    let svg = render_fixture("listmap_yaml");
    assert!(svg.starts_with("<svg"));
    // Each list-item map becomes its own child box with bold keys.
    for label in ["alice", "admin", "bob", "dev", "x", "y"] {
        assert!(svg.contains(label), "missing {}", label);
    }
    // Two map items -> "name"/"role" keys rendered (bold key cells exist).
    assert!(svg.contains("name"));
    assert!(svg.contains("role"));
}

#[test]
fn test_map_anchor_block_renders_without_truncation() {
    // The Java reference crashes on `key: &name` + nested block; pumlr nests
    // the block and keeps the alias literal.
    let svg = render_svg(
        "@startyaml\nbase: &b\n  host: localhost\n  port: 5432\ncopy: *b\n@endyaml\n",
    )
    .unwrap();
    assert!(svg.contains("localhost"));
    assert!(svg.contains("5432"));
    assert!(svg.contains("*b"));
}

#[test]
fn test_simple_yaml_regression() {
    let svg = render_fixture("simple_yaml");
    for label in ["pumlr", "0.1.0", "sequence", "activity", "shinya", "true"] {
        assert!(svg.contains(label), "missing {}", label);
    }
}

#[test]
fn test_simple_json_regression() {
    let svg = render_fixture("simple_json");
    assert!(svg.starts_with("<svg"));
}
