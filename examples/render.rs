//! Render a single PlantUML file to SVG.
//!
//! Usage: cargo run --example render -- <input.puml> <output.svg>

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <input.puml> <output.svg>", args[0]);
        std::process::exit(1);
    }

    let input = std::fs::read_to_string(&args[1])
        .unwrap_or_else(|e| panic!("failed to read {}: {}", args[1], e));
    let svg =
        pumlr::render_svg(&input).unwrap_or_else(|e| panic!("failed to render {}: {}", args[1], e));
    std::fs::write(&args[2], &svg).unwrap_or_else(|e| panic!("failed to write {}: {}", args[2], e));
    println!("Generated {} ({} bytes)", args[2], svg.len());
}
