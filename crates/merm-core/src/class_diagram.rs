use std::collections::HashMap;
use crate::ast_rewriter::LayoutDirection;
use crate::engine::{DiagramNode, RenderedDiagram};
use crate::error::CoreError;
use crate::theme::ColorPalette;

#[derive(Debug, Clone)]
pub struct ClassMember {
    pub visibility: char, // '+', '-', '#', '~'
    pub raw: String,
    pub is_method: bool,
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub id: String,
    pub stereotype: Option<String>,
    pub attributes: Vec<ClassMember>,
    pub methods: Vec<ClassMember>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationKind {
    Inheritance,    // <|-- or --|>
    Realization,    // ..|> or <|..
    Composition,    // *-- or --*
    Aggregation,    // o-- or --o
    Association,    // --> or <--
    Dependency,     // ..> or <..
    Link,           // --
}

#[derive(Debug, Clone)]
pub struct ClassRelation {
    pub from: String,
    pub to: String,
    pub kind: RelationKind,
    pub label: Option<String>,
}

pub struct ClassDiagramParser;

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

        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("%%") {
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
                let mut member_str = trimmed.to_string();
                let mut vis = '+';
                if member_str.starts_with('+') || member_str.starts_with('-') || member_str.starts_with('#') || member_str.starts_with('~') {
                    vis = member_str.chars().next().unwrap();
                    member_str = member_str[1..].trim().to_string();
                }

                if member_str.starts_with("<<") && member_str.ends_with(">>") {
                    let stereo = member_str[2..member_str.len() - 2].trim().to_string();
                    if let Some(c) = classes.get_mut(class_id) {
                        c.stereotype = Some(stereo);
                    }
                    continue;
                }

                let is_method = member_str.contains('(');
                let member = ClassMember {
                    visibility: vis,
                    raw: member_str,
                    is_method,
                };

                if let Some(c) = classes.get_mut(class_id) {
                    if is_method {
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
                    classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id.clone(),
                        stereotype: None,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                    current_class_id = Some(class_id);
                }
                continue;
            }

            // Standalone class declaration: class Name
            if let Some(stripped) = trimmed.strip_prefix("class ") {
                let name = stripped.trim();
                if let Some(colon_idx) = name.find(":::") {
                    let class_id = name[..colon_idx].trim().to_string();
                    classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id,
                        stereotype: None,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                } else {
                    let class_id = name.split_whitespace().next().unwrap_or("").to_string();
                    classes.entry(class_id.clone()).or_insert_with(|| ClassDef {
                        id: class_id,
                        stereotype: None,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                    });
                }
                continue;
            }

            // Relationship parsing: A <|-- B : label, A *-- B, etc.
            if let Some(rel) = Self::parse_relation(trimmed) {
                classes.entry(rel.from.clone()).or_insert_with(|| ClassDef {
                    id: rel.from.clone(),
                    stereotype: None,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                });
                classes.entry(rel.to.clone()).or_insert_with(|| ClassDef {
                    id: rel.to.clone(),
                    stereotype: None,
                    attributes: Vec::new(),
                    methods: Vec::new(),
                });
                relations.push(rel);
                continue;
            }
        }

