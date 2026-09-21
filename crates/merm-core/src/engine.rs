use std::sync::mpsc::{channel, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::ast_rewriter::LayoutDirection;
use crate::class_diagram::ClassDiagramParser;
use crate::error::CoreError;
use crate::theme::ColorPalette;
use crate::xml_utils::{escape_xml, split_and_escape_lines};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationKind {
    Standard,
    Inheritance,
    Realization,
    Composition,
    Aggregation,
    Association,
    Dependency,
    Link,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ClassMemberInfo {
    pub visibility: char,
    pub name: String,
    pub type_name: Option<String>,
    pub comment: Option<String>,
    pub is_method: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeContractInfo {
    pub executable: bool,
    pub input_expected: Option<String>,
    pub input_example: Option<String>,
    pub input_default: Option<String>,
    pub output_expected: Option<String>,
    pub output_default: Option<String>,
    pub last_status: Option<String>,
    pub health: String,
    pub last_duration_ms: f32,
    pub exit_code: i32,
    pub source_location: Option<String>,
}

impl Default for NodeContractInfo {
    fn default() -> Self {
        Self {
            executable: true,
            input_expected: Some(
                "{\n  \"user\": \"string\",\n  \"pass\": \"string\"\n}".to_string(),
            ),
            input_example: Some("{\n  \"user\": \"admin\",\n  \"pass\": \"secret\"\n}".to_string()),
            input_default: Some("{}".to_string()),
            output_expected: Some(
                "{\n  \"token\": \"string\",\n  \"expires_at\": \"u64\"\n}".to_string(),
            ),
            output_default: Some("1".to_string()),
            last_status: Some("Idle".to_string()),
            health: "Healthy".to_string(),
            last_duration_ms: 1.2,
            exit_code: 0,
            source_location: Some("src/services/auth.rs:42".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    pub stereotype: Option<String>,
    pub doc_comment: Option<String>,
    pub lines: Vec<String>,
    pub attributes: Vec<ClassMemberInfo>,
    pub methods: Vec<ClassMemberInfo>,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub contract: Option<NodeContractInfo>,
}

impl DiagramNode {
    pub fn clean_title(&self) -> String {
        let mut title = self.label.clone();
        if title.is_empty() {
            title = self.id.clone();
        }
        if let Some(pos) = title.find("<<") {
            title = title[..pos].trim().to_string();
        }
        if let Some(pos) = title.find('(') {
            if let Some(end) = title.rfind(')') {
                if end > pos {
                    title = title[pos + 1..end].trim().to_string();
                }
            }
        }
        if title.is_empty() {
            self.id.clone()
        } else {
            title
        }
    }

    pub fn role(&self) -> String {
        if let Some(ref st) = self.stereotype {
            let s = st.trim_matches(['<', '>', ' ', '"', '\'']).to_lowercase();
            if !s.is_empty() {
                return s;
            }
        }
        let full = format!("{} {}", self.id, self.label).to_lowercase();
        for keyword in &[
            "actor", "user", "app", "frontend", "mobile", "service", "api", "gateway", "database",
            "postgres", "redis", "cache", "queue", "kafka",
        ] {
            if full.contains(keyword) {
                return match *keyword {
                    "user" => "actor".to_string(),
                    "gateway" | "api" => "service".to_string(),
                    "postgres" => "database".to_string(),
                    "redis" => "cache".to_string(),
                    "kafka" => "queue".to_string(),
                    other => other.to_string(),
                };
            }
        }
        "service".to_string()
    }
}

#[derive(Debug, Clone)]
pub struct FlowEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
    pub dotted: bool,
    pub thick: bool,
    pub kind: RelationKind,
}

#[derive(Debug, Clone)]
pub struct RenderedDiagram {
    pub svg: String,
    pub width: f32,
    pub height: f32,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<FlowEdge>,
    pub is_class_diagram: bool,
    pub direction: LayoutDirection,
    pub selected_node_id: Option<String>,
}

impl RenderedDiagram {
    pub fn regenerate_svg(&mut self, palette: &ColorPalette) {
        let is_horizontal =
            self.direction == LayoutDirection::LR || self.direction == LayoutDirection::RL;

        let mut max_x = self.width;
        let mut max_y = self.height;

        for node in &self.nodes {
            max_x = max_x.max(node.x + node.width + 80.0);
            max_y = max_y.max(node.y + node.height + 80.0);
        }
        let total_width = max_x;
        let total_height = max_y;

        let mut svg = String::with_capacity(32768);
        use std::fmt::Write as _;
        let _ = write!(
            svg,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}">"##,
            total_width, total_height, total_width, total_height
        );

        // Marker definitions with theme semantic colors
        svg.push_str(&format!(
            r##"
        <defs>
            <pattern id="canvas-grid" width="24" height="24" patternUnits="userSpaceOnUse">
                <circle cx="12" cy="12" r="0.8" fill="#283141" opacity="0.5"/>
            </pattern>
            <marker id="flow-arrow" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="8" markerHeight="8" orient="auto">
                <path d="M 0 1 L 10 5 L 0 9 z" fill="{}"/>
            </marker>
            <marker id="inheritance" viewBox="0 0 16 12" refX="15" refY="6" markerWidth="16" markerHeight="12" orient="auto">
                <polygon points="0 0, 15 6, 0 12" fill="{}" stroke="{}" stroke-width="2"/>
            </marker>
            <marker id="realization" viewBox="0 0 16 12" refX="15" refY="6" markerWidth="16" markerHeight="12" orient="auto">
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
        {}
        "##,
            palette.edge_stroke,
            palette.card_bg, palette.edge_stroke,
            palette.card_bg, palette.public_vis,
            palette.private_vis, palette.private_vis,
            palette.card_bg, palette.protected_vis,
            palette.border,
            palette.package_vis,
            if palette.background.eq_ignore_ascii_case("transparent")
                || palette.background.eq_ignore_ascii_case("none")
            {
                String::new()
            } else {
                format!(
                    r##"<rect width="100%" height="100%" fill="{}"/>
        <rect width="100%" height="100%" fill="url(#canvas-grid)"/>"##,
                    palette.background
                )
            }
        ));

        // Draw edges
        for edge in &self.edges {
            let from_node = self.nodes.iter().find(|n| n.id == edge.from);
            let to_node = self.nodes.iter().find(|n| n.id == edge.to);

            if let (Some(f), Some(t)) = (from_node, to_node) {
                let is_same_row = !is_horizontal && (f.y - t.y).abs() < 20.0;
                let (sx, sy, ex, ey) = if is_horizontal {
                    (
                        f.x + f.width,
                        f.y + f.height / 2.0,
                        t.x,
                        t.y + t.height / 2.0,
                    )
                } else if is_same_row {
                    if f.x < t.x {
                        (
                            f.x + f.width,
                            f.y + f.height / 2.0,
                            t.x,
                            t.y + t.height / 2.0,
                        )
                    } else {
                        (
                            f.x,
                            f.y + f.height / 2.0,
                            t.x + t.width,
                            t.y + t.height / 2.0,
                        )
                    }
                } else {
                    (
                        f.x + f.width / 2.0,
                        f.y + f.height,
                        t.x + t.width / 2.0,
                        t.y,
                    )
                };

                let (stroke_color, marker_attr, dash_style, stroke_w) = match edge.kind {
                    RelationKind::Inheritance => (
                        &palette.edge_stroke,
                        r#"marker-end="url(#inheritance)""#,
                        "",
                        "2",
                    ),
                    RelationKind::Realization => (
                        &palette.public_vis,
                        r#"marker-end="url(#realization)""#,
                        r#"stroke-dasharray="6,4" "#,
                        "2",
                    ),
                    RelationKind::Composition => (
                        &palette.private_vis,
                        r#"marker-end="url(#composition)""#,
                        "",
                        "2",
                    ),
                    RelationKind::Aggregation => (
                        &palette.protected_vis,
                        r#"marker-end="url(#aggregation)""#,
                        "",
                        "2",
                    ),
                    RelationKind::Association => (
                        &palette.border,
                        r#"marker-end="url(#association)""#,
                        "",
                        "2",
                    ),
                    RelationKind::Dependency => (
                        &palette.package_vis,
                        r#"marker-end="url(#dependency)""#,
                        r#"stroke-dasharray="4,4" "#,
                        "2",
                    ),
                    RelationKind::Link => (&palette.divider, "", "", "1.5"),
                    RelationKind::Standard => {
                        let dash = if edge.dotted {
                            r#"stroke-dasharray="6,4" "#
                        } else {
                            ""
                        };
                        let w = if edge.thick { "3.5" } else { "2" };
                        (
                            &palette.edge_stroke,
                            r#"marker-end="url(#flow-arrow)""#,
                            dash,
                            w,
                        )
                    }
                };

                let (c1x, c1y, c2x, c2y) = if is_horizontal || is_same_row {
                    let mid_x = (sx + ex) / 2.0;
                    (mid_x, sy, mid_x, ey)
                } else if ey > sy {
                    let mid_y = (sy + ey) / 2.0;
                    (sx, mid_y, ex, mid_y)
                } else {
                    let sweep_x = sx.max(ex) + 80.0;
                    (sweep_x, sy, sweep_x, ey)
                };

                let mid_x = (sx + ex) / 2.0;
                let mid_y = (sy + ey) / 2.0;

                let (actual_stroke, actual_w, opacity_attr) =
                    if let Some(ref sel) = self.selected_node_id {
                        if edge.to == *sel {
                            ("#3fb950", "2.5", "opacity=\"1.0\" ")
                        } else if edge.from == *sel {
                            ("#f0883e", "2.5", "opacity=\"1.0\" ")
                        } else {
                            (stroke_color.as_str(), "1.5", "opacity=\"0.35\" ")
                        }
                    } else {
                        (stroke_color.as_str(), stroke_w, "")
                    };

                let dash_part = if dash_style.trim().is_empty() {
                    String::new()
                } else {
                    format!("{} ", dash_style.trim())
                };
                let marker_part = if marker_attr.trim().is_empty() {
                    String::new()
                } else {
                    format!("{} ", marker_attr.trim())
                };
                let opacity_part = if opacity_attr.trim().is_empty() {
                    String::new()
                } else {
                    format!("{} ", opacity_attr.trim())
                };

                svg.push_str(&format!(
                    r##"<path d="M {} {} C {} {}, {} {}, {} {}" fill="none" stroke="{}" stroke-width="{}" {}{}{}/>"##,
                    sx, sy, c1x, c1y, c2x, c2y, ex, ey, actual_stroke, actual_w, dash_part, marker_part, opacity_part
                ));

                if let Some(ref lbl) = edge.label {
                    let escaped_lbl = escape_xml(lbl);
                    let badge_w = (escaped_lbl.len() as f32 * 7.5 + 16.0).max(40.0);
                    let badge_h = 20.0f32;
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" stroke="{}" stroke-width="1" opacity="0.95"/>
                        <text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="11" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
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
        for node in &self.nodes {
            let is_selected = self.selected_node_id.as_deref() == Some(&node.id);
            let role = node.role();
            let clean_title = node.clean_title();
            let role_col = palette.node_role_color(&role, &clean_title);
            let role_bg = palette.node_role_bg(&role, &clean_title);
            let border_color = if is_selected {
                palette.text_accent.as_str()
            } else {
                role_col
            };
            let border_width = if is_selected { "3" } else { "2" };

            // Outer selection halo
            if is_selected {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="12" fill="none" stroke="{}" stroke-width="2.5" stroke-dasharray="5,4" opacity="0.9"/>"##,
                    node.x - 5.0, node.y - 5.0, node.width + 10.0, node.height + 10.0, palette.text_accent
                ));
            }

            let is_class =
                self.is_class_diagram || !node.attributes.is_empty() || !node.methods.is_empty();

            if is_class {
                // Class Diagram Card
                let header_h = if node.doc_comment.is_some() && node.stereotype.is_some() {
                    60.0f32
                } else if node.doc_comment.is_some() || node.stereotype.is_some() {
                    50.0f32
                } else {
                    42.0f32
                };

                svg.push_str(&format!(
                    r##"<g id="node_{}" class="class-node">
                    <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="{}"/>
                    <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" opacity="0.5"/>
                    <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1.5"/>"##,
                    escape_xml(&node.id),
                    node.x, node.y, node.width, node.height, role_bg, border_color, border_width,
                    node.x, node.y, node.width, header_h, palette.card_header,
                    node.x, node.y + header_h, node.x + node.width, node.y + header_h, border_color
                ));

                // Header Contents: Stereotype, Class Name, Doc Comment
                let center_x = node.x + node.width / 2.0;
                if let (Some(ref stereo), Some(ref doc)) = (&node.stereotype, &node.doc_comment) {
                    let esc_stereo = escape_xml(stereo);
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="11" font-weight="bold" text-anchor="middle">«{}»</text>"##,
                        center_x, node.y + 15.0, palette.stereotype_color, esc_stereo
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="14" font-weight="bold" text-anchor="middle">{}</text>"##,
                        center_x, node.y + 32.0, palette.text_main, escape_xml(&node.id)
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="10" font-style="italic" text-anchor="middle">// {}</text>"##,
                        center_x, node.y + 48.0, palette.comment_color, escape_xml(doc)
                    ));
                } else if let Some(ref stereo) = node.stereotype {
                    let esc_stereo = escape_xml(stereo);
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="11" font-weight="bold" text-anchor="middle">«{}»</text>"##,
                        center_x, node.y + 16.0, palette.stereotype_color, esc_stereo
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="14" font-weight="bold" text-anchor="middle">{}</text>"##,
                        center_x, node.y + 35.0, palette.text_main, escape_xml(&node.id)
                    ));
                } else if let Some(ref doc) = node.doc_comment {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="14" font-weight="bold" text-anchor="middle">{}</text>"##,
                        center_x, node.y + 20.0, palette.text_main, escape_xml(&node.id)
                    ));
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="10" font-style="italic" text-anchor="middle">// {}</text>"##,
                        center_x, node.y + 38.0, palette.comment_color, escape_xml(doc)
                    ));
                } else {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="15" font-weight="bold" text-anchor="middle" dominant-baseline="middle">{}</text>"##,
                        center_x, node.y + header_h / 2.0, palette.text_main, escape_xml(&node.id)
                    ));
                }

                // Contract Compartment
                let mut cur_y = node.y + header_h;
                if let Some(ref c) = node.contract {
                    let contract_h = 44.0f32;
                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.30"/>
                        <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                        node.x,
                        cur_y,
                        node.width,
                        contract_h,
                        palette.badge_bg,
                        node.x,
                        cur_y + contract_h,
                        node.x + node.width,
                        cur_y + contract_h,
                        palette.divider
                    ));

                    let in_exp = c.input_expected.as_deref().unwrap_or("JSON");
                    let in_def = c.input_default.as_deref().unwrap_or("{}");
                    let out_exp = c.output_expected.as_deref().unwrap_or("String");
                    let out_def = c.output_default.as_deref().unwrap_or("1");

                    let status_str = if let Some(ref st) = c.last_status {
                        format!(" │ {}", st)
                    } else {
                        String::new()
                    };

                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="11"><tspan fill="{}" font-weight="bold">IN </tspan><tspan fill="{}">Exp: {}</tspan><tspan fill="{}"> │ Def: {}</tspan></text>
                        <text x="{}" y="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="11"><tspan fill="{}" font-weight="bold">OUT</tspan><tspan fill="{}"> Exp: {}</tspan><tspan fill="{}"> │ Def: {}</tspan><tspan fill="{}" font-weight="bold">{}</tspan></text>"##,
                        node.x + 14.0, cur_y + 17.0, palette.method_color, palette.text_main, escape_xml(in_exp), palette.type_color, escape_xml(in_def),
                        node.x + 14.0, cur_y + 34.0, palette.stereotype_color, palette.text_main, escape_xml(out_exp), palette.type_color, escape_xml(out_def), palette.public_vis, escape_xml(&status_str)
                    ));

                    cur_y += contract_h + 18.0;
                } else {
                    cur_y += 18.0;
                }

                // Attributes Compartment
                if node.attributes.is_empty() {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="12" font-style="italic">  (no attributes)</text>"##,
                        node.x + 16.0, cur_y, palette.text_muted
                    ));
                    cur_y += 22.0;
                } else {
                    for attr in &node.attributes {
                        let vis_color = match attr.visibility {
                            '+' => &palette.public_vis,
                            '-' => &palette.private_vis,
                            '#' => &palette.protected_vis,
                            _ => &palette.package_vis,
                        };

                        let type_part = if let Some(ref t) = attr.type_name {
                            format!(
                                r##"<tspan fill="{}" font-weight="bold">{} </tspan>"##,
                                palette.type_color,
                                escape_xml(t)
                            )
                        } else {
                            String::new()
                        };

                        let var_color = if is_selected {
                            &palette.text_accent
                        } else {
                            &palette.var_color
                        };

                        let comm_part = if let Some(ref comm) = attr.comment {
                            format!(
                                r##"<tspan fill="{}" font-style="italic">  // {}</tspan>"##,
                                palette.comment_color,
                                escape_xml(comm)
                            )
                        } else {
                            String::new()
                        };

                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="12"><tspan fill="{}" font-weight="bold">{} </tspan>{}<tspan fill="{}">{}</tspan>{}</text>"##,
                            node.x + 14.0, cur_y, vis_color, attr.visibility, type_part, var_color, escape_xml(&attr.name), comm_part
                        ));

                        cur_y += 22.0;
                    }
                }

                // Divider line between attributes and methods
                svg.push_str(&format!(
                    r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                    node.x,
                    cur_y,
                    node.x + node.width,
                    cur_y,
                    palette.divider
                ));
                cur_y += 18.0;

                // Methods Compartment
                if node.methods.is_empty() {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="12" font-style="italic">  (no methods)</text>"##,
                        node.x + 16.0, cur_y, palette.text_muted
                    ));
                } else {
                    for meth in &node.methods {
                        let vis_color = match meth.visibility {
                            '+' => &palette.public_vis,
                            '-' => &palette.private_vis,
                            '#' => &palette.protected_vis,
                            _ => &palette.package_vis,
                        };

                        let method_color = if is_selected {
                            &palette.text_accent
                        } else {
                            &palette.method_color
                        };

                        let ret_part = if let Some(ref t) = meth.type_name {
                            format!(
                                r##"<tspan fill="{}">: </tspan><tspan fill="{}" font-weight="bold">{}</tspan>"##,
                                palette.text_muted,
                                palette.type_color,
                                escape_xml(t)
                            )
                        } else {
                            String::new()
                        };

                        let comm_part = if let Some(ref comm) = meth.comment {
                            format!(
                                r##"<tspan fill="{}" font-style="italic">  // {}</tspan>"##,
                                palette.comment_color,
                                escape_xml(comm)
                            )
                        } else {
                            String::new()
                        };

                        svg.push_str(&format!(
                            r##"<text x="{}" y="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="12"><tspan fill="{}" font-weight="bold">{} </tspan><tspan fill="{}" font-weight="bold">{}</tspan>{}{}</text>"##,
                            node.x + 14.0, cur_y, vis_color, meth.visibility, method_color, escape_xml(&meth.name), ret_part, comm_part
                        ));

                        cur_y += 22.0;
                    }
                }

                // Floating Action Bar directly beneath selected node
                if is_selected {
                    let pill_w = 110.0f32;
                    let pill_h = 24.0f32;
                    let pill_x = node.x + (node.width - pill_w) / 2.0;
                    let pill_y = node.y + node.height + 8.0;

                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="12" fill="{}" stroke="{}" stroke-width="1"/>
                        <text x="{}" y="{}" fill="{}" font-size="12" font-family="monospace" text-anchor="middle" dominant-baseline="central">+   日   ❐   🗑</text>"##,
                        pill_x, pill_y, pill_w, pill_h, palette.card_bg, palette.divider,
                        pill_x + pill_w / 2.0, pill_y + pill_h / 2.0, palette.text_sub
                    ));
                }

                svg.push_str("</g>\n");
            } else {
                // Modern Architecture Service Card (Apple / Linear Dark aesthetic matching reference image)
                let role = node.role();
                let clean_title = node.clean_title();
                let role_col = palette.node_role_color(&role, &clean_title);
                let role_bg = palette.node_role_bg(&role, &clean_title);
                let card_stroke = if is_selected {
                    palette.text_accent.as_str()
                } else {
                    role_col
                };
                let border_w = if is_selected { "2.5" } else { "2" };

                svg.push_str(&format!(
                    r##"<g id="node_{}" class="arch-node">
                    <rect x="{}" y="{}" width="{}" height="{}" rx="10" fill="{}" stroke="{}" stroke-width="{}"/>"##,
                    escape_xml(&node.id),
                    node.x, node.y, node.width, node.height, role_bg, card_stroke, border_w
                ));

                // Left Vector Icon matching the role
                let icon_cx = node.x + 26.0;
                let icon_cy = node.y + node.height / 2.0;
                let icon_svg =
                    palette.node_role_icon_svg(&role, &clean_title, icon_cx, icon_cy, role_col);
                svg.push_str(&icon_svg);

                // Title and role badge
                let text_x = node.x + 46.0;
                let title_y = node.y + node.height * 0.42;
                let role_y = node.y + node.height * 0.72;
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="#ffffff" font-family="system-ui, -apple-system, sans-serif" font-size="13" font-weight="bold">{}</text>
                    <text x="{}" y="{}" fill="{}" font-family="system-ui, -apple-system, sans-serif" font-size="10.5" font-weight="600">&lt;&lt;{}&gt;&gt;</text>"##,
                    text_x, title_y, escape_xml(&clean_title),
                    text_x, role_y, role_col, escape_xml(&role)
                ));

                // Floating Action Bar directly beneath selected node
                if is_selected {
                    let pill_w = 110.0f32;
                    let pill_h = 24.0f32;
                    let pill_x = node.x + (node.width - pill_w) / 2.0;
                    let pill_y = node.y + node.height + 8.0;

                    svg.push_str(&format!(
                        r##"<rect x="{}" y="{}" width="{}" height="{}" rx="12" fill="{}" stroke="{}" stroke-width="1"/>
                        <text x="{}" y="{}" fill="{}" font-size="12" font-family="monospace" text-anchor="middle" dominant-baseline="central">+   日   ❐   🗑</text>"##,
                        pill_x, pill_y, pill_w, pill_h, palette.card_bg, palette.divider,
                        pill_x + pill_w / 2.0, pill_y + pill_h / 2.0, palette.text_sub
                    ));
                }

                svg.push_str("</g>\n");
            }
        }

        svg.push_str("</svg>");
        self.svg = svg;
    }
}

