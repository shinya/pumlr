/// Generic layered (Sugiyama-lite) graph layout shared by node-and-edge
/// diagrams (class, state, use case, component).
///
/// Nodes are ranked top-to-bottom by edge constraints (longest-path), ordered
/// within a rank by barycenter sweeps, and X-aligned towards their neighbors.

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub width: f32,
    pub height: f32,
    /// Output: top-left position, origin (0,0).
    pub x: f32,
    pub y: f32,
}

impl GraphNode {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            width,
            height,
            x: 0.0,
            y: 0.0,
        }
    }

    pub fn center_x(&self) -> f32 {
        self.x + self.width / 2.0
    }
}

#[derive(Debug, Clone)]
pub struct GraphEdge {
    /// Upper node (rank source).
    pub from: usize,
    /// Lower node; gets `rank(from) + min_len`.
    pub to: usize,
    /// 0 keeps both on the same rank (PlantUML single-dash arrows).
    pub min_len: usize,
    /// Labeled edges widen the gap between their ranks.
    pub labeled: bool,
}

pub struct GraphLayoutResult {
    pub width: f32,
    pub height: f32,
    pub ranks: Vec<usize>,
}

/// Lay out `nodes` in place. `vgap` is the base vertical gap between ranks,
/// extended by `labeled_extra` when a labeled edge crosses the gap.
pub fn layout(
    nodes: &mut [GraphNode],
    edges: &[GraphEdge],
    hgap: f32,
    vgap: f32,
    labeled_extra: f32,
) -> GraphLayoutResult {
    let n = nodes.len();
    if n == 0 {
        return GraphLayoutResult {
            width: 0.0,
            height: 0.0,
            ranks: Vec::new(),
        };
    }

    // --- 1a. Same-rank (min_len == 0) edges merge nodes into rank groups
    // (dot's rank=same semantics) via union-find.
    let mut uf: Vec<usize> = (0..n).collect();
    fn find(uf: &mut [usize], mut i: usize) -> usize {
        while uf[i] != i {
            uf[i] = uf[uf[i]];
            i = uf[i];
        }
        i
    }
    for e in edges {
        if e.min_len == 0 {
            let (a, b) = (find(&mut uf, e.from), find(&mut uf, e.to));
            if a != b {
                uf[a] = b;
            }
        }
    }
    let group: Vec<usize> = (0..n).map(|i| find(&mut uf, i)).collect();

    // --- 1b. Rank assignment: longest path over a DAG (on groups) obtained by
    // dropping cycle-closing (back) edges found via DFS in declaration order.
    let group_edges: Vec<GraphEdge> = edges
        .iter()
        .filter(|e| e.min_len > 0 && group[e.from] != group[e.to])
        .map(|e| GraphEdge {
            from: group[e.from],
            to: group[e.to],
            min_len: e.min_len,
            labeled: e.labeled,
        })
        .collect();
    let edges_for_rank: &[GraphEdge] = &group_edges;
    let forward: Vec<&GraphEdge> = {
        let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
        for (ei, e) in edges_for_rank.iter().enumerate() {
            out[e.from].push(ei);
        }
        // 0 = unvisited, 1 = on stack, 2 = done
        let mut color = vec![0u8; n];
        let mut keep = vec![true; edges_for_rank.len()];
        // Iterative DFS from every unvisited node (declaration order).
        for root in 0..n {
            if color[root] != 0 {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = vec![(root, 0)];
            color[root] = 1;
            while let Some(&mut (node, ref mut next)) = stack.last_mut() {
                if *next < out[node].len() {
                    let ei = out[node][*next];
                    *next += 1;
                    let target = edges_for_rank[ei].to;
                    match color[target] {
                        0 => {
                            color[target] = 1;
                            stack.push((target, 0));
                        }
                        1 => keep[ei] = false, // back edge closes a cycle
                        _ => {}
                    }
                } else {
                    color[node] = 2;
                    stack.pop();
                }
            }
        }
        edges_for_rank
            .iter()
            .enumerate()
            .filter(|(ei, _)| keep[*ei])
            .map(|(_, e)| e)
            .collect()
    };

    let mut ranks = vec![0usize; n];
    for _ in 0..=n {
        let mut changed = false;
        for e in &forward {
            let want = ranks[e.from] + e.min_len;
            if ranks[e.to] < want {
                ranks[e.to] = want;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // Propagate group ranks back to every member node.
    let ranks: Vec<usize> = (0..n).map(|i| ranks[group[i]]).collect();
    let max_rank = ranks.iter().copied().max().unwrap_or(0);

    // --- 2. Order within ranks: initial declaration order + barycenter sweeps.
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
    for (i, &r) in ranks.iter().enumerate() {
        rows[r].push(i);
    }

    // Neighbor lists (undirected for ordering purposes).
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges {
        neighbors[e.from].push(e.to);
        neighbors[e.to].push(e.from);
    }

    for _ in 0..3 {
        // Position index of each node within its row.
        let mut pos = vec![0.0f32; n];
        for row in &rows {
            for (i, &node) in row.iter().enumerate() {
                pos[node] = i as f32;
            }
        }
        for row in rows.iter_mut() {
            let barycenter = |node: usize, pos: &[f32]| -> f32 {
                let others: Vec<f32> = neighbors[node]
                    .iter()
                    .filter(|&&o| ranks[o] != ranks[node])
                    .map(|&o| pos[o])
                    .collect();
                if others.is_empty() {
                    pos[node]
                } else {
                    others.iter().sum::<f32>() / others.len() as f32
                }
            };
            row.sort_by(|&a, &b| {
                barycenter(a, &pos)
                    .partial_cmp(&barycenter(b, &pos))
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }
    }

    // --- 3. X positions: pack, then align to neighbor centers.
    for row in &rows {
        let mut x = 0.0f32;
        for &node in row {
            nodes[node].x = x;
            x += nodes[node].width + hgap;
        }
    }

    for sweep in 0..6 {
        let downward = sweep % 2 == 0;
        let row_range: Vec<usize> = if downward {
            (0..rows.len()).collect()
        } else {
            (0..rows.len()).rev().collect()
        };
        for &r in &row_range {
            let row = &rows[r];
            // Desired center = mean of adjacent-rank neighbor centers.
            let desired: Vec<f32> = row
                .iter()
                .map(|&node| {
                    let adj: Vec<f32> = neighbors[node]
                        .iter()
                        .filter(|&&o| ranks[o] != ranks[node])
                        .map(|&o| nodes[o].center_x())
                        .collect();
                    if adj.is_empty() {
                        nodes[node].center_x()
                    } else {
                        adj.iter().sum::<f32>() / adj.len() as f32
                    }
                })
                .collect();
            // Left-to-right pass with min-gap clamping, then shift the row
            // back by the mean displacement so clamping doesn't drift it.
            let mut placed: Vec<f32> = Vec::with_capacity(row.len());
            let mut min_x = f32::NEG_INFINITY;
            for (i, &node) in row.iter().enumerate() {
                let want = desired[i] - nodes[node].width / 2.0;
                let x = if min_x.is_finite() { want.max(min_x) } else { want };
                placed.push(x);
                min_x = x + nodes[node].width + hgap;
            }
            let mean_shift: f32 = placed
                .iter()
                .enumerate()
                .map(|(i, &x)| x - (desired[i] - nodes[row[i]].width / 2.0))
                .sum::<f32>()
                / row.len() as f32;
            for (i, &node) in row.iter().enumerate() {
                nodes[node].x = placed[i] - mean_shift;
            }
        }
    }

    // Normalize to x >= 0.
    let min_x = nodes.iter().map(|nd| nd.x).fold(f32::INFINITY, f32::min);
    for nd in nodes.iter_mut() {
        nd.x -= min_x;
    }

    // --- 4. Y positions: rows stacked with gaps; center within row.
    let mut y = 0.0f32;
    for (r, row) in rows.iter().enumerate() {
        let row_h = row
            .iter()
            .map(|&nd| nodes[nd].height)
            .fold(0.0f32, f32::max);
        for &node in row {
            nodes[node].y = y + (row_h - nodes[node].height) / 2.0;
        }
        if r + 1 < rows.len() {
            let labeled = edges.iter().any(|e| {
                e.labeled
                    && ranks[e.from].min(ranks[e.to]) <= r
                    && ranks[e.from].max(ranks[e.to]) > r
            });
            y += row_h + vgap + if labeled { labeled_extra } else { 0.0 };
        } else {
            y += row_h;
        }
    }

    let width = nodes
        .iter()
        .map(|nd| nd.x + nd.width)
        .fold(0.0f32, f32::max);
    GraphLayoutResult {
        width,
        height: y,
        ranks,
    }
}

/// Clip the segment from an outside point `(fx, fy)` towards the center of a
/// rectangle, returning the intersection with the rectangle border.
pub fn clip_to_rect(fx: f32, fy: f32, x: f32, y: f32, w: f32, h: f32) -> (f32, f32) {
    clip_segment_to_rect(fx, fy, x + w / 2.0, y + h / 2.0, x, y, w, h)
}

/// Clip the segment from an outside point `(fx, fy)` to an inside point
/// `(tx, ty)`, returning the first intersection with the rectangle border.
#[allow(clippy::too_many_arguments)]
pub fn clip_segment_to_rect(
    fx: f32,
    fy: f32,
    tx: f32,
    ty: f32,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> (f32, f32) {
    let dx = tx - fx;
    let dy = ty - fy;
    if dx.abs() < 0.0001 && dy.abs() < 0.0001 {
        return (tx, ty);
    }
    // Smallest t in [0,1] along (fx,fy)→(cx,cy) that lands on a border.
    let mut best: Option<f32> = None;
    if dx.abs() > 0.0001 {
        for bx in [x, x + w] {
            let t = (bx - fx) / dx;
            if (0.0..=1.0).contains(&t) {
                let py = fy + dy * t;
                if py >= y - 0.01 && py <= y + h + 0.01 {
                    best = Some(best.map_or(t, |b: f32| b.min(t)));
                }
            }
        }
    }
    if dy.abs() > 0.0001 {
        for by in [y, y + h] {
            let t = (by - fy) / dy;
            if (0.0..=1.0).contains(&t) {
                let px = fx + dx * t;
                if px >= x - 0.01 && px <= x + w + 0.01 {
                    best = Some(best.map_or(t, |b: f32| b.min(t)));
                }
            }
        }
    }
    let t = best.unwrap_or(1.0);
    (fx + dx * t, fy + dy * t)
}

/// Liang-Barsky: does the segment (x1,y1)-(x2,y2) pass through the rectangle?
/// Touching the border only (shared endpoint on the border) does not count.
pub fn segment_intersects_rect(
    (x1, y1): (f32, f32),
    (x2, y2): (f32, f32),
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> bool {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    for (p, q) in [
        (-dx, x1 - x),
        (dx, x + w - x1),
        (-dy, y1 - y),
        (dy, y + h - y1),
    ] {
        if p.abs() < 0.0001 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    // Require a non-trivial overlap so border touches don't count.
    t1 - t0 > 0.02
}

/// Route a polyline from `start` to `end`, detouring around the first
/// obstacle rectangle `(x, y, w, h)` that the straight segment passes through.
pub fn route_with_detour(
    start: (f32, f32),
    end: (f32, f32),
    obstacles: &[(f32, f32, f32, f32)],
) -> Vec<(f32, f32)> {
    let mut points = vec![start];
    if let Some(&(ox, oy, ow, oh)) = obstacles
        .iter()
        .find(|&&(x, y, w, h)| segment_intersects_rect(start, end, x, y, w, h))
    {
        let line_x = (start.0 + end.0) / 2.0;
        let detour_x = if line_x <= ox + ow / 2.0 {
            ox - 15.0
        } else {
            ox + ow + 15.0
        };
        let top_y = oy - 10.0;
        let bot_y = oy + oh + 10.0;
        if start.1 <= end.1 {
            points.push((detour_x, top_y));
            points.push((detour_x, bot_y));
        } else {
            points.push((detour_x, bot_y));
            points.push((detour_x, top_y));
        }
    }
    points.push(end);
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_hierarchy() {
        let mut nodes = vec![
            GraphNode::new(100.0, 50.0),
            GraphNode::new(80.0, 50.0),
            GraphNode::new(80.0, 50.0),
        ];
        let edges = vec![
            GraphEdge {
                from: 0,
                to: 1,
                min_len: 1,
                labeled: false,
            },
            GraphEdge {
                from: 0,
                to: 2,
                min_len: 1,
                labeled: false,
            },
        ];
        let result = layout(&mut nodes, &edges, 35.0, 60.0, 17.0);
        assert_eq!(result.ranks, vec![0, 1, 1]);
        // Children below parent.
        assert!(nodes[1].y > nodes[0].y);
        assert!(nodes[2].y > nodes[0].y);
        // Parent roughly centered over children.
        let child_span_center = (nodes[1].center_x() + nodes[2].center_x()) / 2.0;
        assert!((nodes[0].center_x() - child_span_center).abs() < 30.0);
        // Vertical gap is 60.
        assert!((nodes[1].y - (nodes[0].y + nodes[0].height) - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_clip_to_rect_from_above() {
        // Line from (50, 0) to center of rect (0, 100, 100x50) → hits top border.
        let (px, py) = clip_to_rect(50.0, 0.0, 0.0, 100.0, 100.0, 50.0);
        assert!((py - 100.0).abs() < 0.1);
        assert!((px - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_clip_to_rect_angled() {
        let (px, py) = clip_to_rect(0.0, 0.0, 100.0, 100.0, 100.0, 100.0);
        // 45-degree line hits the corner region.
        assert!(px >= 99.0 && py >= 99.0);
    }

    #[test]
    fn test_same_rank_edge() {
        let mut nodes = vec![GraphNode::new(50.0, 30.0), GraphNode::new(50.0, 30.0)];
        let edges = vec![GraphEdge {
            from: 0,
            to: 1,
            min_len: 0,
            labeled: false,
        }];
        let result = layout(&mut nodes, &edges, 35.0, 60.0, 17.0);
        assert_eq!(result.ranks, vec![0, 0]);
        assert_eq!(nodes[0].y, nodes[1].y);
    }
}
