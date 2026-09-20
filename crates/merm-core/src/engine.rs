use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::Duration;

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
    /// This prevents ReDoS or unbounded JavaScript engine execution from freezing the process.
    pub fn render_with_watchdog(&self, source: &str) -> Result<RenderedDiagram, CoreError> {
        let (tx, rx) = channel();
        let source_owned = source.to_string();
        let timeout = self.timeout;

        thread::Builder::new()
            .name("quickjs-layout-worker".to_string())
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
        // Mock / deterministic synthetic DOM layout engine producing valid SVG
        let lines: Vec<&str> = source.lines().collect();
        let mut nodes = Vec::new();
        let mut current_y = 40.0;

        for line in lines.iter() {
            let trimmed = line.trim();
            if trimmed.is_empty()
                || trimmed.starts_with("flowchart")
                || trimmed.starts_with("graph")
                || trimmed.starts_with("sequenceDiagram")
                || trimmed.starts_with("%%")
            {
                continue;
            }

            // Extract all bracketed nodes e.g. NodeId[Label]
            let mut rem = trimmed;
            let mut found_bracket_node = false;

            while let Some(b_start) = rem.find('[') {
                if let Some(b_end) = rem[b_start..].find(']') {
                    let full_b_end = b_start + b_end;
                    let before = rem[..b_start].trim();
                    let node_id = before.split_whitespace().last().unwrap_or("").to_string();
                    let label = rem[b_start + 1..full_b_end].trim().to_string();

                    if !node_id.is_empty() && !nodes.iter().any(|n: &DiagramNode| n.id == node_id) {
                        nodes.push(DiagramNode {
                            id: node_id,
                            label,
                            x: 100.0,
                            y: current_y,
                            width: 160.0,
                            height: 48.0,
                        });
                        current_y += 70.0;
                        found_bracket_node = true;
                    }
                    rem = &rem[full_b_end + 1..];
                } else {
                    break;
                }
            }

            if !found_bracket_node && trimmed.contains("-->") {
                let parts: Vec<&str> = trimmed.split("-->").collect();
                if parts.len() == 2 {
                    let from_id = parts[0].trim().to_string();
                    let to_id = parts[1].trim().to_string();
                    if !from_id.is_empty() && !nodes.iter().any(|n| n.id == from_id) {
                        nodes.push(DiagramNode {
                            id: from_id.clone(),
                            label: from_id,
                            x: 50.0,
                            y: current_y,
                            width: 120.0,
                            height: 40.0,
                        });
                        current_y += 60.0;
                    }
                    if !to_id.is_empty() && !nodes.iter().any(|n| n.id == to_id) {
                        nodes.push(DiagramNode {
                            id: to_id.clone(),
                            label: to_id,
                            x: 250.0,
                            y: current_y,
                            width: 120.0,
                            height: 40.0,
                        });
                        current_y += 60.0;
                    }
                }
            }
        }

        let total_width = 800.0f32;
        let total_height = current_y + 60.0;

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"#,
            total_width, total_height, total_width, total_height
        );
        svg.push_str(r##"<rect width="100%" height="100%" fill="#1e1e2e"/>"##);

        for node in &nodes {
            svg.push_str(&format!(
                r##"<g id="node_{}" class="node"><rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="#313244" stroke="#89b4fa" stroke-width="2"/><text x="{}" y="{}" fill="#cdd6f4" font-family="monospace" font-size="14" text-anchor="middle" dominant-baseline="middle">{}</text></g>"##,
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