pub struct LayoutEngine {
    timeout: Duration,
    #[allow(dead_code)]
    memory_limit_bytes: usize,
    pub palette: ColorPalette,
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self {
            timeout: Duration::from_millis(2000),
            memory_limit_bytes: 32 * 1024 * 1024,
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
    token
        .split(":::")
        .next()
        .unwrap_or(token)
        .trim()
        .to_string()
}

fn clean_label(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches('"').trim_matches('\'').trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn sanitize_line_for_node_extraction(line: &str) -> String {
    if !line.contains('|') {
        return line.to_string();
    }

    let mut out = String::with_capacity(line.len());
    let mut in_pipe = false;

    for c in line.chars() {
        if c == '|' {
            in_pipe = !in_pipe;
            out.push(' ');
            continue;
        }
        if in_pipe {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
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
                    kind: RelationKind::Standard,
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
                kind: RelationKind::Standard,
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
                    kind: RelationKind::Standard,
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
                        kind: RelationKind::Standard,
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
                    kind: RelationKind::Standard,
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
                    kind: RelationKind::Standard,
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
                        kind: RelationKind::Standard,
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
                    kind: RelationKind::Standard,
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
                kind: RelationKind::Standard,
            });
        }
    }

    None
}

fn extract_nodes_from_line(line: &str, nodes: &mut Vec<DiagramNode>) {
    let sanitized = sanitize_line_for_node_extraction(line);
    let mut rem = sanitized.as_str();

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

                let _line_count = lines.len();
                let max_len = lines.iter().map(|l| l.chars().count()).max().unwrap_or(8);
                let width = (max_len as f32 * 7.5 + 48.0).clamp(195.0, 240.0);
                let height = 54.0f32;

                let stereo = if let Some(p1) = clean_lbl.find("<<") {
                    clean_lbl[p1..]
                        .find(">>")
                        .map(|p2| clean_lbl[p1 + 2..p1 + p2].trim().to_string())
                } else {
                    None
                };

                if let Some(existing) = nodes.iter_mut().find(|n| n.id == node_id) {
                    existing.label = clean_lbl.to_string();
                    existing.lines = lines;
                    existing.width = width;
                    existing.height = height;
                    if stereo.is_some() {
                        existing.stereotype = stereo;
                    }
                    if existing.contract.is_none() {
                        existing.contract = Some(NodeContractInfo::default());
                    }
                } else {
                    nodes.push(DiagramNode {
                        id: node_id,
                        label: clean_lbl.to_string(),
                        stereotype: stereo,
                        doc_comment: None,
                        lines,
                        attributes: Vec::new(),
                        methods: Vec::new(),
                        x: 0.0,
                        y: 0.0,
                        width,
                        height,
                        contract: Some(NodeContractInfo::default()),
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
                        stereotype: None,
                        doc_comment: None,
                        lines: vec![esc],
                        attributes: Vec::new(),
                        methods: Vec::new(),
                        x: 0.0,
                        y: 0.0,
                        width: 195.0,
                        height: 54.0,
                        contract: None,
                    });
                }
                if !nodes.iter().any(|n| n.id == edge.to) {
                    let esc = escape_xml(&edge.to);
                    nodes.push(DiagramNode {
                        id: edge.to.clone(),
                        label: edge.to.clone(),
                        stereotype: None,
                        doc_comment: None,
                        lines: vec![esc],
                        attributes: Vec::new(),
                        methods: Vec::new(),
                        x: 0.0,
                        y: 0.0,
                        width: 195.0,
                        height: 54.0,
                        contract: None,
                    });
                }
                edges.push(edge);
            }
        }

        if nodes.is_empty() {
            let svg = format!(
                r##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><rect width="100%" height="100%" fill="{}"/><text x="400" y="300" fill="{}" font-family="monospace, 'Noto Color Emoji', sans-serif" font-size="16" text-anchor="middle">No elements to display</text></svg>"##,
                palette.background, palette.text_main
            );
            return Ok(RenderedDiagram {
                svg,
                width: 800.0,
                height: 600.0,
                nodes,
                edges: Vec::new(),
                is_class_diagram: false,
                direction: dir,
                selected_node_id: None,
            });
        }

        let is_horizontal = dir == LayoutDirection::LR || dir == LayoutDirection::RL;

        // 1. Compute in-degrees of nodes to identify root nodes
        let mut in_degrees: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for edge in &edges {
            *in_degrees.entry(edge.to.clone()).or_insert(0) += 1;
        }

        let mut roots: Vec<String> = nodes
            .iter()
            .filter(|n| in_degrees.get(&n.id).copied().unwrap_or(0) == 0)
            .map(|n| n.id.clone())
            .collect();
        if roots.is_empty() && !nodes.is_empty() {
            roots.push(nodes[0].id.clone());
        }

        // 2. BFS shortest distance from roots to determine primary rank
        let mut ranks: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut queue = std::collections::VecDeque::new();
        for r in &roots {
            ranks.insert(r.clone(), 0);
            queue.push_back((r.clone(), 0));
        }

        while let Some((curr, r)) = queue.pop_front() {
            for edge in &edges {
                if edge.from == curr {
                    let next_r = r + 1;
                    let should_update = match ranks.get(&edge.to) {
                        None => true,
                        Some(&old_r) => next_r < old_r,
                    };
                    if should_update {
                        ranks.insert(edge.to.clone(), next_r);
                        queue.push_back((edge.to.clone(), next_r));
                    }
                }
            }
        }

        // Fill any unvisited nodes
        let max_r = ranks.values().copied().max().unwrap_or(0);
        for node in &nodes {
            ranks.entry(node.id.clone()).or_insert(max_r + 1);
        }

        let max_rank = ranks.values().copied().max().unwrap_or(0);
        let mut layers: Vec<Vec<usize>> = vec![Vec::new(); max_rank + 1];
        for (idx, node) in nodes.iter().enumerate() {
            let r = ranks.get(&node.id).copied().unwrap_or(0);
            layers[r].push(idx);
        }

        // 3. Intra-layer ordering: if an edge connects nodes in the same layer (e.g. WebFrontend -> ApiGateway),
        // order source before target to ensure clean left-to-right flow.
        for layer in &mut layers {
            if layer.len() > 1 {
                layer.sort_by(|&a_idx, &b_idx| {
                    let a_id = &nodes[a_idx].id;
                    let b_id = &nodes[b_idx].id;
                    if edges.iter().any(|e| e.from == *a_id && e.to == *b_id) {
                        std::cmp::Ordering::Less
                    } else if edges.iter().any(|e| e.from == *b_id && e.to == *a_id) {
                        std::cmp::Ordering::Greater
                    } else {
                        a_idx.cmp(&b_idx)
                    }
                });
            }
        }

        let margin_x = 80.0f32;
        let margin_y = 60.0f32;
        let h_gap = 36.0f32;
        let v_gap = 56.0f32;

        let total_width;
        let total_height;

        if !is_horizontal {
            // TD Layout: Compute 3-column / widest-row alignment
            let max_layer_count = layers.iter().map(|l| l.len()).max().unwrap_or(1);
            let standard_card_w = 195.0f32;
            let max_content_w = max_layer_count as f32 * standard_card_w
                + (max_layer_count.saturating_sub(1) as f32 * h_gap);
            total_width = (max_content_w + 2.0 * margin_x).max(960.0);

            let mut cur_y = margin_y + 30.0;
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
            total_height = (cur_y - v_gap + margin_y + 40.0).max(680.0);
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
                let col_w: f32 = layer.iter().map(|&i| nodes[i].width).fold(0.0f32, f32::max);
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

        let mut diagram = RenderedDiagram {
            svg: String::new(),
            width: total_width,
            height: total_height,
            nodes,
            edges,
            is_class_diagram: false,
            direction: dir,
            selected_node_id: None,
        };
        diagram.regenerate_svg(palette);
        Ok(diagram)
    }
}
