#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    TD, // Top-to-Bottom
    TB, // Top-to-Bottom (alias)
    BT, // Bottom-to-Top
    LR, // Left-to-Right
    RL, // Right-to-Left
}

impl LayoutDirection {
    pub fn next(&self) -> Self {
        match self {
            LayoutDirection::TD | LayoutDirection::TB => LayoutDirection::LR,
            LayoutDirection::LR => LayoutDirection::RL,
            LayoutDirection::RL => LayoutDirection::BT,
            LayoutDirection::BT => LayoutDirection::TD,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            LayoutDirection::TD => "TD",
            LayoutDirection::TB => "TB",
            LayoutDirection::BT => "BT",
            LayoutDirection::LR => "LR",
            LayoutDirection::RL => "RL",
        }
    }
}

pub struct AstRewriter;

impl AstRewriter {
    /// Pivots diagram layout direction (e.g. flowchart TD -> flowchart LR, or adds/modifies direction LR in classDiagram).
    pub fn pivot_direction(source: &str, target_dir: LayoutDirection) -> String {
        let mut rewritten = Vec::new();
        let mut direction_replaced = false;
        let mut is_class_diagram = false;

        for line in source.lines() {
            let trimmed = line.trim();

            if trimmed.starts_with("classDiagram") {
                is_class_diagram = true;
                rewritten.push(line.to_string());
                continue;
            }

            if trimmed.starts_with("direction ") {
                rewritten.push(format!("    direction {}", target_dir.as_str()));
                direction_replaced = true;
                continue;
            }

            if !direction_replaced
                && (trimmed.starts_with("flowchart") || trimmed.starts_with("graph"))
            {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    let cmd = parts[0];
                    rewritten.push(format!("{} {}", cmd, target_dir.as_str()));
                    direction_replaced = true;
                    continue;
                }
            }

            rewritten.push(line.to_string());
        }

        // If it was a classDiagram without a direction line, inject it right after classDiagram
        if is_class_diagram && !direction_replaced {
            let mut final_lines = Vec::new();
            for line in rewritten {
                final_lines.push(line.clone());
                if line.trim().starts_with("classDiagram") {
                    final_lines.push(format!("    direction {}", target_dir.as_str()));
                }
            }
            return final_lines.join("\n");
        }

        rewritten.join("\n")
    }

    /// Isolates an ego-network around a target node. Keeps diagram header and lines containing the node ID.
    pub fn isolate_node(source: &str, target_node: &str) -> String {
        let mut filtered = Vec::new();
        let mut header_passed = false;

        for line in source.lines() {
            let trimmed = line.trim();
            if !header_passed {
                filtered.push(line.to_string());
                if trimmed.starts_with("flowchart")
                    || trimmed.starts_with("graph")
                    || trimmed.starts_with("classDiagram")
                    || trimmed.starts_with("sequenceDiagram")
                {
                    header_passed = true;
                }
                continue;
            }

            if line.contains(target_node) {
                filtered.push(line.to_string());
            }
        }

        filtered.join("\n")
    }
}
