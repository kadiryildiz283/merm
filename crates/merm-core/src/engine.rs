use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::ast_rewriter::LayoutDirection;
use crate::class_diagram::ClassDiagramParser;
use crate::error::CoreError;

pub struct LayoutEngine {
    timeout: Duration,
    #[allow(dead_code)]
    memory_limit_bytes: usize,
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
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(2000),
            memory_limit_bytes: 32 * 1024 * 1024, // 32 MB
        }
    }
}

impl LayoutEngine {
    pub fn new(timeout: Duration, memory_limit_bytes: usize) -> Self {
        Self {
            timeout,
            memory_limit_bytes,
        }
    }

    /// Renders a diagram with strict timeout watchdog protection.
    /// This prevents ReDoS or unbounded execution from freezing the process.
    pub fn render_with_watchdog(&self, source: &str) -> Result<RenderedDiagram, CoreError> {
        let (tx, rx) = channel();
        let source_owned = source.to_string();
        let timeout = self.timeout;

        thread::Builder::new()
            .name("layout-worker".to_string())
            .spawn(move || {
                let res = Self::render_internal(&source_owned);
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

    fn render_internal(source: &str) -> Result<RenderedDiagram, CoreError> {
        // 1. If it's a class diagram, dispatch to ClassDiagramParser
        if ClassDiagramParser::is_class_diagram(source) {
            return ClassDiagramParser::parse_and_render(source, None);
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

            // Extract node declarations with labels: A[Label], B(Round), C{Diamond}
            let mut rem = trimmed;
            while let Some(start) = rem.find(['[', '(', '{']) {
                let open_char = rem.as_bytes()[start] as char;
                let close_char = match open_char {
                    '[' => ']',
                    '(' => ')',
                    '{' => '}',
                    _ => ']',
                };

                if let Some(end) = rem[start..].find(close_char) {
                    let full_end = start + end;
                    let before = rem[..start].trim();
                    let node_id = before.split_whitespace().last().unwrap_or("").to_string();
                    let label = rem[start + 1..full_end].trim().to_string();

                    if !node_id.is_empty() && !nodes.iter().any(|n| n.id == node_id) {
                        nodes.push(DiagramNode {
                            id: node_id,
                            label,
                            x: 0.0,
                            y: 0.0,
                            width: 180.0,
                            height: 52.0,
                        });
                    }
                    rem = &rem[full_end + 1..];
                } else {
                    break;
                }
            }

            // Extract edges e.g. A --> B, A -->|label| B
            if trimmed.contains("-->") {
                let parts: Vec<&str> = trimmed.split("-->").collect();
                if parts.len() >= 2 {
                    let from_raw = parts[0].trim();
                    let from_id = from_raw
                        .split(|c: char| c == '[' || c == '(' || c == '{' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    let after_arrow = parts[1].trim();
                    let (edge_label, to_raw) = if let Some(stripped) = after_arrow.strip_prefix('|') {
                        if let Some(end_bar) = stripped.find('|') {
                            let lbl = stripped[..end_bar].trim().to_string();
                            let rest = stripped[end_bar + 1..].trim();
                            (Some(lbl), rest)
                        } else {
                            (None, after_arrow)
                        }
                    } else {
                        (None, after_arrow)
                    };

                    let to_id = to_raw.split(|c: char| c == '[' || c == '(' || c == '{' || c.is_whitespace())
                        .next()
                        .unwrap_or("")
                        .trim()
                        .to_string();

                    if !from_id.is_empty() && !nodes.iter().any(|n| n.id == from_id) {
                        nodes.push(DiagramNode {
                            id: from_id.clone(),
                            label: from_id.clone(),
                            x: 0.0,
                            y: 0.0,
                            width: 160.0,
                            height: 48.0,
                        });
                    }
                    if !to_id.is_empty() && !nodes.iter().any(|n| n.id == to_id) {
                        nodes.push(DiagramNode {
                            id: to_id.clone(),
                            label: to_id.clone(),
                            x: 0.0,
                            y: 0.0,
                            width: 160.0,
                            height: 48.0,
                        });
                    }

                    if !from_id.is_empty() && !to_id.is_empty() {
                        edges.push(FlowEdge {
                            from: from_id,
                            to: to_id,
                            label: edge_label,
                        });
                    }
                }
            }
        }

        // Layout node positions according to direction (TD or LR)
        let is_horizontal = dir == LayoutDirection::LR || dir == LayoutDirection::RL;
        let mut cur_x = 60.0f32;
        let mut cur_y = 60.0f32;
        let spacing = 80.0f32;

        for node in &mut nodes {
            node.x = cur_x;
            node.y = cur_y;

            if is_horizontal {
                cur_x += node.width + spacing;
            } else {
                cur_y += node.height + spacing;
            }
        }

        let total_width = if is_horizontal { cur_x + 100.0 } else { 800.0f32 };
        let total_height = if is_horizontal { 600.0f32 } else { cur_y + 100.0 };

        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"##,
            total_width, total_height, total_width, total_height
        );

        // Marker for arrow
        svg.push_str(r##"
        <defs>
            <marker id="flow-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto">
                <path d="M 0 1 L 10 5 L 0 9 z" fill="#89b4fa"/>
            </marker>
        </defs>
        <rect width="100%" height="100%" fill="#1e1e2e"/>
        "##);

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

                let mid_x = (sx + ex) / 2.0;
                let mid_y = (sy + ey) / 2.0;

                svg.push_str(&format!(
                    r##"<path d="M {} {} C {} {}, {} {}, {} {}" fill="none" stroke="#89b4fa" stroke-width="2" marker-end="url(#flow-arrow)"/>"##,
                    sx, sy, mid_x, sy, mid_x, ey, ex, ey
                ));

                if let Some(ref lbl) = edge.label {
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="18" rx="4" fill="#181825" opacity="0.9"/>
                        <text x="{}" y="{}" fill="#cdd6f4" font-family="monospace" font-size="11" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        mid_x - (lbl.len() * 4) as f32 - 4.0, mid_y - 9.0, (lbl.len() * 8 + 8) as f32,
                        mid_x, mid_y, lbl
                    ));
                }
            }
        }

        // Draw nodes
        for node in &nodes {
            svg.push_str(&format!(
                r##"<g id="node_{}" class="node">
                <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="#313244" stroke="#89b4fa" stroke-width="2"/>
                <text x="{}" y="{}" fill="#cdd6f4" font-family="monospace" font-size="14" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>
                </g>"##,
                node.id,
                node.x,
                node.y,
                node.width,
                node.height,
                node.x + node.width / 2.0,
                node.y + node.height / 2.0,
                node.label
            ));
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
