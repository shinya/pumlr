/// Drawing primitives used by the layout engine.
/// These are renderer-agnostic and get converted to SVG by the renderer.

#[derive(Debug, Clone)]
pub enum Primitive {
    Rect(Rect),
    Line(Line),
    Text(Text),
    Arrow(Arrow),
    DashedLine(DashedLine),
    Polygon(Polygon),
    Path(Path),
}

#[derive(Debug, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f32,
    pub rx: f32,
    pub ry: f32,
}

#[derive(Debug, Clone)]
pub struct Line {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke: String,
    pub stroke_width: f32,
}

#[derive(Debug, Clone)]
pub struct DashedLine {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke: String,
    pub stroke_width: f32,
    pub dash_array: String,
}

#[derive(Debug, Clone)]
pub struct Text {
    pub x: f32,
    pub y: f32,
    pub content: String,
    pub font_size: f32,
    pub font_family: String,
    pub fill: String,
    pub anchor: TextAnchor,
}

#[derive(Debug, Clone, Copy)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone)]
pub struct Arrow {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stroke: String,
    pub stroke_width: f32,
    pub head: ArrowHeadStyle,
    pub dashed: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum ArrowHeadStyle {
    Filled,
    Open,
}

#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<(f32, f32)>,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f32,
}

#[derive(Debug, Clone)]
pub struct Path {
    pub d: String,
    pub fill: String,
    pub stroke: String,
    pub stroke_width: f32,
    pub dashed: bool,
}
