use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::ast_rewriter::LayoutDirection;
use crate::class_diagram::ClassDiagramParser;
use crate::error::CoreError;
use crate::theme::ColorPalette;
use crate::xml_utils::{escape_xml, split_and_escape_lines};

pub struct LayoutEngine {
    timeout: Duration,
    #[allow(dead_code)]
    memory_limit_bytes: usize,
    pub palette: ColorPalette,
}

#[derive(Debug, Clone)]
pub struct RenderedDiagram {
    pub svg: String,
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<DiagramNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    pub lines: Vec<String>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone)]
struct FlowEdge {
    from: String,
    to: String,
    label: Option<String>,
    dotted: bool,
    thick: bool,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(2000),
            memory_limit_bytes: 32 * 1024 * 1024, // 32 MB
            palette: ColorPalette::default(),
        }
    }
}

fn clean_node_id(raw: &str) -> String {
    let trimmed = raw.trim();
    let token = trimmed
        .split(|c: char| c == '[' || c == '(' || c == '{' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .trim();
    token.split(":::").next().unwrap_or(token).trim().to_string()
}

fn clean_label(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_edge(trimmed: &str) -> Option<FlowEdge> {
    // 1. Dotted arrow with label: A -. "label" .-> B or A -. label .-> B
    if let Some(p1) = trimmed.find("-.") {
        if let Some(p2) = trimmed[p1 + 2..].find(".->") {
            let from = clean_node_id(&trimmed[..p1]);
            let label = clean_label(&trimmed[p1 + 2..p1 + 2 + p2]);
            let to = clean_node_id(&trimmed[p1 + 2 + p2 + 3..]);
            if !from.is_empty() && !to.is_empty() {
                return Some(FlowEdge {
                    from,
                    to,
                    label,
                    dotted: true,
                    thick: false,
                });
            }
        }
    }

    // 2. Dotted arrow without label: A -.-> B
    if let Some((from_raw, to_raw)) = trimmed.split_once("-.->") {
        let from = clean_node_id(from_raw);
        let to = clean_node_id(to_raw);
        if !from.is_empty() && !to.is_empty() {
            return Some(FlowEdge {
                from,
                to,
                label: None,
                dotted: true,
                thick: false,
            });
        }
    }

    // 3. Thick arrow with label: A == "label" ==> B or A == label ==> B
    if let Some(p1) = trimmed.find("==") {
        if let Some(p2) = trimmed[p1 + 2..].find("==>") {
            let from = clean_node_id(&trimmed[..p1]);
            let label = clean_label(&trimmed[p1 + 2..p1 + 2 + p2]);
            let to = clean_node_id(&trimmed[p1 + 2 + p2 + 3..]);
            if !from.is_empty() && !to.is_empty() {
                return Some(FlowEdge {
                    from,
                    to,
                    label,
                    dotted: false,
                    thick: true,
                });
            }
        }
    }

    // 4. Thick arrow with pipe label or plain: A ==>|label| B or A ==> B
    if let Some((from_raw, rest)) = trimmed.split_once("==>") {
        let rest = rest.trim();
        if let Some(stripped) = rest.strip_prefix('|') {
            if let Some(end_bar) = stripped.find('|') {
                let label = clean_label(&stripped[..end_bar]);
                let to = clean_node_id(&stripped[end_bar + 1..]);
                let from = clean_node_id(from_raw);
                if !from.is_empty() && !to.is_empty() {
                    return Some(FlowEdge {
                        from,
                        to,
                        label,
                        dotted: false,
                        thick: true,
                    });
                }
            }
        } else {
            let from = clean_node_id(from_raw);
            let to = clean_node_id(rest);
            if !from.is_empty() && !to.is_empty() {
                return Some(FlowEdge {
                    from,
                    to,
                    label: None,
                    dotted: false,
                    thick: true,
                });
            }
        }
    }

    // 5. Normal arrow with label between dashes: A -- "label" --> B or A -- label --> B
    if let Some(p1) = trimmed.find("--") {
        if let Some(p2) = trimmed[p1 + 2..].find("-->") {
            let from = clean_node_id(&trimmed[..p1]);
            let label = clean_label(&trimmed[p1 + 2..p1 + 2 + p2]);
            let to = clean_node_id(&trimmed[p1 + 2 + p2 + 3..]);
            if !from.is_empty() && !to.is_empty() {
                return Some(FlowEdge {
                    from,
                    to,
                    label,
                    dotted: false,
                    thick: false,
                });
            }
        }
    }

    // 6. Normal arrow with pipe label or plain: A -->|label| B or A --> B
    if let Some((from_raw, rest)) = trimmed.split_once("-->") {
        let rest = rest.trim();
        if let Some(stripped) = rest.strip_prefix('|') {
            if let Some(end_bar) = stripped.find('|') {
                let label = clean_label(&stripped[..end_bar]);
                let to = clean_node_id(&stripped[end_bar + 1..]);
                let from = clean_node_id(from_raw);
                if !from.is_empty() && !to.is_empty() {
                    return Some(FlowEdge {
                        from,
                        to,
                        label,
                        dotted: false,
                        thick: false,
                    });
                }
            }
        } else {
            let from = clean_node_id(from_raw);
            let to = clean_node_id(rest);
            if !from.is_empty() && !to.is_empty() {
                return Some(FlowEdge {
                    from,
                    to,
                    label: None,
                    dotted: false,
                    thick: false,
                });
            }
        }
    }

    // 7. Plain link: A --- B
    if let Some((from_raw, to_raw)) = trimmed.split_once("---") {
        let from = clean_node_id(from_raw);
        let to = clean_node_id(to_raw);
        if !from.is_empty() && !to.is_empty() {
            return Some(FlowEdge {
                from,
                to,
                label: None,
                dotted: false,
                thick: false,
            });
        }
    }

    None
}

fn extract_nodes_from_line(line: &str, nodes: &mut Vec<DiagramNode>) {
    let mut rem = line;
    while let Some(start) = rem.find(['[', '(', '{']) {
        let open_char = rem.as_bytes()[start] as char;
        let close_char = match open_char {
            '[' => ']',
            '(' => ')',
            '{' => '}',
            _ => ']',
        };

        let after_open = &rem[start + 1..];
        let end = if let Some(stripped) = after_open.strip_prefix('"') {
            if let Some(q_end) = stripped.find('"') {
                let search_start = 1 + q_end + 1;
                after_open[search_start..]
                    .find(close_char)
                    .map(|pos| search_start + pos)
            } else {
                after_open.find(close_char)
            }
        } else {
            after_open.find(close_char)
        };

        if let Some(end_offset) = end {
            let full_end = start + 1 + end_offset;
            let before = rem[..start].trim();
            let raw_id = before.split_whitespace().last().unwrap_or("").trim();
            let node_id = clean_node_id(raw_id);

            let raw_label = rem[start + 1..full_end].trim();
            let clean_lbl = raw_label.trim_matches('"').trim_matches('\'').trim();

            if !node_id.is_empty() {
                let lines = split_and_escape_lines(clean_lbl);
                let lines = if lines.is_empty() {
                    vec![escape_xml(clean_lbl)]
                } else {
                    lines
                };

                let line_count = lines.len();
                let max_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(8);
                let width = (max_len as f32 * 7.5 + 36.0).clamp(180.0, 520.0);
                let height = (line_count as f32 * 18.0 + 26.0).max(52.0);

                if let Some(existing) = nodes.iter_mut().find(|n| n.id == node_id) {
                    existing.label = clean_lbl.to_string();
                    existing.lines = lines;
                    existing.width = width;
                    existing.height = height;
                } else {
                    nodes.push(DiagramNode {
                        id: node_id,
                        label: clean_lbl.to_string(),
                        lines,
                        x: 0.0,
                        y: 0.0,
                        width,
                        height,
                    });
                }
            }
            rem = &rem[full_end + 1..];
        } else {
            break;
        }
    }
}

impl LayoutEngine {
    pub fn new(timeout: Duration, memory_limit_bytes: usize, palette: ColorPalette) -> Self {
        Self {
            timeout,
            memory_limit_bytes,
            palette,
        }
    }

    /// Renders a diagram with strict timeout watchdog protection.
    /// This prevents ReDoS or unbounded execution from freezing the process.
    pub fn render_with_watchdog(&self, source: &str) -> Result<RenderedDiagram, CoreError> {
        let (tx, rx) = channel();
        let source_owned = source.to_string();
        let timeout = self.timeout;
        let palette = self.palette.clone();

        thread::Builder::new()
            .name("layout-worker".to_string())
            .spawn(move || {
                let res = Self::render_internal(&source_owned, &palette);
                let _ = tx.send(res);
            })
            .map_err(|_| CoreError::SyntaxError("Failed to spawn layout thread".to_string()))?;

        match rx.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(CoreError::ExecutionTimeout(timeout)),
            Err(RecvTimeoutError::Disconnected) => {
                Err(CoreError::SyntaxError("Layout worker crashed".to_string()))
            }
        }
    }

    fn render_internal(source: &str, palette: &ColorPalette) -> Result<RenderedDiagram, CoreError> {
        // 1. If it's a class diagram, dispatch to ClassDiagramParser
        if ClassDiagramParser::is_class_diagram(source) {
            return ClassDiagramParser::parse_and_render(source, None, Some(palette));
        }

        // 2. Flowchart / General diagram layout
        let mut nodes: Vec<DiagramNode> = Vec::new();
        let mut edges: Vec<FlowEdge> = Vec::new();
        let mut dir = LayoutDirection::TD;

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
                continue;
            }

            if trimmed.starts_with("flowchart") || trimmed.starts_with("graph") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    match parts[1] {
                        "LR" => dir = LayoutDirection::LR,
                        "RL" => dir = LayoutDirection::RL,
                        "BT" => dir = LayoutDirection::BT,
                        _ => dir = LayoutDirection::TD,
                    }
                }
                continue;
            }

            // Skip non-node directives
            if trimmed.starts_with("classDef")
                || trimmed.starts_with("class ")
                || trimmed.starts_with("style ")
                || trimmed.starts_with("linkStyle")
                || trimmed.starts_with("subgraph")
                || trimmed == "end"
                || trimmed.starts_with("end ")
                || trimmed.starts_with("end;")
                || trimmed.starts_with("click ")
                || trimmed.starts_with("accTitle")
                || trimmed.starts_with("accDescr")
            {
                continue;
            }

            // Extract node declarations with labels
            extract_nodes_from_line(trimmed, &mut nodes);

            // Extract edges
            if let Some(edge) = parse_edge(trimmed) {
                if !nodes.iter().any(|n| n.id == edge.from) {
                    let esc = escape_xml(&edge.from);
                    nodes.push(DiagramNode {
                        id: edge.from.clone(),
                        label: edge.from.clone(),
                        lines: vec![esc],
                        x: 0.0,
                        y: 0.0,
                        width: 180.0,
                        height: 52.0,
                    });
                }
                if !nodes.iter().any(|n| n.id == edge.to) {
                    let esc = escape_xml(&edge.to);
                    nodes.push(DiagramNode {
                        id: edge.to.clone(),
                        label: edge.to.clone(),
                        lines: vec![esc],
                        x: 0.0,
                        y: 0.0,
                        width: 180.0,
                        height: 52.0,
                    });
                }
                edges.push(edge);
            }
        }

        if nodes.is_empty() {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><rect width="100%" height="100%" fill="{}"/><text x="400" y="300" fill="{}" font-family="monospace, sans-serif" font-size="16" text-anchor="middle">No elements to display</text></svg>"##,
                palette.background, palette.text_main
            );
            return Ok(RenderedDiagram {
                svg,
                width: 800.0,
                height: 600.0,
                nodes,
            });
        }

        let is_horizontal = dir == LayoutDirection::LR || dir == LayoutDirection::RL;

        // Assign ranks via longest path relaxation with cycle limit
        let mut ranks: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for node in &nodes {
            ranks.insert(node.id.clone(), 0);
        }

        let max_iters = nodes.len();
        for _ in 0..max_iters {
            let mut changed = false;
            for edge in &edges {
                if let Some(&r_from) = ranks.get(&edge.from) {
                    if let Some(r_to) = ranks.get_mut(&edge.to) {
                        if *r_to <= r_from {
                            *r_to = r_from + 1;
                            changed = true;
                        }
                    }
                }
            }
            if !changed {
                break;
            }
        }

        let max_rank = ranks.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
        for (idx, node) in nodes.iter().enumerate() {
            let r = ranks.get(&node.id).copied().unwrap_or(0);
            layers[r].push(idx);
        }

        let margin_x = 80.0f32;
        let margin_y = 60.0f32;
        let h_gap = 40.0f32;
        let v_gap = 60.0f32;

        let total_width;
        let total_height;

        if !is_horizontal {
            // TD Layout
            let mut max_row_width = 0.0f32;
            for layer in &layers {
                if layer.is_empty() {
                    continue;
                }
                let row_w: f32 = layer.iter().map(|&i| nodes[i].width).sum::<f32>()
                    + (layer.len().saturating_sub(1) as f32 * h_gap);
                max_row_width = max_row_width.max(row_w);
            }
            total_width = (max_row_width + 2.0 * margin_x).max(1000.0);

            let mut cur_y = margin_y;
            for layer in &layers {
                if layer.is_empty() {
                    continue;
                }
                let row_h: f32 = layer
                    .iter()
                    .map(|&i| nodes[i].height)
                    .fold(0.0f32, f32::max);
                let row_w: f32 = layer.iter().map(|&i| nodes[i].width).sum::<f32>()
                    + (layer.len().saturating_sub(1) as f32 * h_gap);

                let offset_x = (total_width - row_w) / 2.0;
                let mut cur_x = offset_x;
                for &idx in layer {
                    nodes[idx].x = cur_x;
                    nodes[idx].y = cur_y + (row_h - nodes[idx].height) / 2.0;
                    cur_x += nodes[idx].width + h_gap;
                }
                cur_y += row_h + v_gap;
            }
            total_height = (cur_y - v_gap + margin_y).max(700.0);
        } else {
            // LR Layout
            let mut max_col_height = 0.0f32;
            for layer in &layers {
                if layer.is_empty() {
                    continue;
                }
                let col_h: f32 = layer.iter().map(|&i| nodes[i].height).sum::<f32>()
                    + (layer.len().saturating_sub(1) as f32 * v_gap);
                max_col_height = max_col_height.max(col_h);
            }
            total_height = (max_col_height + 2.0 * margin_y).max(800.0);

            let mut cur_x = margin_x;
            for layer in &layers {
                if layer.is_empty() {
                    continue;
                }
                let col_w: f32 = layer
                    .iter()
                    .map(|&i| nodes[i].width)
                    .fold(0.0f32, f32::max);
                let col_h: f32 = layer.iter().map(|&i| nodes[i].height).sum::<f32>()
                    + (layer.len().saturating_sub(1) as f32 * v_gap);

                let offset_y = (total_height - col_h) / 2.0;
                let mut cur_y = offset_y;
                for &idx in layer {
                    nodes[idx].x = cur_x + (col_w - nodes[idx].width) / 2.0;
                    nodes[idx].y = cur_y;
                    cur_y += nodes[idx].height + v_gap;
                }
                cur_x += col_w + h_gap;
            }
            total_width = (cur_x - h_gap + margin_x).max(1000.0);
        }

        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"##,
            total_width, total_height, total_width, total_height
        );

        // Marker for arrow using theme
        svg.push_str(&format!(
            r##"
        <defs>
            <marker id="flow-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto">
                <path d="M 0 1 L 10 5 L 0 9 z" fill="{}"/>
            </marker>
        </defs>
        <rect width="100%" height="100%" fill="{}"/>
        "##,
            palette.edge_stroke, palette.background
        ));

        // Draw edges
        for edge in &edges {
            let from_node = nodes.iter().find(|n| n.id == edge.from);
            let to_node = nodes.iter().find(|n| n.id == edge.to);

            if let (Some(f), Some(t)) = (from_node, to_node) {
                let (sx, sy, ex, ey) = if is_horizontal {
                    (f.x + f.width, f.y + f.height / 2.0, t.x, t.y + t.height / 2.0)
                } else {
                    (f.x + f.width / 2.0, f.y + f.height, t.x + t.width / 2.0, t.y)
                };

                let dash = if edge.dotted {
                    r#"stroke-dasharray="6,4" "#
                } else {
                    ""
                };
                let width = if edge.thick { "3.5" } else { "2" };

                let (c1x, c1y, c2x, c2y) = if is_horizontal {
                    if ex > sx {
                        let mid_x = (sx + ex) / 2.0;
                        (mid_x, sy, mid_x, ey)
                    } else {
                        let sweep_y = sy.max(ey) + 80.0;
                        (sx, sweep_y, ex, sweep_y)
                    }
                } else if ey > sy {
                    let mid_y = (sy + ey) / 2.0;
                    (sx, mid_y, ex, mid_y)
                } else {
                    let sweep_x = sx.max(ex) + 80.0;
                    (sweep_x, sy, sweep_x, ey)
                };

                let mid_x = (sx + ex) / 2.0;
                let mid_y = (sy + ey) / 2.0;

                svg.push_str(&format!(
                    r##"<path d="M {} {} C {} {}, {} {}, {} {}" fill="none" stroke="{}" stroke-width="{}" {}marker-end="url(#flow-arrow)"/>"##,
                    sx, sy, c1x, c1y, c2x, c2y, ex, ey, palette.edge_stroke, width, dash
                ));

                if let Some(ref lbl) = edge.label {
                    let escaped_lbl = escape_xml(lbl);
                    let badge_w = (escaped_lbl.len() as f32 * 7.5 + 16.0).max(40.0);
                    let badge_h = 20.0f32;
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" stroke="{}" stroke-width="1" opacity="0.95"/>
                        <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="11" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        mid_x - badge_w / 2.0,
                        mid_y - badge_h / 2.0,
                        badge_w,
                        badge_h,
                        palette.badge_bg,
                        palette.border,
                        mid_x,
                        mid_y,
                        palette.text_main,
                        escaped_lbl
                    ));
                }
            }
        }

        // Draw nodes
        for node in &nodes {
            svg.push_str(&format!(
                r##"<g id="node_{}" class="node">
                <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="2"/>"##,
                escape_xml(&node.id),
                node.x,
                node.y,
                node.width,
                node.height,
                palette.card_bg,
                palette.border
            ));

            let center_x = node.x + node.width / 2.0;
            if node.lines.len() <= 1 {
                let text = node
                    .lines
                    .first()
                    .cloned()
                    .unwrap_or_else(|| escape_xml(&node.label));
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="13" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                    center_x,
                    node.y + node.height / 2.0,
                    palette.text_main,
                    text
                ));
            } else {
                let line_height = 18.0f32;
                let start_y =
                    node.y + (node.height - (node.lines.len() as f32 * line_height)) / 2.0 + 10.0;
                for (idx, line) in node.lines.iter().enumerate() {
                    let y_pos = start_y + (idx as f32 * line_height);
                    let (weight, size, fill) = if idx == 0 {
                        ("bold", "13", &palette.text_main)
                    } else {
                        ("normal", "11", &palette.text_sub)
                    };
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="{}" font-weight="{}" text-anchor="middle">{}</text>"##,
                        center_x, y_pos, fill, size, weight, line
                    ));
                }
            }

            svg.push_str("</g>\n");
        }

        svg.push_str("</svg>");

        Ok(RenderedDiagram {
            svg,
            width: total_width,
            height: total_height,
            nodes,
        })
    }
}
