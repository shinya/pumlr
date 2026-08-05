/// Integration tests for mind-map extensions (left side / OrgMode sides).
use pumlr::render_svg;

/// x attribute of the `<text>` element whose content is exactly `label`.
fn text_x(svg: &str, label: &str) -> f32 {
    let mut rest = svg;
    while let Some(start) = rest.find("<text") {
        let elem = &rest[start..];
        let close = elem.find('>').unwrap();
        let end = elem.find("</text>").unwrap();
        if &elem[close + 1..end] == label {
            let attrs = &elem[..close];
            let xpos = attrs.find("x=\"").unwrap() + 3;
            let xend = attrs[xpos..].find('"').unwrap();
            return attrs[xpos..xpos + xend].parse().unwrap();
        }
        rest = &rest[start + end..];
    }
    panic!("label {label:?} not found in svg");
}

#[test]
fn test_sides_mindmap_fixture() {
    let input = std::fs::read_to_string("tests/fixtures/sides_mindmap.puml").unwrap();
    let svg = render_svg(&input).unwrap();
    for label in [
        "OS", "Ubuntu", "Linux Mint", "Kubuntu", "LMDE", "SolydXK", "Fedora", "CentOS", "Rocky",
        "Arch", "Manjaro",
    ] {
        assert!(svg.contains(label), "missing {label}");
    }
    let root = text_x(&svg, "OS");
    // Nodes after `left side` are drawn to the left of the root.
    assert!(text_x(&svg, "Fedora") < root);
    assert!(text_x(&svg, "CentOS") < text_x(&svg, "Fedora"));
    assert!(text_x(&svg, "Manjaro") < text_x(&svg, "Arch"));
    // Nodes before `left side` stay on the right.
    assert!(text_x(&svg, "Ubuntu") > root);
    assert!(text_x(&svg, "Linux Mint") > text_x(&svg, "Ubuntu"));
    assert!(text_x(&svg, "LMDE") > root);
}

#[test]
fn test_left_side_directive_switches_sides() {
    let svg = render_svg("@startmindmap\n* root\n** right1\nleft side\n** west1\n@endmindmap")
        .unwrap();
    let root = text_x(&svg, "root");
    assert!(text_x(&svg, "right1") > root);
    assert!(text_x(&svg, "west1") < root);
}

#[test]
fn test_right_side_directive_switches_back() {
    let svg = render_svg(
        "@startmindmap\n* root\nleft side\n** west1\nright side\n** east1\n@endmindmap",
    )
    .unwrap();
    let root = text_x(&svg, "root");
    assert!(text_x(&svg, "west1") < root);
    assert!(text_x(&svg, "east1") > root);
}

#[test]
fn test_orgmode_plus_minus_sides() {
    let svg = render_svg(
        "@startmindmap\n+ root\n++ east1\n+++ east2\n-- west1\n--- west2\n@endmindmap",
    )
    .unwrap();
    let root = text_x(&svg, "root");
    assert!(text_x(&svg, "east1") > root);
    assert!(text_x(&svg, "east2") > text_x(&svg, "east1"));
    assert!(text_x(&svg, "west1") < root);
    assert!(text_x(&svg, "west2") < text_x(&svg, "west1"));
}

#[test]
fn test_left_only_mindmap_renders() {
    let svg = render_svg("@startmindmap\n* root\nleft side\n** west1\n** west2\n@endmindmap")
        .unwrap();
    let root = text_x(&svg, "root");
    assert!(text_x(&svg, "west1") < root);
    assert!(text_x(&svg, "west2") < root);
}
