use crate::ast_rewriter::LayoutDirection;
use crate::engine::{
    ClassMemberInfo, DiagramNode, FlowEdge, NodeContractInfo, RelationKind, RenderedDiagram,
};
use crate::error::CoreError;
use crate::theme::ColorPalette;
use crate::xml_utils::escape_xml;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: String,
    pub stereotype: Option<String>,
    pub doc_comment: Option<String>,
    pub attributes: Vec<ClassMemberInfo>,
    pub methods: Vec<ClassMemberInfo>,
}

#[derive(Debug, Clone)]
pub struct ClassRelation {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
    pub label: Option<String>,
}

pub struct ClassDiagramParser;

fn parse_member(raw_line: &str) -> ClassMemberInfo {
    let mut s = raw_line.trim();

    // Extract comment if present (%% or //)
    let mut comment = None;
    if let Some(idx) = s.find("%%") {
        comment = Some(s[idx + 2..].trim().to_string());
        s = s[..idx].trim();
    } else if let Some(idx) = s.find("//") {
        comment = Some(s[idx + 2..].trim().to_string());
        s = s[..idx].trim();
    }

    // Extract visibility (+, -, #, ~)
    let mut vis = '+';
    if s.starts_with('+') || s.starts_with('-') || s.starts_with('#') || s.starts_with('~') {
        vis = s.chars().next().unwrap();
        s = s[1..].trim();
    }

    let is_method = s.contains('(');

    let (name, type_name) = if is_method {
        if let Some(paren_close) = s.find(')') {
            let sig = s[..=paren_close].trim();
            let ret = s[paren_close + 1..].trim();
            let ret_type = if ret.is_empty() {
                None
            } else {
                Some(ret.trim_start_matches(':').trim().to_string())
            };
            (sig.to_string(), ret_type)
        } else {
            (s.to_string(), None)
        }
    } else if let Some((n, t)) = s.split_once(':') {
        (n.trim().to_string(), Some(t.trim().to_string()))
    } else {
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() >= 2 {
            // First is type, rest is variable name
            (parts[1..].join(" "), Some(parts[0].to_string()))
        } else {
            (s.to_string(), None)
        }
    };

    ClassMemberInfo {
        visibility: vis,
        name,
        type_name,
        comment,
        is_method,
    }
}

impl ClassDiagramParser {
    pub fn is_class_diagram(source: &str) -> bool {
        source.lines().any(|l| {
            let t = l.trim();
            t.starts_with("classDiagram") || t.starts_with("classDiagram-v2")
        })
    }

