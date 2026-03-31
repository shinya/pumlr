fn main() {
    let fixtures = [
        ("tests/fixtures/full_sequence.puml", "output_sequence.svg"),
        ("tests/fixtures/complex_activity.puml", "output_activity.svg"),
    ];

    for (input_path, output_path) in &fixtures {
        let input = std::fs::read_to_string(input_path)
            .unwrap_or_else(|_| panic!("failed to read {}", input_path));
        let svg = plantuml_rust::render_svg(&input)
            .unwrap_or_else(|e| panic!("failed to render {}: {}", input_path, e));
        std::fs::write(output_path, &svg)
            .unwrap_or_else(|_| panic!("failed to write {}", output_path));
        println!("Generated {} ({} bytes)", output_path, svg.len());
    }
}
