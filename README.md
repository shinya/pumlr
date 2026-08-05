# pumlr

A pure-Rust PlantUML renderer — generates SVG from PlantUML text without Java or Graphviz.

The rendering is developed against the output of PlantUML 1.2026 (Java) as the
reference: every supported construct is compared side-by-side with the real
PlantUML output and matched in topology and styling (see *Development loop*
below). The default theme reproduces PlantUML's current default look; the
pre-2023 classic look (pale yellow / dark red) is available as the `classic`
theme.

## Status

Nine diagram types are supported: sequence, activity, class, state,
use case, component, mind map / WBS, Gantt, and JSON / YAML data diagrams.

### Sequence diagrams

- Participant types with icons: `participant`, `actor` (stick figure),
  `boundary`, `control`, `entity`, `database` (cylinder), `collections`,
  `queue` — with aliases (`participant "Long Name" as alias`) and fill
  colors (`participant Foo #LightBlue`)
- Arrows: `->`, `-->`, `->>`, `-->>` and left-facing variants; self-messages;
  arrow colors (`-[#red]>`)
- Activation bars: `activate` / `deactivate` / `return` (reply to the caller,
  arrows stop at bar edges) and the `++` / `--` message shorthand
- `box "Title" #Color ... end box` participant groupings
- Groups: `alt/else`, `loop`, `opt`, `break`, `par`, `critical`, `group`
- Notes: `note left of`, `note right of`, `note over A, B` (spanning),
  with colors (`note right of X #Color : text`)
- `create` participants (appear at their first message), `ref over A, B`
- `autonumber [start [increment] ["format"]]` with `<b>`-style markup in the
  format (styled numbers), nested activations, multiline notes inside
  groups, `hide footbox`
- `header` / `footer` / `caption`
- Separators (`== label ==`), delays (`...`), spacing (`|||`, `||N||`), title

### Activity diagrams

- Actions `:text;` with colors (`:text; <<#Color>>`, legacy `#Color:text;`),
  `start` / `stop` / `end` / `detach`
- `if / then / elseif / else / endif` (elseif renders as a PlantUML-style
  hexagon chain), `switch / case / endswitch`
- `while / endwhile`, `repeat / backward / repeat while`
- `fork / fork again / end fork`, `partition { ... }`
- Swimlanes (`|Lane|`, including lane switches inside if/while/repeat branches)
- Edge labels (`-> label;`), notes, title

### Class diagrams

- `class` / `abstract class` / `interface` / `enum` with stereotype icons
  (C / A / I / E circles), fields and methods with visibility icons
  (`+` / `-` / `#` / `~`), `Class : member` syntax, aliases and fill colors
- Relations: extension `<|--`, realization `<|..`, composition `*--`,
  aggregation `o--`, association `-->`, dependency `..>`, with labels and
  cardinalities (`"1" --> "0..*"`), direction hints (`-down->`)
- `package Name { ... }` folder frames (nested), `<<stereotype>>` display,
  `{static}` members (underlined), lollipop interfaces (`circle` / `()`)

### State diagrams

- `[*]` start / end pseudo states, transitions with labels,
  state descriptions (`State : text`)
- Composite states (`state X { ... }`, nested), concurrent regions (`--`
  separators), shallow history (`[H]`), `state "Long" as S`
- Bidirectional transition pairs are drawn as separated parallel edges;
  straight-line routing detours around boxes in the way

### Use case diagrams

- `actor` (stick figure) and `usecase` (ellipse sized like PlantUML's),
  inline `:Actor:` / `(Use case)` forms, aliases
- Associations with `<<include>>` / `<<extend>>` labels (rendered as
  «guillemets»), dashed `..>` arrows
- `rectangle Name { ... }` containers, `left to right direction`

### Component diagrams

- `[Component]` boxes with the component icon, `component` / `interface`
  keywords, lollipop interfaces (`Auth - [API]`)
- `package` / `node` / `folder` / `cloud` / `database` / `frame` containers
- Arrows with labels, dashed dependencies

### Mind maps & WBS

- `@startmindmap` / `@startwbs` with `*` depth markers, `*[#color]` node
  colors, `*_` boxless nodes, `left side` / OrgMode `+`/`-` side control
- Mind map: root at the left, bezier connectors, subtree centering
- WBS: root on top, level-1 row, deeper levels as indented vertical lists

### Gantt charts

- `Project starts <date>`, `[Task] starts <date> and lasts N days`,
  `[Task] starts at [Other]'s end` dependency chains with elbow arrows
- Closed weekdays (`saturday are closed`: shading + working-day durations
  with split bars), milestones (`happens at`), task colors (`is colored
  in`), resources (`on {Alice}`)
- Day-grid timeline with weekday / day headers (mirrored footer), month
  spans, task bars with inline labels

### JSON / YAML data diagrams

- `@startjson` (full JSON) and `@startyaml` (a practical YAML subset:
  nested maps, lists incl. maps-in-lists, block scalars `|`/`>`, anchors
  kept literal like PlantUML)
- Two-column tables with bold keys, single-column array boxes, dashed
  bullet links to nested boxes, `☑ true` checkboxes for JSON booleans

## Usage

```toml
[dependencies]
pumlr = "0.1"
```

```rust
let input = r#"@startuml
Alice -> Bob : Hello
Bob --> Alice : Hi
@enduml"#;

let svg = pumlr::render_svg(input).unwrap();

// Or pick a theme:
let options = pumlr::RenderOptions {
    theme: Some("classic".into()),
    ..Default::default()
};
let svg = pumlr::render_svg_with_options(input, &options).unwrap();
```

Command line (via the bundled example):

```bash
cargo run --example render -- diagram.puml diagram.svg
```

## Development loop

The project is developed by comparing against the Java PlantUML output:

```bash
# Requires: plantuml (Java) and rsvg-convert on PATH
scripts/compare.sh                  # render all fixtures with both engines
scripts/compare.sh simple_activity  # or a single fixture
open compare/                       # *_ref.png (Java) vs *_pumlr.png (this crate)
```

Style values (colors, stroke widths, corner radii, spacings) are extracted
from the reference SVG output and recorded in `SPEC.md`.

```bash
cargo test    # unit + integration tests
```

### Project structure

```
src/
  lib.rs               # Public API: render_svg(), render_svg_with_options()
  preprocess.rs        # @start/@end detection, comment stripping
  parser/              # PlantUML text -> AST (sequence.rs, activity.rs)
  ast/                 # AST type definitions
  layout/              # AST -> positioned primitives (+ text_measure.rs)
  render/svg.rs        # Primitives -> SVG string
  render/primitives.rs # Drawing primitives (Rect, Line, Arrow, ...)
  theme/               # Theme trait, default (PlantUML 1.2026) and classic
tests/
  integration_test.rs  # End-to-end tests
  fixtures/            # .puml files used by tests and scripts/compare.sh
scripts/compare.sh     # Side-by-side comparison against Java PlantUML
```

## License

MIT