    pub fn parse_and_render(
        source: &str,
        dir_override: Option<LayoutDirection>,
        palette: Option<&ColorPalette>,
    ) -> Result<RenderedDiagram, CoreError> {
        let default_palette = ColorPalette::default();
        let active_palette = palette.unwrap_or(&default_palette);
        let mut classes: HashMap<String, ClassDef> = HashMap::new();
        let mut relations: Vec<ClassRelation> = Vec::new();
        let mut current_dir = LayoutDirection::TD;

        let mut current_class_id: Option<String> = None;
        let mut pending_doc_comment: Option<String> = None;

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("%%") || trimmed.starts_with("//") {
                let comm = if let Some(stripped) = trimmed.strip_prefix("%%") {
                    stripped.trim().to_string()
                } else if let Some(stripped) = trimmed.strip_prefix("//") {
                    stripped.trim().to_string()
                } else {
                    String::new()
                };

                if !comm.is_empty() {
                    if let Some(ref class_id) = current_class_id {
                        if let Some(c) = classes.get_mut(class_id) {
                            if c.doc_comment.is_none()
                                && c.attributes.is_empty()
                                && c.methods.is_empty()
                            {
                                c.doc_comment = Some(comm);
                            }
                        }
                    } else {
                        pending_doc_comment = Some(comm);
                    }
                }
                continue;
            }

            if trimmed.starts_with("classDiagram") {
                continue;
            }

            if trimmed.starts_with("direction ") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    match parts[1] {
                        "LR" => current_dir = LayoutDirection::LR,
                        "RL" => current_dir = LayoutDirection::RL,
                        "BT" => current_dir = LayoutDirection::BT,
                        _ => current_dir = LayoutDirection::TD,
                    }
                }
                continue;
            }

            // End of class body
            if trimmed == "}" {
                current_class_id = None;
                continue;
            }

            // Inside class body: e.g. +String name, +deposit(amount) bool
            if let Some(ref class_id) = current_class_id {
                let member_str = trimmed.to_string();

                if member_str.starts_with("<<") && member_str.ends_with(">>") {
                    let stereo = member_str[2..member_str.len() - 2].trim().to_string();
                    if let Some(c) = classes.get_mut(class_id) {
                        c.stereotype = Some(stereo);
                    }
                    continue;
                }

                let member = parse_member(&member_str);
                if let Some(c) = classes.get_mut(class_id) {
                    if member.is_method {
                        c.methods.push(member);
                    } else {
                        c.attributes.push(member);
                    }
                }
                continue;
            }

            // Single line class with body: class Name {
            if trimmed.starts_with("class ") && trimmed.ends_with('{') {
                let inner = trimmed[6..trimmed.len() - 1].trim();
                let class_id = inner.split_whitespace().next().unwrap_or("").to_string();
                if !class_id.is_empty() {
                    let doc = pending_doc_comment.take();
                    classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id.clone(),
                        stereotype: None,
                        doc_comment: doc,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                    current_class_id = Some(class_id);
                }
                continue;
            }

            // Standalone stereotype: <<interface>> Shape
            if trimmed.starts_with("<<") && trimmed.contains(">>") {
                if let Some(end_idx) = trimmed.find(">>") {
                    let stereo = trimmed[2..end_idx].trim().to_string();
                    let target_class = trimmed[end_idx + 2..].trim();
                    if !target_class.is_empty() {
                        let doc = pending_doc_comment.take();
                        classes
                            .entry(target_class.to_string())
                            .and_modify(|c| c.stereotype = Some(stereo.clone()))
                            .or_insert_with(|| ClassDef {
                                id: target_class.to_string(),
                                stereotype: Some(stereo),
                                doc_comment: doc,
                                attributes: Vec::new(),
                                methods: Vec::new(),
                            });
                        continue;
                    }
                }
            }

            // Standalone class declaration: class Name
            if let Some(stripped) = trimmed.strip_prefix("class ") {
                let name = stripped.trim();
                let class_id = if let Some(colon_idx) = name.find(":::") {
                    name[..colon_idx].trim().to_string()
                } else {
                    name.split_whitespace().next().unwrap_or("").to_string()
                };
                if !class_id.is_empty() {
                    let doc = pending_doc_comment.take();
                    classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id,
                        stereotype: None,
                        doc_comment: doc,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                }
                continue;
            }

            // Relationship parsing: A <|-- B : label, A *-- B, etc.
            if let Some(rel) = Self::parse_relation(trimmed) {
                let doc_from = pending_doc_comment.take();
                classes.entry(rel.from.clone()).or_insert_with(|| ClassDef {
                    id: rel.from.clone(),
                    stereotype: None,
                    doc_comment: doc_from,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                });
                classes.entry(rel.to.clone()).or_insert_with(|| ClassDef {
                    id: rel.to.clone(),
                    stereotype: None,
                    doc_comment: None,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                });
                relations.push(rel);
                continue;
            }

            // Colon-style member definition: ClassName : +type name // comment
            if let Some((target_class, member_text)) = trimmed.split_once(':') {
                let class_id = target_class.trim().to_string();
                let member_s = member_text.trim();
                if !class_id.is_empty()
                    && !member_s.is_empty()
                    && !class_id.contains('<')
                    && !class_id.contains('>')
                    && !class_id.contains('-')
                    && !class_id.contains('*')
                    && !class_id.contains('o')
                    && !class_id.contains('.')
                {
                    let member = parse_member(member_s);
                    let doc = pending_doc_comment.take();
                    let entry = classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id,
                        stereotype: None,
                        doc_comment: doc,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                    if member.is_method {
                        entry.methods.push(member);
                    } else {
                        entry.attributes.push(member);
                    }
                    continue;
                }
            }
        }

        let direction = dir_override.unwrap_or(current_dir);
        Self::layout_and_render_svg(&classes, &relations, direction, active_palette)
    }

    fn parse_relation(line: &str) -> Option<ClassRelation> {
        let (rel_part, label) = if let Some(colon_idx) = line.find(':') {
            (
                line[..colon_idx].trim(),
                Some(line[colon_idx + 1..].trim().to_string()),
            )
        } else {
            (line.trim(), None)
        };

        let patterns = [
            ("<|--", RelationKind::Inheritance, true),
            ("--|>", RelationKind::Inheritance, false),
            ("..|>", RelationKind::Realization, false),
            ("<|..", RelationKind::Realization, true),
            ("*--", RelationKind::Composition, true),
            ("--*", RelationKind::Composition, false),
            ("o--", RelationKind::Aggregation, true),
            ("--o", RelationKind::Aggregation, false),
            ("-->", RelationKind::Association, true),
            ("<--", RelationKind::Association, false),
            ("..>", RelationKind::Dependency, true),
            ("<..", RelationKind::Dependency, false),
            ("--", RelationKind::Link, true),
        ];

        for &(token, kind, left_to_right) in &patterns {
            if let Some(idx) = rel_part.find(token) {
                let from = rel_part[..idx].trim().trim_matches('"').trim().to_string();
                let to = rel_part[idx + token.len()..]
                    .trim()
                    .trim_matches('"')
                    .trim()
                    .to_string();

                if !from.is_empty() && !to.is_empty() {
                    let (f, t) = if left_to_right {
                        (from, to)
                    } else {
                        (to, from)
                    };
                    return Some(ClassRelation {
                        from: f,
                        to: t,
                        kind,
                        label,
                    });
                }
            }
        }

        None
    }

    fn layout_and_render_svg(
        classes: &HashMap<String, ClassDef>,
        relations: &[ClassRelation],
        dir: LayoutDirection,
        palette: &ColorPalette,
    ) -> Result<RenderedDiagram, CoreError> {
        let mut class_list: Vec<&ClassDef> = classes.values().collect();
        class_list.sort_by(|a, b| a.id.cmp(&b.id));

        // Assign ranks based on relations
        let mut ranks: HashMap<String, usize> = HashMap::new();
        for r in relations {
            let from_rank = *ranks.get(&r.from).unwrap_or(&0);
            let to_rank = ranks.entry(r.to.clone()).or_insert(0);
            *to_rank = (*to_rank).max(from_rank + 1);
        }

        let mut rank_groups: HashMap<usize, Vec<&ClassDef>> = HashMap::new();
        for c in &class_list {
            let r = *ranks.get(&c.id).unwrap_or(&0);
            rank_groups.entry(r).or_default().push(c);
        }

        let mut max_rank = 0;
        for &r in rank_groups.keys() {
            if r > max_rank {
                max_rank = r;
            }
        }

        let mut nodes: Vec<DiagramNode> = Vec::new();
        let mut edges: Vec<FlowEdge> = Vec::new();

        let margin_x = 60.0f32;
        let margin_y = 60.0f32;
        let row_spacing = 80.0f32;
        let col_spacing = 60.0f32;

        let is_horizontal = dir == LayoutDirection::LR || dir == LayoutDirection::RL;
        let mut current_offset = if is_horizontal { margin_x } else { margin_y };
        let mut max_cross = 0.0f32;

        for r in 0..=max_rank {
            let group = rank_groups.get(&r).cloned().unwrap_or_default();
            if group.is_empty() {
                continue;
            }

            let mut cross_offset = if is_horizontal { margin_y } else { margin_x };
            let mut max_main_in_rank = 0.0f32;

            for class in group {
                let mut max_char_len = class.id.len();
                if let Some(ref d) = class.doc_comment {
                    max_char_len = max_char_len.max(d.len() + 4);
                }
                for attr in &class.attributes {
                    let mut len = 2 + attr.name.len();
                    if let Some(ref t) = attr.type_name {
                        len += t.len() + 1;
                    }
                    if let Some(ref c) = attr.comment {
                        len += c.len() + 5;
                    }
                    max_char_len = max_char_len.max(len);
                }
                for meth in &class.methods {
                    let mut len = 2 + meth.name.len();
                    if let Some(ref t) = meth.type_name {
                        len += t.len() + 3;
                    }
                    if let Some(ref c) = meth.comment {
                        len += c.len() + 5;
                    }
                    max_char_len = max_char_len.max(len);
                }

                let card_w = (max_char_len as f32 * 7.8 + 52.0).clamp(280.0, 720.0);
                let attr_count = class.attributes.len().max(1);
                let meth_count = class.methods.len().max(1);
                let class_header_h = if class.doc_comment.is_some() && class.stereotype.is_some() {
                    60.0f32
                } else if class.doc_comment.is_some() || class.stereotype.is_some() {
                    50.0f32
                } else {
                    42.0f32
                };
                let contract_h = 44.0f32; // Visible execution contract compartment
                let card_h = class_header_h
                    + contract_h
                    + (attr_count as f32 * 22.0)
                    + (meth_count as f32 * 22.0)
                    + 36.0;

                let (x, y) = if is_horizontal {
                    (current_offset, cross_offset)
                } else {
                    (cross_offset, current_offset)
                };

                nodes.push(DiagramNode {
                    id: class.id.clone(),
                    label: class.id.clone(),
                    stereotype: class.stereotype.clone(),
                    doc_comment: class.doc_comment.clone(),
                    lines: vec![escape_xml(&class.id)],
                    attributes: class.attributes.clone(),
                    methods: class.methods.clone(),
                    x,
                    y,
                    width: card_w,
                    height: card_h,
                    contract: Some(NodeContractInfo::default()),
                });

                if is_horizontal {
                    cross_offset += card_h + col_spacing;
                    max_main_in_rank = max_main_in_rank.max(card_w);
                    max_cross = max_cross.max(cross_offset);
                } else {
                    cross_offset += card_w + col_spacing;
                    max_main_in_rank = max_main_in_rank.max(card_h);
                    max_cross = max_cross.max(cross_offset);
                }
            }

            current_offset += max_main_in_rank + row_spacing;
        }

        for rel in relations {
            edges.push(FlowEdge {
                from: rel.from.clone(),
                to: rel.to.clone(),
                label: rel.label.clone(),
                dotted: rel.kind == RelationKind::Realization
                    || rel.kind == RelationKind::Dependency,
                thick: false,
                kind: rel.kind,
            });
        }

        let (total_w, total_h) = if is_horizontal {
            (
                (current_offset + margin_x).max(900.0),
                (max_cross + margin_y).max(600.0),
            )
        } else {
            (
                (max_cross + margin_x).max(900.0),
                (current_offset + margin_y).max(600.0),
            )
        };

        let mut diagram = RenderedDiagram {
            svg: String::new(),
            width: total_w,
            height: total_h,
            nodes,
            edges,
            is_class_diagram: true,
            direction: dir,
            selected_node_id: None,
        };

        diagram.regenerate_svg(palette);
        Ok(diagram)
    }
}
