use crate::error::CoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramBlock {
    pub diagram_type: DiagramType,
    pub source: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagramType {
    Flowchart,
    Sequence,
    Class,
    State,
    Er,
    Unknown,
}

impl DiagramType {
    pub fn from_source(source: &str) -> Self {
        let trimmed = source.trim_start();
        if trimmed.starts_with("flowchart") || trimmed.starts_with("graph") {
            DiagramType::Flowchart
        } else if trimmed.starts_with("sequenceDiagram") {
            DiagramType::Sequence
        } else if trimmed.starts_with("classDiagram") {
            DiagramType::Class
        } else if trimmed.starts_with("stateDiagram") || trimmed.starts_with("stateDiagram-v2") {
            DiagramType::State
        } else if trimmed.starts_with("erDiagram") {
            DiagramType::Er
        } else {
            DiagramType::Unknown
        }
    }
}

pub struct DiagramExtractor;

impl DiagramExtractor {
    /// Extracts all ```mermaid ... ``` code blocks from markdown text.
    /// If the input is not markdown but raw mermaid code, wraps it as a single block.
    pub fn extract(content: &str) -> Result<Vec<DiagramBlock>, CoreError> {
        let mut blocks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut in_mermaid_block = false;
        let mut current_block_lines = Vec::new();
        let mut block_start_line = 0;

        for (idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            if trimmed.starts_with("```mermaid") {
                in_mermaid_block = true;
                block_start_line = idx + 1;
                current_block_lines.clear();
            } else if in_mermaid_block && trimmed.starts_with("```") {
                in_mermaid_block = false;
                let source = current_block_lines.join("\n");
                let diagram_type = DiagramType::from_source(&source);
                blocks.push(DiagramBlock {
                    diagram_type,
                    source,
                    start_line: block_start_line,
                    end_line: idx + 1,
                });
            } else if in_mermaid_block {
                current_block_lines.push(*line);
            }
        }

        // If no markdown blocks were found, but the raw text itself is a Mermaid diagram
        if blocks.is_empty() {
            let trimmed = content.trim();
            let dtype = DiagramType::from_source(trimmed);
            if dtype != DiagramType::Unknown {
                blocks.push(DiagramBlock {
                    diagram_type: dtype,
                    source: content.to_string(),
                    start_line: 1,
                    end_line: lines.len().max(1),
                });
            } else {
                return Err(CoreError::NoDiagramFound);
            }
        }

        Ok(blocks)
    }
}
