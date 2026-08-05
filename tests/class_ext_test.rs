/// Integration tests for class-diagram extensions:
/// nested packages, stereotype display, {static} underline, lollipop interfaces.
use pumlr::render_svg;

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("tests/fixtures/{name}.puml")).unwrap()
}

/// Attribute value of the first `<{tag}` element containing `needle`.
fn attr_of<'a>(svg: &'a str, tag: &str, needle: &str, attr: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let mut rest = svg;
    while let Some(start) = rest.find(&open) {
        let elem = &rest[start..];
        let end = elem.find('>').map(|i| i + 1).unwrap_or(elem.len());
        // Include text content for tags like <text ...>content</text>.
        let scope_end = elem.find(&format!("</{tag}>")).map(|i| i + 1).unwrap_or(end);
        let scope = &elem[..scope_end.max(end)];
        if scope.contains(needle) {
            let key = format!("{attr}=\"");
            let attrs = &elem[..end];
            if let Some(pos) = attrs.find(&key) {
                let val = &attrs[pos + key.len()..];
                return val.find('"').map(|q| &val[..q]);
            }
            return None;
        }
        rest = &rest[start + open.len()..];
    }
    None
}

#[test]
fn test_nested_pkg_fixture_renders() {
    let svg = render_svg(&fixture("nested_pkg_class")).unwrap();
    for needle in ["outer", "inner", "Item", "Service", "Client"] {
        assert!(svg.contains(needle), "missing {needle}");
    }
    // One folder-frame path per package.
    assert_eq!(svg.matches("<path").count(), 2);
}

#[test]
fn test_nested_pkg_inner_frame_inside_outer() {
    let svg = render_svg("@startuml\npackage outer {\n  package inner {\n    class A\n  }\n}\n@enduml").unwrap();
    // Both frames drawn; the inner tab text sits right of / below the outer's.
    let outer_x: f32 = attr_of(&svg, "text", ">outer<", "x").unwrap().parse().unwrap();
    let inner_x: f32 = attr_of(&svg, "text", ">inner<", "x").unwrap().parse().unwrap();
    let outer_y: f32 = attr_of(&svg, "text", ">outer<", "y").unwrap().parse().unwrap();
    let inner_y: f32 = attr_of(&svg, "text", ">inner<", "y").unwrap().parse().unwrap();
    assert!(inner_x > outer_x);
    assert!(inner_y > outer_y);
}

#[test]
fn test_stereotype_displayed_above_name() {
    let svg = render_svg(&fixture("nested_pkg_class")).unwrap();
    // Guillemet-wrapped stereotype rendered in italic, above the class name.
    assert!(svg.contains("\u{ab}entity\u{bb}"));
    assert!(svg.contains("\u{ab}service\u{bb}"));
    let stereo_y: f32 = attr_of(&svg, "text", "\u{ab}entity\u{bb}", "y")
        .unwrap()
        .parse()
        .unwrap();
    let name_y: f32 = attr_of(&svg, "text", ">Item<", "y").unwrap().parse().unwrap();
    assert!(stereo_y < name_y, "stereotype above the class name");
    assert_eq!(
        attr_of(&svg, "text", "\u{ab}entity\u{bb}", "font-style"),
        Some("italic")
    );
}

#[test]
fn test_stereotype_grows_header() {
    let plain = render_svg("@startuml\nclass Foo\n@enduml").unwrap();
    let stereo = render_svg("@startuml\nclass Foo <<entity>>\n@enduml").unwrap();
    let h_plain: f32 = attr_of(&plain, "rect", "rx=\"2.5\"", "height").unwrap().parse().unwrap();
    let h_stereo: f32 = attr_of(&stereo, "rect", "rx=\"2.5\"", "height").unwrap().parse().unwrap();
    assert!((h_plain - 48.0).abs() < 0.01);
    assert!((h_stereo - 56.6211).abs() < 0.01);
}

#[test]
fn test_static_members_underlined() {
    let svg = render_svg(&fixture("static_lolli_class")).unwrap();
    for member in ["count: int", "instance(): Counter"] {
        let elem = attr_of(&svg, "text", member, "text-decoration");
        assert_eq!(elem, Some("underline"), "{member} must be underlined");
    }
    // Non-static member is not underlined.
    assert_eq!(attr_of(&svg, "text", "value: int", "text-decoration"), None);
    // No bold on static members (PlantUML uses underline only).
    assert_eq!(attr_of(&svg, "text", "count: int", "font-weight"), None);
}

#[test]
fn test_lollipop_interface_circle() {
    let svg = render_svg(&fixture("static_lolli_class")).unwrap();
    // Lollipop circle: r=8, default fill.
    let r = attr_of(&svg, "circle", "r=\"8\"", "r");
    assert_eq!(r, Some("8"));
    assert!(svg.contains("Runnable"));
    // Circle is left of the class box and vertically centered against it.
    let cx: f32 = attr_of(&svg, "circle", "r=\"8\"", "cx").unwrap().parse().unwrap();
    let cy: f32 = attr_of(&svg, "circle", "r=\"8\"", "cy").unwrap().parse().unwrap();
    let bx: f32 = attr_of(&svg, "rect", "rx=\"2.5\"", "x").unwrap().parse().unwrap();
    let by: f32 = attr_of(&svg, "rect", "rx=\"2.5\"", "y").unwrap().parse().unwrap();
    let bh: f32 = attr_of(&svg, "rect", "rx=\"2.5\"", "height").unwrap().parse().unwrap();
    assert!(cx < bx, "circle to the left of the box");
    assert!((cy - (by + bh / 2.0)).abs() < 0.5, "circle vertically centered");
}

#[test]
fn test_lollipop_arrow_variant() {
    let svg = render_svg("@startuml\nclass Bar\nBaz ()-- Bar\n@enduml").unwrap();
    assert!(svg.contains("Baz"));
    assert!(attr_of(&svg, "circle", "r=\"8\"", "cx").is_some());
}

#[test]
fn test_existing_class_fixtures_still_render() {
    for name in ["simple_class", "relations_class", "package_class"] {
        let svg = render_svg(&fixture(name)).unwrap();
        assert!(svg.contains("<svg"), "{name} failed to render");
    }
}