        let direction = dir_override.unwrap_or(current_dir);
        Self::layout_and_render_svg(&classes, &relations, direction, active_palette)
    }

    fn parse_relation(line: &str) -> Option<ClassRelation> {
        let (rel_part, label) = if let Some(colon_idx) = line.find(':') {
            (line[..colon_idx].trim(), Some(line[colon_idx + 1..].trim().to_string()))
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
                let to = rel_part[idx + token.len()..].trim().trim_matches('"').trim().to_string();

                if !from.is_empty() && !to.is_empty() {
                    let (f, t) = if left_to_right { (from, to) } else { (to, from) };
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

        let mut node_positions: HashMap<String, (f32, f32, f32, f32)> = HashMap::new();
        let mut nodes: Vec<DiagramNode> = Vec::new();

        let margin_x = 60.0f32;
        let margin_y = 60.0f32;
        let card_w = 260.0f32;
        let header_h = 44.0f32;
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
                let attr_count = class.attributes.len();
                let meth_count = class.methods.len();
                let card_h = header_h + (attr_count.max(1) * 22) as f32 + (meth_count.max(1) * 22) as f32 + 20.0;

                let (x, y) = if is_horizontal {
                    (current_offset, cross_offset)
                } else {
                    (cross_offset, current_offset)
                };

                node_positions.insert(class.id.clone(), (x, y, card_w, card_h));
                nodes.push(DiagramNode {
                    id: class.id.clone(),
                    label: class.id.clone(),
                    x,
                    y,
                    width: card_w,
                    height: card_h,
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

        let (total_w, total_h) = if is_horizontal {
            ((current_offset + margin_x).max(900.0), (max_cross + margin_y).max(600.0))
        } else {
            ((max_cross + margin_x).max(900.0), (current_offset + margin_y).max(600.0))
        };

        let mut svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"##,
            total_w, total_h, total_w, total_h
        );

        // Marker Definitions with Theme Colors
        svg.push_str(&format!(
            r##"
        <defs>
            <marker id="inheritance" viewBox="0 0 16 12" refX="15" refY="6" markerWidth="16" markerHeight="12" orient="auto">
                <polygon points="0 0, 15 6, 0 12" fill="{}" stroke="{}" stroke-width="2"/>
            </marker>
            <marker id="composition" viewBox="0 0 16 12" refX="16" refY="6" markerWidth="16" markerHeight="12" orient="auto">
                <polygon points="0 6, 8 0, 16 6, 8 12" fill="{}" stroke="{}"/>
            </marker>
            <marker id="aggregation" viewBox="0 0 16 12" refX="16" refY="6" markerWidth="16" markerHeight="12" orient="auto">
                <polygon points="0 6, 8 0, 16 6, 8 12" fill="{}" stroke="{}" stroke-width="2"/>
            </marker>
            <marker id="association" viewBox="0 0 12 10" refX="11" refY="5" markerWidth="12" markerHeight="10" orient="auto">
                <polyline points="0 1, 10 5, 0 9" fill="none" stroke="{}" stroke-width="2"/>
            </marker>
            <marker id="dependency" viewBox="0 0 12 10" refX="11" refY="5" markerWidth="12" markerHeight="10" orient="auto">
                <polyline points="0 1, 10 5, 0 9" fill="none" stroke="{}" stroke-width="2"/>
            </marker>
        </defs>
        "##,
            palette.card_bg, palette.border,
            palette.border, palette.border,
            palette.card_bg, palette.border,
            palette.border,
            palette.text_muted
        ));

        // Background
        svg.push_str(&format!(r##"<rect width="{}" height="{}" fill="{}"/>"##, total_w, total_h, palette.background));

        // Draw Relationships
        for rel in relations {
            if let (Some(&(fx, fy, fw, fh)), Some(&(tx, ty, tw, th))) = (
                node_positions.get(&rel.from),
                node_positions.get(&rel.to),
            ) {
                let (start_x, start_y, end_x, end_y) = if is_horizontal {
                    (fx + fw, fy + fh / 2.0, tx, ty + th / 2.0)
                } else {
                    (fx + fw / 2.0, fy + fh, tx + tw / 2.0, ty)
                };

                let marker = match rel.kind {
                    RelationKind::Inheritance => r#"marker-end="url(#inheritance)""#,
                    RelationKind::Realization => r#"marker-end="url(#inheritance)" stroke-dasharray="6,4""#,
                    RelationKind::Composition => r#"marker-end="url(#composition)""#,
                    RelationKind::Aggregation => r#"marker-end="url(#aggregation)""#,
                    RelationKind::Association => r#"marker-end="url(#association)""#,
                    RelationKind::Dependency => r#"marker-end="url(#dependency)" stroke-dasharray="4,4""#,
                    RelationKind::Link => "",
                };

                let mid_x = (start_x + end_x) / 2.0;
                let mid_y = (start_y + end_y) / 2.0;

                let path_d = if is_horizontal {
                    format!("M {} {} C {} {}, {} {}, {} {}", start_x, start_y, mid_x, start_y, mid_x, end_y, end_x, end_y)
                } else {
                    format!("M {} {} C {} {}, {} {}, {} {}", start_x, start_y, start_x, mid_y, end_x, mid_y, end_x, end_y)
                };

                svg.push_str(&format!(
                    r##"<path d="{}" fill="none" stroke="{}" stroke-width="2" {}/>"##,
                    path_d, palette.edge_stroke, marker
                ));

                if let Some(ref lbl) = rel.label {
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="20" rx="4" fill="{}" opacity="0.95"/>
                        <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="11" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        mid_x - (lbl.len() * 4) as f32 - 6.0, mid_y - 10.0, (lbl.len() * 8 + 12) as f32,
                        palette.badge_bg,
                        mid_x, mid_y, palette.text_main, lbl
                    ));
                }
            }
        }

        // Draw Class Cards
        for class in class_list {
            if let Some(&(x, y, w, h)) = node_positions.get(&class.id) {
                svg.push_str(&format!(
                    r##"<g id="node_{}" class="class-node">
                    <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="2"/>
                    <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}"/>
                    <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>"##,
                    class.id,
                    x, y, w, h, palette.card_bg, palette.border,
                    x, y, w, header_h, palette.card_header,
                    x, y + header_h, x + w, y + header_h, palette.border
                ));

                // Stereotype
                if let Some(ref stereo) = class.stereotype {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="11" text-anchor="middle" dominant-baseline="middle">&laquo;{}&raquo;</text>"##,
                        x + w / 2.0, y + 14.0, palette.text_accent, stereo
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="14" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        x + w / 2.0, y + 30.0, palette.text_main, class.id
                    ));
                } else {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="15" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        x + w / 2.0, y + 22.0, palette.text_main, class.id
                    ));
                }

                // Attributes
                let mut cur_y = y + header_h + 16.0;
                if class.attributes.is_empty() {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12" font-style="italic">  (no attributes)</text>"##,
                        x + 16.0, cur_y, palette.text_muted
                    ));
                    cur_y += 20.0;
                } else {
                    for attr in &class.attributes {
                        let vis_color = match attr.visibility {
                            '+' => &palette.public_vis,
                            '-' => &palette.private_vis,
                            '#' => &palette.protected_vis,
                            _ => &palette.package_vis,
                        };
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12" font-weight="bold">{}</text>
                            <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12">{}</text>"##,
                            x + 14.0, cur_y, vis_color, attr.visibility,
                            x + 28.0, cur_y, palette.text_sub, attr.raw
                        ));
                        cur_y += 20.0;
                    }
                }

                // Divider line between attributes and methods
                svg.push_str(&format!(
                    r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                    x, cur_y, x + w, cur_y, palette.divider
                ));
                cur_y += 16.0;

                // Methods
                if class.methods.is_empty() {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12" font-style="italic">  (no methods)</text>"##,
                        x + 16.0, cur_y, palette.text_muted
                    ));
                } else {
                    for meth in &class.methods {
                        let vis_color = match meth.visibility {
                            '+' => &palette.public_vis,
                            '-' => &palette.private_vis,
                            '#' => &palette.protected_vis,
                            _ => &palette.package_vis,
                        };
                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12" font-weight="bold">{}</text>
                            <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="12">{}</text>"##,
                            x + 14.0, cur_y, vis_color, meth.visibility,
                            x + 28.0, cur_y, palette.text_main, meth.raw
                        ));
                        cur_y += 20.0;
                    }
                }

                svg.push_str("</g>");
            }
        }

        svg.push_str("</svg>");

        Ok(RenderedDiagram {
            svg,
            width: total_w,
            height: total_h,
            nodes,
        })
    }
}
