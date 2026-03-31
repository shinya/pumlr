# plantuml-rust

PlantUML text to SVG renderer written in pure Rust. No Java, no Graphviz, no external dependencies.

## Status

Early development. Sequence diagrams are supported.

### Supported features (Sequence Diagram)

- Participant types: `participant`, `actor`, `boundary`, `control`, `entity`, `database`, `collections`, `queue`
- Aliases: `participant "Long Name" as alias`
- Arrows: `->`, `-->`, `->>`, `-->>` and left-facing variants
- Self-referencing messages
- Groups: `alt/else`, `loop`, `opt`, `break`, `par`, `critical`, `group`
- Notes: `note left of`, `note right of`, `note over` (single-line and multi-line)
- `activate` / `deactivate`
- `autonumber`
- Separators (`== label ==`), delays (`...`), spacing (`|||`, `||N||`)
- Title
- Comments (single-line `'` and block `/' ... '/`)

## Usage

Add to `Cargo.toml`:

```toml
[dependencies]
plantuml-rust = { path = "../plantuml-rust" }
```

```rust
let input = r#"@startuml
Alice -> Bob : Hello
Bob --> Alice : Hi
@enduml"#;

let svg = plantuml_rust::render_svg(input).unwrap();
// svg is a String containing valid SVG XML
```

## Development

```bash
# Run tests
cargo test

# Lint
cargo clippy

# Generate a sample SVG
cargo run --example generate_svg
open output.svg
```

### Project structure

```
src/
  lib.rs              # Public API: render_svg(), detect_diagram_type()
  preprocess.rs        # @start/@end detection, comment stripping
  parser/sequence.rs   # PlantUML text -> AST
  ast/sequence.rs      # AST type definitions
  layout/sequence.rs   # AST -> positioned primitives
  layout/text_measure.rs
  render/svg.rs        # Primitives -> SVG string
  render/primitives.rs # Drawing primitives (Rect, Line, Arrow, etc.)
  theme/               # Theme trait + default theme
tests/
  integration_test.rs  # End-to-end tests
  fixtures/            # Sample .puml files
```

## License

MIT
