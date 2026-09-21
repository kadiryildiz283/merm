use crate::app_state::{AppState, CanvasTool, LayoutAlgorithm, RightPanelTab, SidebarTab};
use crate::modal::{UiAction, UiMode};
use merm_core::escape_xml;

pub struct StudioOverlay;

#[allow(clippy::too_many_arguments)]
impl StudioOverlay {
    pub fn build(app_state: &AppState, width: u32, height: u32) -> String {
        let palette = app_state.theme.palette();
        let w = width as f32;
        let h = height as f32;

        let ui_scale = if w >= 2500.0 || h >= 1500.0 {
            1.85f32
        } else if w >= 1800.0 || h >= 1000.0 {
            1.35f32
        } else {
            1.0f32
        };

        let top_h = 36.0 * ui_scale;
        let bottom_status_h = 24.0 * ui_scale;
        let is_ast_view = app_state.active_sidebar_tab == SidebarTab::AstView;
        let sidebar_w = if app_state.show_left_sidebar && !is_ast_view {
            210.0 * ui_scale
        } else {
            0.0
        };
        let inspector_w = if app_state.show_right_panel && !is_ast_view {
            300.0 * ui_scale
        } else {
            0.0
        };

        let mut svg = String::with_capacity(16384);
        svg.push_str(&format!(
            r#"<svg width="{}" height="{}" viewBox="0 0 {} {}" xmlns="http://www.w3.org/2000/svg">"#,
            width, height, width, height
        ));

        // 1. Center Area: AST View & Split Code Editor (when AST View tab is active)
        if is_ast_view {
            Self::render_ast_split_view(
                app_state,
                0.0,
                top_h,
                w,
                h - top_h - bottom_status_h,
                ui_scale,
                &palette,
                &mut svg,
            );
        } else {
            // Center Canvas Floating Controls (Toolbar & Minimap)
            Self::render_canvas_floating_tools(
                app_state,
                sidebar_w,
                top_h,
                w - sidebar_w - inspector_w,
                h - top_h - bottom_status_h,
                ui_scale,
                &palette,
                &mut svg,
            );
        }

        // 2. Left Navigation Sidebar
        if app_state.show_left_sidebar && !is_ast_view {
            Self::render_left_sidebar(
                app_state,
                top_h,
                sidebar_w,
                h - top_h - bottom_status_h,
                ui_scale,
                &palette,
                &mut svg,
            );
        }

        // 3. Right Inspector / System Overview Drawer
        if app_state.show_right_panel && !is_ast_view {
            Self::render_right_inspector(
                app_state,
                w - inspector_w,
                top_h,
                inspector_w,
                h - top_h - bottom_status_h,
                ui_scale,
                &palette,
                &mut svg,
            );
        }

        // 4. Top Window Bar (Always on top)
        Self::render_top_bar(app_state, w, top_h, ui_scale, &palette, &mut svg);

        // 5. Persistent Vim Split Buffer (when active)
        if app_state.show_split_buffer {
            Self::render_split_buffer(
                app_state,
                w,
                h,
                bottom_status_h,
                ui_scale,
                &palette,
                &mut svg,
            );
        }

        // 6. Bottom Window Statusline
        Self::render_status_bar(
            app_state,
            w,
            h,
            bottom_status_h,
            ui_scale,
            &palette,
            &mut svg,
        );

        // 7. Centered Command Palette Modal (if open)
        if app_state.command_palette_visible || app_state.modal.mode == UiMode::Command {
            Self::render_command_palette(app_state, w, h, ui_scale, &palette, &mut svg);
        }

        // 8. Performance Telemetry HUD (:perf, :fps)
        if app_state.telemetry.enabled {
            Self::render_telemetry(app_state, w, top_h, ui_scale, &palette, &mut svg);
        }

        // 9. NodeEdit Modal (if active)
        if app_state.modal.mode == UiMode::NodeEdit {
            Self::render_node_edit(app_state, w, h, ui_scale, &palette, &mut svg);
        }

        svg.push_str("</svg>");
        svg
    }

    fn render_top_bar(
        app_state: &AppState,
        w: f32,
        top_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        // Background bar
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{}" height="{}" fill="{}" opacity="0.98"/>
            <line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            w, top_h, palette.card_bg, top_h, w, top_h, palette.border
        ));

        // macOS 3 dots (🔴 🟡 🟢)
        let dot_y = top_h / 2.0;
        let dot_r = 5.0 * ui_scale;
        svg.push_str(&format!(
            r##"<circle cx="{}" cy="{}" r="{}" fill="#ff5f56"/>
            <circle cx="{}" cy="{}" r="{}" fill="#ffbd2e"/>
            <circle cx="{}" cy="{}" r="{}" fill="#27c93f"/>"##,
            16.0 * ui_scale,
            dot_y,
            dot_r,
            32.0 * ui_scale,
            dot_y,
            dot_r,
            48.0 * ui_scale,
            dot_y,
            dot_r
        ));

        // Brand Logo: [O] merm with warm gold rounded icon badge
        let logo_x = 68.0 * ui_scale;
        let icon_box_s = 18.0 * ui_scale;
        let icon_box_y = (top_h - icon_box_s) / 2.0;
        let logo_font = (13.0 * ui_scale).round() as u32;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="#f0883e"/>
            <text x="{}" y="{}" fill="#ffffff" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="middle" dominant-baseline="central">⬡</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, -apple-system, sans-serif" font-size="{}" font-weight="bold" dominant-baseline="central">merm</text>"##,
            logo_x, icon_box_y, icon_box_s, icon_box_s,
            logo_x + icon_box_s / 2.0, dot_y, (11.0 * ui_scale).round() as u32,
            logo_x + icon_box_s + 8.0 * ui_scale, dot_y, palette.text_main, logo_font
        ));

        // Breadcrumb: backend / architecture (or auth_service)
        let breadcrumb_x = logo_x + icon_box_s + 64.0 * ui_scale;
        let sub_name = if app_state.active_sidebar_tab == SidebarTab::AstView {
            "auth_service"
        } else {
            "architecture"
        };
        let breadcrumb_text = format!("{} / {}", app_state.active_workspace, sub_name);
        let breadcrumb_font = (11.5 * ui_scale).round() as u32;
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="{}" dominant-baseline="central">{}</text>"##,
            breadcrumb_x, dot_y, palette.text_sub, breadcrumb_font, escape_xml(&breadcrumb_text)
        ));

        // Right Action Pills
        let pill_h = 24.0 * ui_scale;
        let pill_y = (top_h - pill_h) / 2.0;
        let pill_font = (10.5 * ui_scale).round() as u32;

        if app_state.active_sidebar_tab == SidebarTab::AstView {
            let split_w = 32.0 * ui_scale;
            let split_x = w - split_w - 14.0 * ui_scale;
            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"/>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="middle" dominant-baseline="central">⊞</text>"##,
                split_x, pill_y, split_w, pill_h, palette.card_header, palette.border,
                split_x + split_w / 2.0, dot_y, palette.text_main, (12.0 * ui_scale).round() as u32
            ));
        } else {
            let zoom_pct = (app_state.transform.scale * 100.0).round() as u32;
            let bar_w = 148.0 * ui_scale;
            let bar_x = w - bar_w - 14.0 * ui_scale;

            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" dominant-baseline="central">🔍 {}%</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="middle" dominant-baseline="central">⛶</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="middle" dominant-baseline="central">⚙</text>"##,
                bar_x, pill_y, bar_w, pill_h, palette.card_header, palette.border,
                bar_x + 10.0 * ui_scale, dot_y, palette.text_main, pill_font, zoom_pct,
                bar_x + bar_w - 42.0 * ui_scale, dot_y, palette.text_sub, pill_font + 1,
                bar_x + bar_w - 18.0 * ui_scale, dot_y, palette.text_sub, pill_font + 1
            ));
        }
    }

    fn render_left_sidebar(
        app_state: &AppState,
        top_y: f32,
        sidebar_w: f32,
        sidebar_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        // Sidebar background & right divider
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            top_y,
            sidebar_w,
            sidebar_h,
            palette.background,
            sidebar_w,
            top_y,
            sidebar_w,
            top_y + sidebar_h,
            palette.border
        ));

        let item_h = 30.0 * ui_scale;
        let mut cur_y = top_y + 12.0 * ui_scale;
        let font_size = (12.0 * ui_scale).round() as u32;

        let nav_items = [
            (SidebarTab::Explorer, "⬚", "Explorer"),
            (SidebarTab::Diagrams, "☷", "Diagrams"),
            (SidebarTab::AstView, "⎇", "AST View"),
            (SidebarTab::Executions, "▷", "Executions"),
            (SidebarTab::Settings, "⚙", "Settings"),
        ];

        for (tab, icon, label) in nav_items {
            let is_active = app_state.active_sidebar_tab == tab;
            let item_x = 10.0 * ui_scale;
            let item_w = sidebar_w - 20.0 * ui_scale;

            if is_active {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" fill-opacity="0.85"/>"##,
                    item_x, cur_y, item_w, item_h, palette.card_header
                ));
            }

            let text_col = if is_active {
                &palette.text_main
            } else {
                &palette.text_sub
            };
            let font_weight = if is_active { "bold" } else { "normal" };

            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-size="13" dominant-baseline="central">{}</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, -apple-system, sans-serif" font-size="{}" font-weight="{}" dominant-baseline="central">{}</text>"##,
                item_x + 12.0 * ui_scale, cur_y + item_h / 2.0, text_col, icon,
                item_x + 34.0 * ui_scale, cur_y + item_h / 2.0, text_col, font_size, font_weight, label
            ));

            cur_y += item_h + 3.0 * ui_scale;
        }

        // Workspaces Section Divider & Header
        cur_y += 18.0 * ui_scale;
        let header_font = (11.0 * ui_scale).round() as u32;
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="500">Workspaces</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">+</text>"##,
            16.0 * ui_scale, cur_y, palette.text_sub, header_font,
            sidebar_w - 16.0 * ui_scale, cur_y, palette.text_muted, header_font + 1
        ));
        cur_y += 16.0 * ui_scale;

        // Workspaces list
        for ws in &app_state.workspaces {
            let is_active = ws == &app_state.active_workspace;
            let item_x = 10.0 * ui_scale;
            let item_w = sidebar_w - 20.0 * ui_scale;
            let ws_h = 26.0 * ui_scale;

            if is_active {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" fill-opacity="0.6"/>"##,
                    item_x, cur_y, item_w, ws_h, palette.card_header
                ));
            }

            let text_col = if is_active {
                &palette.text_main
            } else {
                &palette.text_sub
            };
            let font_weight = if is_active { "bold" } else { "normal" };

            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-size="11" dominant-baseline="central">⎚</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="{}" dominant-baseline="central">{}</text>"##,
                20.0 * ui_scale, cur_y + ws_h / 2.0, text_col,
                34.0 * ui_scale, cur_y + ws_h / 2.0, text_col, font_size, font_weight, ws
            ));

            cur_y += ws_h + 4.0 * ui_scale;
        }

        // Bottom Footer Status inside Sidebar:
        // Rust
        // v0.1.0
        // 🟢 Ready
        let footer_y = top_y + sidebar_h - 56.0 * ui_scale;
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Rust</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">v0.1.0</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="500">🟢 Ready</text>"##,
            16.0 * ui_scale, footer_y, palette.text_sub, (11.0 * ui_scale).round() as u32,
            16.0 * ui_scale, footer_y + 16.0 * ui_scale, palette.text_muted, (10.5 * ui_scale).round() as u32,
            16.0 * ui_scale, footer_y + 34.0 * ui_scale, palette.public_vis, (11.0 * ui_scale).round() as u32
        ));
    }

    fn render_right_inspector(
        app_state: &AppState,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        // Drawer Background & Left Border
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            x,
            y,
            w,
            h,
            palette.background,
            x,
            y,
            x,
            y + h,
            palette.border
        ));

        // Check if a node is currently selected
        if let Some(ref sel_id) = app_state.active_node_id {
            let active_node = app_state.current_diagram.as_ref().and_then(|d| {
                d.nodes
                    .iter()
                    .find(|n| &n.id == sel_id || &n.clean_title() == sel_id)
            });

            let clean_title = active_node
                .map(|n| n.clean_title())
                .unwrap_or_else(|| sel_id.clone());
            let role = active_node
                .map(|n| n.role())
                .unwrap_or_else(|| "service".to_string());
            let role_col = palette.role_color(&role);
            let role_icon = palette.role_icon_symbol(&role);

            // 1. Header: Icon, Title, Role badge, Health status
            let header_h = 44.0 * ui_scale;
            let icon_box_size = 28.0 * ui_scale;
            let icon_x = x + 14.0 * ui_scale;
            let icon_y = y + 10.0 * ui_scale;

            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" fill-opacity="0.18" stroke="{}" stroke-width="1"/>
                <text x="{}" y="{}" fill="{}" font-size="{}" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                icon_x, icon_y, icon_box_size, icon_box_size, role_col, role_col,
                icon_x + icon_box_size / 2.0, icon_y + icon_box_size / 2.0, role_col, (15.0 * ui_scale).round() as u32, role_icon
            ));

            let title_x = icon_x + icon_box_size + 10.0 * ui_scale;
            let title_font = (13.5 * ui_scale).round() as u32;
            let role_font = (10.5 * ui_scale).round() as u32;

            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">{}</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="600">&lt;&lt;{}&gt;&gt;</text>"##,
                title_x, y + 20.0 * ui_scale, palette.text_main, title_font, escape_xml(&clean_title),
                title_x, y + 34.0 * ui_scale, role_col, role_font, escape_xml(&role)
            ));

            // Health badge at top right: 🟢 Healthy
            let health_str = "🟢 Healthy";
            let health_font = (10.5 * ui_scale).round() as u32;
            let health_w = 74.0 * ui_scale;
            let health_x = x + w - health_w - 12.0 * ui_scale;
            let health_y = y + 12.0 * ui_scale;
            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="20" rx="4" fill="{}" fill-opacity="0.12" stroke="{}" stroke-width="1"/>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="500" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                health_x, health_y, health_w, palette.public_vis, palette.public_vis,
                health_x + health_w / 2.0, health_y + 10.0, palette.public_vis, health_font, health_str
            ));

            // 2. Tabs Row: Overview │ Contract │ Code │ Runtime │ Logs
            let tabs_y = y + header_h + 8.0 * ui_scale;
            let tabs_h = 26.0 * ui_scale;
            let tab_items = [
                (RightPanelTab::Overview, "Overview"),
                (RightPanelTab::Contract, "Contract"),
                (RightPanelTab::Code, "Code"),
                (RightPanelTab::Runtime, "Runtime"),
                (RightPanelTab::Logs, "Logs"),
            ];

            let tab_w = (w - 20.0 * ui_scale) / 5.0;
            for (idx, (t, label)) in tab_items.iter().enumerate() {
                let is_active = app_state.right_panel_tab == *t;
                let tx = x + 10.0 * ui_scale + (idx as f32 * tab_w);
                let text_col = if is_active {
                    &palette.text_main
                } else {
                    &palette.text_sub
                };
                let weight = if is_active { "bold" } else { "normal" };

                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="{}" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                    tx + tab_w / 2.0, tabs_y + tabs_h / 2.0, text_col, (11.0 * ui_scale).round() as u32, weight, label
                ));

                if is_active {
                    svg.push_str(&format!(
                        r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
                        tx + 2.0,
                        tabs_y + tabs_h,
                        tx + tab_w - 2.0,
                        tabs_y + tabs_h,
                        palette.edge_stroke
                    ));
                }
            }

            svg.push_str(&format!(
                r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
                x,
                tabs_y + tabs_h,
                x + w,
                tabs_y + tabs_h,
                palette.border
            ));

            // 3. Tab Content Area
            let content_y = tabs_y + tabs_h + 12.0 * ui_scale;

            match app_state.right_panel_tab {
                RightPanelTab::Contract => {
                    Self::render_contract_tab(
                        app_state,
                        active_node,
                        x,
                        content_y,
                        w,
                        ui_scale,
                        palette,
                        svg,
                    );
                }
                RightPanelTab::Overview => {
                    Self::render_overview_tab(
                        app_state,
                        active_node,
                        x,
                        content_y,
                        w,
                        ui_scale,
                        palette,
                        svg,
                    );
                }
                RightPanelTab::Runtime => {
                    Self::render_runtime_tab(
                        app_state,
                        active_node,
                        x,
                        content_y,
                        w,
                        ui_scale,
                        palette,
                        svg,
                    );
                }
                RightPanelTab::Logs | RightPanelTab::Code => {
                    Self::render_contract_tab(
                        app_state,
                        active_node,
                        x,
                        content_y,
                        w,
                        ui_scale,
                        palette,
                        svg,
                    );
                }
            }
        } else {
            // Mode B: System Overview Drawer (Panel 4 right sidebar)
            Self::render_system_overview(app_state, x, y, w, ui_scale, palette, svg);
        }
    }

    fn render_contract_tab(
        _app_state: &AppState,
        active_node: Option<&merm_core::engine::DiagramNode>,
        x: f32,
        start_y: f32,
        w: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let card_w = w - 24.0 * ui_scale;
        let card_x = x + 12.0 * ui_scale;
        let font_title = (11.0 * ui_scale).round() as u32;
        let font_code = (10.5 * ui_scale).round() as u32;
        let mut cur_y = start_y;

        let contract = active_node.and_then(|n| n.contract.as_ref());

        let sections = [
            (
                "Input (Expected)",
                contract
                    .and_then(|c| c.input_expected.as_deref())
                    .unwrap_or("{\n  \"user\": \"string\",\n  \"pass\": \"string\"\n}"),
            ),
            (
                "Input (Example)",
                contract
                    .and_then(|c| c.input_example.as_deref())
                    .unwrap_or("{\n  \"user\": \"admin\",\n  \"pass\": \"secret\"\n}"),
            ),
            (
                "Output (Expected)",
                contract
                    .and_then(|c| c.output_expected.as_deref())
                    .unwrap_or("{\n  \"token\": \"string\",\n  \"expires_at\": \"u64\"\n}"),
            ),
            (
                "Output (Default)",
                contract
                    .and_then(|c| c.output_default.as_deref())
                    .unwrap_or("\"{}\""),
            ),
        ];

        for (title, code) in sections {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">{}</text>"##,
                card_x, cur_y + 10.0 * ui_scale, palette.text_main, font_title, title
            ));
            cur_y += 18.0 * ui_scale;

            let lines: Vec<&str> = code.lines().collect();
            let code_h = ((lines.len() as f32 * 14.5 + 12.0) * ui_scale).max(28.0 * ui_scale);

            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"/>"##,
                card_x, cur_y, card_w, code_h, palette.card_bg, palette.divider
            ));

            for (line_idx, line) in lines.iter().enumerate() {
                let ly = cur_y + 13.0 * ui_scale + (line_idx as f32 * 14.5 * ui_scale);
                let trimmed = line.trim();
                if trimmed == "{" || trimmed == "}" {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                        card_x + 10.0 * ui_scale, ly, palette.text_sub, font_code, escape_xml(line)
                    ));
                } else if let Some((k, v)) = line.split_once(':') {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" font-family="monospace" font-size="{}"><tspan fill="#39c5cf">{}</tspan><tspan fill="{}">:</tspan><tspan fill="#e5a93c">{}</tspan></text>"##,
                        card_x + 10.0 * ui_scale, ly, font_code, escape_xml(k), palette.text_sub, escape_xml(v)
                    ));
                } else {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                        card_x + 10.0 * ui_scale, ly, palette.type_color, font_code, escape_xml(line)
                    ));
                }
            }

            cur_y += code_h + 12.0 * ui_scale;
        }

        // Section: Runtime State
        let state_h = 68.0 * ui_scale;
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Runtime State</text>"##,
            card_x, cur_y + 10.0 * ui_scale, palette.text_main, font_title
        ));
        cur_y += 18.0 * ui_scale;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">State</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end"><tspan fill="{}">🟢 Idle</tspan></text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Last Run</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">1.2 ms</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Exit Code</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">0</text>"##,
            card_x, cur_y, card_w, state_h, palette.card_bg, palette.divider,
            card_x + 10.0 * ui_scale, cur_y + 18.0 * ui_scale, palette.text_sub, font_code,
            card_x + card_w - 10.0 * ui_scale, cur_y + 18.0 * ui_scale, palette.public_vis, font_code, palette.public_vis,
            card_x + 10.0 * ui_scale, cur_y + 38.0 * ui_scale, palette.text_sub, font_code,
            card_x + card_w - 10.0 * ui_scale, cur_y + 38.0 * ui_scale, palette.text_main, font_code,
            card_x + 10.0 * ui_scale, cur_y + 56.0 * ui_scale, palette.text_sub, font_code,
            card_x + card_w - 10.0 * ui_scale, cur_y + 56.0 * ui_scale, palette.text_main, font_code
        ));
        cur_y += state_h + 16.0 * ui_scale;

        // Footer Source Link Button: Source  src/services/auth.rs:42  >
        let src_path = contract
            .and_then(|c| c.source_location.as_deref())
            .unwrap_or("src/services/auth.rs:42");
        let btn_h = 28.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" dominant-baseline="central">Source</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="{}" dominant-baseline="central">{}</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="end" dominant-baseline="central">&gt;</text>"##,
            card_x, cur_y, card_w, btn_h, palette.card_header, palette.border,
            card_x + 10.0 * ui_scale, cur_y + btn_h / 2.0, palette.text_sub, (11.0 * ui_scale).round() as u32,
            card_x + 56.0 * ui_scale, cur_y + btn_h / 2.0, palette.edge_stroke, (10.5 * ui_scale).round() as u32, src_path,
            card_x + card_w - 10.0 * ui_scale, cur_y + btn_h / 2.0, palette.text_sub, (12.0 * ui_scale).round() as u32
        ));
    }

    fn render_overview_tab(
        _app_state: &AppState,
        active_node: Option<&merm_core::engine::DiagramNode>,
        x: f32,
        start_y: f32,
        w: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let card_w = w - 24.0 * ui_scale;
        let card_x = x + 12.0 * ui_scale;
        let font_title = (11.5 * ui_scale).round() as u32;
        let font_body = (11.0 * ui_scale).round() as u32;

        let node_id = active_node.map(|n| n.id.as_str()).unwrap_or("Unknown");
        let role = active_node
            .map(|n| n.role())
            .unwrap_or_else(|| "service".to_string());

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Node Overview</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">ID: {}</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">Role: &lt;&lt;{}&gt;&gt;</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Engine: Rust Syn AST Aware</text>"##,
            card_x, start_y, card_w, 140.0 * ui_scale, palette.card_bg, palette.divider,
            card_x + 12.0 * ui_scale, start_y + 22.0 * ui_scale, palette.text_main, font_title,
            card_x + 12.0 * ui_scale, start_y + 50.0 * ui_scale, palette.text_sub, font_body, escape_xml(node_id),
            card_x + 12.0 * ui_scale, start_y + 74.0 * ui_scale, palette.text_sub, font_body, escape_xml(&role),
            card_x + 12.0 * ui_scale, start_y + 98.0 * ui_scale, palette.public_vis, font_body
        ));
    }

    fn render_runtime_tab(
        app_state: &AppState,
        active_node: Option<&merm_core::engine::DiagramNode>,
        x: f32,
        start_y: f32,
        w: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let card_w = w - 24.0 * ui_scale;
        let card_x = x + 12.0 * ui_scale;
        let node_id = active_node.map(|n| n.id.as_str()).unwrap_or("Unknown");

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Test Harness: {}</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">Status: {}</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Press [t] or ':test {}' to execute</text>"##,
            card_x, start_y, card_w, 120.0 * ui_scale, palette.card_bg, palette.divider,
            card_x + 12.0 * ui_scale, start_y + 24.0 * ui_scale, palette.text_main, (12.0 * ui_scale).round() as u32, escape_xml(node_id),
            card_x + 12.0 * ui_scale, start_y + 52.0 * ui_scale, palette.public_vis, (11.0 * ui_scale).round() as u32,
            if app_state.is_busy { "Running..." } else { "Idle (Ready)" },
            card_x + 12.0 * ui_scale, start_y + 80.0 * ui_scale, palette.text_sub, (10.5 * ui_scale).round() as u32, escape_xml(node_id)
        ));
    }

    fn render_system_overview(
        app_state: &AppState,
        x: f32,
        y: f32,
        w: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let card_w = w - 24.0 * ui_scale;
        let card_x = x + 12.0 * ui_scale;
        let font_header = (12.5 * ui_scale).round() as u32;
        let font_item = (11.5 * ui_scale).round() as u32;
        let mut cur_y = y + 16.0 * ui_scale;

        // 1. Section: System Overview
        let (node_count, edge_count) = if let Some(ref d) = app_state.current_diagram {
            (d.nodes.len(), d.edges.len())
        } else {
            (12, 14)
        };

        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">System Overview</text>"##,
            card_x, cur_y + 12.0 * ui_scale, palette.text_main, font_header
        ));
        cur_y += 24.0 * ui_scale;

        let sys_h = 100.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Nodes</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">{}</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Edges</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">{}</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Services</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">6</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">Databases</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">2</text>"##,
            card_x, cur_y, card_w, sys_h, palette.card_bg, palette.divider,
            card_x + 12.0 * ui_scale, cur_y + 22.0 * ui_scale, palette.text_sub, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 22.0 * ui_scale, palette.text_main, font_item, node_count,
            card_x + 12.0 * ui_scale, cur_y + 44.0 * ui_scale, palette.text_sub, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 44.0 * ui_scale, palette.text_main, font_item, edge_count,
            card_x + 12.0 * ui_scale, cur_y + 66.0 * ui_scale, palette.text_sub, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 66.0 * ui_scale, palette.text_main, font_item,
            card_x + 12.0 * ui_scale, cur_y + 88.0 * ui_scale, palette.text_sub, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 88.0 * ui_scale, palette.text_main, font_item
        ));
        cur_y += sys_h + 18.0 * ui_scale;

        // 2. Section: Runtime Health
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Runtime Health</text>"##,
            card_x, cur_y + 12.0 * ui_scale, palette.text_main, font_header
        ));
        cur_y += 24.0 * ui_scale;

        let health_card_h = 80.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">🟢 Healthy</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">10</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">🟡 Warning</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">1</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">🔴 Error</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="end">1</text>"##,
            card_x, cur_y, card_w, health_card_h, palette.card_bg, palette.divider,
            card_x + 12.0 * ui_scale, cur_y + 22.0 * ui_scale, palette.text_main, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 22.0 * ui_scale, palette.text_main, font_item,
            card_x + 12.0 * ui_scale, cur_y + 46.0 * ui_scale, palette.text_main, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 46.0 * ui_scale, palette.text_main, font_item,
            card_x + 12.0 * ui_scale, cur_y + 70.0 * ui_scale, palette.text_main, font_item,
            card_x + card_w - 12.0 * ui_scale, cur_y + 70.0 * ui_scale, palette.text_main, font_item
        ));
        cur_y += health_card_h + 18.0 * ui_scale;

        // 3. Section: Zoom Level Slider
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Zoom Level</text>"##,
            card_x, cur_y + 12.0 * ui_scale, palette.text_main, font_header
        ));
        cur_y += 24.0 * ui_scale;

        let zoom_pct = (app_state.transform.scale * 100.0).round() as u32;
        let slider_track_w = card_w - 50.0 * ui_scale;
        let knob_x = card_x + (slider_track_w * (app_state.transform.scale.clamp(0.2, 2.5) / 2.5));
        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="4" stroke-linecap="round"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="4" stroke-linecap="round"/>
            <circle cx="{}" cy="{}" r="7" fill="{}" stroke="{}" stroke-width="2"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" dominant-baseline="central">{}%</text>"##,
            card_x, cur_y + 8.0, card_x + slider_track_w, cur_y + 8.0, palette.divider,
            card_x, cur_y + 8.0, knob_x, cur_y + 8.0, palette.edge_stroke,
            knob_x, cur_y + 8.0, palette.card_bg, palette.edge_stroke,
            card_x + card_w - 38.0 * ui_scale, cur_y + 8.0, palette.text_main, font_item, zoom_pct
        ));
        cur_y += 34.0 * ui_scale;

        // 4. Section: Layout
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Layout</text>"##,
            card_x, cur_y + 12.0 * ui_scale, palette.text_main, font_header
        ));
        cur_y += 24.0 * ui_scale;

        let algos = [
            (LayoutAlgorithm::Hierarchical, "Hierarchical"),
            (LayoutAlgorithm::ForceDirected, "Force Directed"),
            (LayoutAlgorithm::Grid, "Grid"),
        ];

        let btn_h = 28.0 * ui_scale;
        for (algo, label) in algos {
            let is_sel = app_state.layout_algorithm == algo;
            let bg_fill = if is_sel {
                "#1a3d66"
            } else {
                palette.card_bg.as_str()
            };
            let border_col = if is_sel {
                palette.edge_stroke.as_str()
            } else {
                palette.divider.as_str()
            };
            let text_col = if is_sel {
                "#ffffff"
            } else {
                palette.text_main.as_str()
            };
            let weight = if is_sel { "bold" } else { "500" };

            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" stroke="{}" stroke-width="1"/>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="{}" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                card_x, cur_y, card_w, btn_h, bg_fill, border_col,
                card_x + card_w / 2.0, cur_y + btn_h / 2.0, text_col, (11.0 * ui_scale).round() as u32, weight, label
            ));
            cur_y += btn_h + 8.0 * ui_scale;
        }
    }

    fn render_canvas_floating_tools(
        app_state: &AppState,
        center_x: f32,
        center_y: f32,
        center_w: f32,
        center_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        // 1. Bottom-Left Floating Toolbar Pill: [ ↖ ✋ 🔍 ⛶ ⤢ ]
        let toolbar_w = 160.0 * ui_scale;
        let toolbar_h = 32.0 * ui_scale;
        let toolbar_x = center_x + 20.0 * ui_scale;
        let toolbar_y = center_y + center_h - toolbar_h - 16.0 * ui_scale;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="7" fill="{}" opacity="0.96" stroke="{}" stroke-width="1"/>"##,
            toolbar_x, toolbar_y, toolbar_w, toolbar_h, palette.card_bg, palette.border
        ));

        let tools = [
            (CanvasTool::Pointer, "↖"),
            (CanvasTool::Pan, "✋"),
            (CanvasTool::Zoom, "🔍"),
            (CanvasTool::Fit, "⛶"),
            (CanvasTool::Fullscreen, "⤢"),
        ];

        let tool_slot_w = toolbar_w / tools.len() as f32;
        for (i, (t, icon)) in tools.iter().enumerate() {
            let bx = toolbar_x + i as f32 * tool_slot_w;
            let is_active = app_state.active_tool == *t;
            if is_active {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="{}" fill-opacity="0.8"/>"##,
                    bx + 2.0, toolbar_y + 2.0, tool_slot_w - 4.0, toolbar_h - 4.0, palette.card_header
                ));
            }
            let col = if is_active {
                &palette.text_main
            } else {
                &palette.text_sub
            };
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-size="{}" text-anchor="middle" dominant-baseline="central">{}</text>"##,
                bx + tool_slot_w / 2.0, toolbar_y + toolbar_h / 2.0, col, (13.0 * ui_scale).round() as u32, icon
            ));
        }

        // 2. Bottom-Right Floating Minimap
        let minimap_w = 150.0 * ui_scale;
        let minimap_h = 98.0 * ui_scale;
        let minimap_x = center_x + center_w - minimap_w - 20.0 * ui_scale;
        let minimap_y = center_y + center_h - minimap_h - 16.0 * ui_scale;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" opacity="0.94" stroke="{}" stroke-width="1"/>"##,
            minimap_x, minimap_y, minimap_w, minimap_h, palette.background, palette.border
        ));

        // Draw miniature node boxes inside minimap
        if let Some(ref diag) = app_state.current_diagram {
            let dw = diag.width.max(1.0);
            let dh = diag.height.max(1.0);
            let scale_x = (minimap_w - 16.0 * ui_scale) / dw;
            let scale_y = (minimap_h - 16.0 * ui_scale) / dh;
            let mini_scale = scale_x.min(scale_y);

            let offset_mini_x = minimap_x + 8.0 * ui_scale;
            let offset_mini_y = minimap_y + 8.0 * ui_scale;

            for n in &diag.nodes {
                let mx = offset_mini_x + n.x * mini_scale;
                let my = offset_mini_y + n.y * mini_scale;
                let mw = (n.width * mini_scale).max(4.0);
                let mh = (n.height * mini_scale).max(3.0);
                let role = n.role();
                let col = palette.node_role_color(&role, &n.clean_title());

                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="1" fill="{}" opacity="0.85"/>"##,
                    mx, my, mw, mh, col
                ));
            }

            // Viewport bounds indicator rectangle inside minimap
            let view_mw = (minimap_w * 0.45).min(minimap_w - 10.0);
            let view_mh = (minimap_h * 0.45).min(minimap_h - 10.0);
            svg.push_str(&format!(
                r##"<rect x="{}" y="{}" width="{}" height="{}" fill="none" stroke="{}" stroke-width="1.5" stroke-dasharray="3,2"/>"##,
                minimap_x + (minimap_w - view_mw) / 2.0, minimap_y + (minimap_h - view_mh) / 2.0,
                view_mw, view_mh, palette.edge_stroke
            ));
        }
    }

    fn render_ast_split_view(
        app_state: &AppState,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let left_w = w * 0.52;
        let right_w = w - left_w;

        // Left Pane: Source Code Editor (Panel 3 left)
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            x,
            y,
            left_w,
            h,
            palette.background,
            x + left_w,
            y,
            x + left_w,
            y + h,
            palette.border
        ));

        // Editor Tab Bar
        let tab_bar_h = 32.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            x,
            y,
            left_w,
            tab_bar_h,
            palette.card_bg,
            x,
            y + tab_bar_h,
            x + left_w,
            y + tab_bar_h,
            palette.border
        ));

        let active_tab_w = 170.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold" dominant-baseline="central">{}</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end" dominant-baseline="central">✕</text>"##,
            x, y, active_tab_w, tab_bar_h, palette.background,
            x, y + tab_bar_h, x + active_tab_w, y + tab_bar_h, palette.edge_stroke,
            x + 12.0 * ui_scale, y + tab_bar_h / 2.0, palette.text_main, (11.0 * ui_scale).round() as u32, escape_xml(&app_state.active_code_file),
            x + active_tab_w - 12.0 * ui_scale, y + tab_bar_h / 2.0, palette.text_sub, (10.0 * ui_scale).round() as u32
        ));

        // Editor Code Area & Line Numbers
        let line_num_w = 40.0 * ui_scale;
        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            x + line_num_w,
            y + tab_bar_h,
            x + line_num_w,
            y + h,
            palette.divider
        ));

        let editor_foot_h = 24.0 * ui_scale;

        // Line numbers & Syntax-colored code
        let line_h = 19.0 * ui_scale;
        let font_code = (11.5 * ui_scale).round() as u32;
        let lines: Vec<&str> = app_state.active_code_content.lines().collect();

        for (i, line) in lines.iter().enumerate().take(25) {
            let ly = y + tab_bar_h + 16.0 * ui_scale + (i as f32 * line_h);

            // Line number
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">{}</text>"##,
                x + line_num_w - 8.0 * ui_scale, ly, palette.text_muted, font_code, i + 1
            ));

            let trimmed = line.trim();
            if trimmed.starts_with("//") {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-style="italic">{}</text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, palette.comment_color, font_code, escape_xml(line)
                ));
            } else if trimmed.starts_with("#[") {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="#79c0ff" font-family="monospace" font-size="{}">{}</text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, font_code, escape_xml(line)
                ));
            } else if trimmed.starts_with("use ") {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="monospace" font-size="{}"><tspan fill="#ff7b72" font-weight="bold">use </tspan><tspan fill="#79c0ff">{}</tspan></text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, font_code, escape_xml(trimmed.strip_prefix("use ").unwrap_or(""))
                ));
            } else if trimmed.starts_with("pub struct ") {
                let name = trimmed
                    .strip_prefix("pub struct ")
                    .unwrap_or("")
                    .trim_end_matches('{')
                    .trim();
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="monospace" font-size="{}"><tspan fill="#ff7b72" font-weight="bold">pub struct </tspan><tspan fill="#7ee787" font-weight="bold">{}</tspan><tspan fill="{}"> {{</tspan></text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, font_code, escape_xml(name), palette.text_sub
                ));
            } else if trimmed.starts_with("pub enum ") {
                let name = trimmed
                    .strip_prefix("pub enum ")
                    .unwrap_or("")
                    .trim_end_matches('{')
                    .trim();
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" font-family="monospace" font-size="{}"><tspan fill="#ff7b72" font-weight="bold">pub enum </tspan><tspan fill="#7ee787" font-weight="bold">{}</tspan><tspan fill="{}"> {{</tspan></text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, font_code, escape_xml(name), palette.text_sub
                ));
            } else if trimmed.starts_with("pub ") && trimmed.contains(':') {
                if let Some((field_part, type_part)) = trimmed.split_once(':') {
                    let field = field_part.strip_prefix("pub ").unwrap_or(field_part).trim();
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" font-family="monospace" font-size="{}"><tspan fill="{}">    pub </tspan><tspan fill="#79c0ff">{}</tspan><tspan fill="{}">:</tspan><tspan fill="#39c5cf">{}</tspan></text>"##,
                        x + line_num_w + 12.0 * ui_scale, ly, font_code, palette.text_sub, escape_xml(field), palette.text_sub, escape_xml(type_part)
                    ));
                } else {
                    svg.push_str(&format!(
                        r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                        x + line_num_w + 12.0 * ui_scale, ly, palette.text_main, font_code, escape_xml(line)
                    ));
                }
            } else if trimmed == "}" {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">}}</text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, palette.text_sub, font_code
                ));
            } else {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                    x + line_num_w + 12.0 * ui_scale, ly, palette.text_main, font_code, escape_xml(line)
                ));
            }
        }

        // Bottom Editor Footer: Rust  Ln 1, Col 1  UTF-8
        let editor_foot_y = y + h - editor_foot_h;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" dominant-baseline="central">Rust</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="middle" dominant-baseline="central">Ln 1, Col 1</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="end" dominant-baseline="central">UTF-8</text>"##,
            x, editor_foot_y, left_w, editor_foot_h, palette.card_bg,
            x, editor_foot_y, x + left_w, editor_foot_y, palette.border,
            x + 14.0 * ui_scale, editor_foot_y + editor_foot_h / 2.0, palette.text_sub, (10.5 * ui_scale).round() as u32,
            x + left_w / 2.0, editor_foot_y + editor_foot_h / 2.0, palette.text_muted, (10.5 * ui_scale).round() as u32,
            x + left_w - 14.0 * ui_scale, editor_foot_y + editor_foot_h / 2.0, palette.text_muted, (10.5 * ui_scale).round() as u32
        ));

        // Right Pane: Interactive AST View (Panel 3 right)
        let rx = x + left_w;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>"##,
            rx, y, right_w, h, palette.background
        ));

        // AST View Header
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" fill="{}"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" dominant-baseline="central">AST View</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" text-anchor="end" dominant-baseline="central">✕</text>"##,
            rx, y, right_w, tab_bar_h, palette.card_bg,
            rx, y + tab_bar_h, rx + right_w, y + tab_bar_h, palette.border,
            rx + 14.0 * ui_scale, y + tab_bar_h / 2.0, palette.text_main, (12.5 * ui_scale).round() as u32,
            rx + right_w - 14.0 * ui_scale, y + tab_bar_h / 2.0, palette.text_sub, (11.0 * ui_scale).round() as u32
        ));

        // AST Tree Hierarchy matching Panel 3
        let mut tree_y = y + tab_bar_h + 16.0 * ui_scale;
        let tree_items: [(&str, &str, &str, f32); 12] = [
            ("📦", "Crate", palette.text_main.as_str(), 0.0),
            ("📁", "modules", palette.text_sub.as_str(), 14.0),
            ("📁", "services", palette.text_sub.as_str(), 28.0),
            ("📁", "auth", palette.text_sub.as_str(), 42.0),
            ("⬚", "AuthRequest (struct)", "#39c5cf", 56.0),
            ("⬚", "AuthResponse (struct)", "#39c5cf", 56.0),
            ("⬚", "AuthError (enum)", "#ff7b72", 56.0),
            ("⬚", "AuthService (struct)", "#e5a93c", 56.0),
            ("⊟", "impl AuthService", palette.text_main.as_str(), 56.0),
            ("⊙", "new()", "#7ee787", 72.0),
            ("⊙", "login()", "#7ee787", 72.0),
            ("⊙", "validate_token()", "#7ee787", 72.0),
        ];

        for (icon, label, col, indent) in tree_items {
            let tx = rx + 14.0 * ui_scale + (indent * ui_scale);
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-size="11" dominant-baseline="central">{}</text>
                <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="{}" dominant-baseline="central">{}</text>"##,
                tx, tree_y, col, icon,
                tx + 16.0 * ui_scale, tree_y, col, (11.0 * ui_scale).round() as u32, escape_xml(label)
            ));
            tree_y += 20.0 * ui_scale;
        }

        // Bottom Card: Symbol Info
        let info_h = 100.0 * ui_scale;
        let info_y = y + h - info_h - 14.0 * ui_scale;
        let info_w = right_w - 28.0 * ui_scale;
        let info_x = rx + 14.0 * ui_scale;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">Symbol Info</text>
            <rect x="{}" y="{}" width="22" height="22" rx="4" fill="#e5a93c" fill-opacity="0.15" stroke="#e5a93c" stroke-width="1"/>
            <text x="{}" y="{}" fill="#e5a93c" font-size="12" text-anchor="middle" dominant-baseline="central">⬚</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold">AuthService</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}">struct</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">Path: crate::services::auth::AuthService</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">Location: src/services/auth.rs:42</text>"##,
            info_x, info_y, info_w, info_h, palette.card_bg, palette.divider,
            info_x + 12.0 * ui_scale, info_y + 16.0 * ui_scale, palette.text_sub, (10.5 * ui_scale).round() as u32,
            info_x + 12.0 * ui_scale, info_y + 26.0 * ui_scale,
            info_x + 23.0 * ui_scale, info_y + 37.0 * ui_scale,
            info_x + 40.0 * ui_scale, info_y + 35.0 * ui_scale, palette.text_main, (12.0 * ui_scale).round() as u32,
            info_x + 40.0 * ui_scale, info_y + 48.0 * ui_scale, palette.type_color, (10.0 * ui_scale).round() as u32,
            info_x + 12.0 * ui_scale, info_y + 68.0 * ui_scale, palette.text_sub, (10.0 * ui_scale).round() as u32,
            info_x + 12.0 * ui_scale, info_y + 84.0 * ui_scale, palette.text_muted, (10.0 * ui_scale).round() as u32
        ));
    }

    fn render_command_palette(
        app_state: &AppState,
        w: f32,
        h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        // Dimming Backdrop
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{}" height="{}" fill="#000000" opacity="0.65"/>"##,
            w, h
        ));

        let card_w = 460.0 * ui_scale;
        let card_h = 285.0 * ui_scale;
        let card_x = (w - card_w) / 2.0;
        let card_y = (h - card_h) / 2.0 - 40.0 * ui_scale;

        // Elevated Palette Card
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="1.5"/>"##,
            card_x, card_y, card_w, card_h, palette.card_bg, palette.border
        ));

        // Search Input Field: :|
        let input_h = 42.0 * ui_scale;
        let prompt_str = if app_state.modal.command_buffer.is_empty() {
            ":|"
        } else {
            &app_state.modal.command_buffer
        };

        svg.push_str(&format!(
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold" dominant-baseline="central">{}</text>"##,
            card_x,
            card_y + input_h,
            card_x + card_w,
            card_y + input_h,
            palette.divider,
            card_x + 16.0 * ui_scale,
            card_y + input_h / 2.0,
            palette.text_main,
            (14.0 * ui_scale).round() as u32,
            escape_xml(prompt_str)
        ));

        // Command Items List matching Panel 4
        let commands = [
            (":open", "Open file in editor", "⌘ O"),
            (":build", "Build the project", "⌘ B"),
            (":test", "Run tests", "⌘ T"),
            (":run", "Execute diagram", "⌘ R"),
            (":theme", "Change theme", "⌘ T"),
            (":help", "Show help", "⌘ ?"),
        ];

        let item_h = 36.0 * ui_scale;
        let mut item_y = card_y + input_h + 8.0 * ui_scale;

        for (idx, (cmd, desc, shortcut)) in commands.iter().enumerate() {
            let is_sel = idx == app_state.command_palette_selected_idx;
            if is_sel {
                svg.push_str(&format!(
                    r##"<rect x="{}" y="{}" width="{}" height="{}" rx="5" fill="#163b65"/>"##,
                    card_x + 8.0 * ui_scale,
                    item_y,
                    card_w - 16.0 * ui_scale,
                    item_h
                ));
            }

            let cmd_col = if is_sel {
                "#ffffff"
            } else {
                palette.text_main.as_str()
            };
            let desc_col = if is_sel {
                "#d0e2ff"
            } else {
                palette.text_sub.as_str()
            };
            let short_col = if is_sel {
                "#79c0ff"
            } else {
                palette.text_muted.as_str()
            };

            let font_cmd = (12.0 * ui_scale).round() as u32;
            let font_desc = (11.0 * ui_scale).round() as u32;

            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold" dominant-baseline="central">{}</text>
                <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" dominant-baseline="central">{}</text>
                <text x="{}" y="{}" fill="{}" font-family="monospace, sans-serif" font-size="{}" text-anchor="end" dominant-baseline="central">{}</text>"##,
                card_x + 18.0 * ui_scale, item_y + item_h / 2.0, cmd_col, font_cmd, cmd,
                card_x + 88.0 * ui_scale, item_y + item_h / 2.0, desc_col, font_desc, desc,
                card_x + card_w - 18.0 * ui_scale, item_y + item_h / 2.0, short_col, font_cmd, shortcut
            ));

            item_y += item_h + 2.0 * ui_scale;
        }
    }

    fn render_split_buffer(
        app_state: &AppState,
        w: f32,
        h: f32,
        bottom_status_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let split_h = (h * 0.45).clamp(200.0 * ui_scale, h - 80.0 * ui_scale);
        let split_y = h - bottom_status_h - split_h;

        // Background
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}" opacity="0.98"/>
            <line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>"##,
            split_y, w, split_h, palette.card_bg, split_y, w, split_y, palette.edge_stroke
        ));

        // Header
        let head_h = 26.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" dominant-baseline="central">🤖 AI Architecture &amp; Diagnostic Buffer [AI Chat Buffer]</text>
            <rect x="{}" y="{}" width="80" height="20" rx="4" fill="{}" stroke="{}" stroke-width="1"/>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="10.5" text-anchor="middle" dominant-baseline="central">📋 Kopyala</text>
            <text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="12" text-anchor="middle" dominant-baseline="central">✕</text>"##,
            split_y, w, head_h, palette.card_header,
            14.0 * ui_scale, split_y + head_h / 2.0, palette.edge_stroke, (11.5 * ui_scale).round() as u32,
            w - 110.0 * ui_scale, split_y + 3.0 * ui_scale, palette.card_bg, palette.edge_stroke,
            w - 70.0 * ui_scale, split_y + head_h / 2.0, palette.edge_stroke,
            w - 18.0 * ui_scale, split_y + head_h / 2.0, palette.private_vis
        ));

        // Content lines
        if let Some(ref content) = app_state.report_content {
            let lines: Vec<&str> = content.lines().collect();
            let line_h = 16.0 * ui_scale;
            let mut ly = split_y + head_h + 12.0 * ui_scale;

            for line in lines
                .iter()
                .skip(app_state.modal.report_scroll_offset)
                .take(15)
            {
                svg.push_str(&format!(
                    r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
                    16.0 * ui_scale, ly, palette.text_main, (11.0 * ui_scale).round() as u32, escape_xml(line)
                ));
                ly += line_h;
            }
        }
    }

    fn render_status_bar(
        app_state: &AppState,
        w: f32,
        h: f32,
        status_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let status_y = h - status_h;
        let font_status = (10.5 * ui_scale).round() as u32;

        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>
            <line x1="0" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="1"/>"##,
            status_y, w, status_h, palette.card_bg, status_y, w, status_y, palette.border
        ));

        // Mode badge: [NORMAL]
        let mode_str = match app_state.modal.mode {
            UiMode::Command => "COMMAND",
            UiMode::Report => "REPORT",
            _ => "NORMAL",
        };
        let mode_w = 64.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="0" y="{}" width="{}" height="{}" fill="{}"/>
            <text x="{}" y="{}" fill="#0e1117" font-family="system-ui, sans-serif" font-size="{}" font-weight="bold" text-anchor="middle" dominant-baseline="central">{}</text>"##,
            status_y, mode_w, status_h, palette.edge_stroke,
            mode_w / 2.0, status_y + status_h / 2.0, font_status, mode_str
        ));

        // Status text: Project: backend │ Merm Studio Dark │ WGPU 120FPS │ Zoom: 1.0x │ Ready
        let status_left = if let Some(ref sel_id) = app_state.active_node_id {
            format!(
                "Project: {} │ {} │ WGPU 120FPS │ 🎯 SELECTED: {} │ {}",
                app_state.active_workspace, palette.name, sel_id, app_state.status_message
            )
        } else {
            format!(
                "Project: {} │ {} │ WGPU 120FPS │ Zoom: {:.1}x │ {}",
                app_state.active_workspace,
                palette.name,
                app_state.transform.scale,
                app_state.status_message
            )
        };

        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="system-ui, sans-serif" font-size="{}" dominant-baseline="central">{}</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end" dominant-baseline="central">:help ⌘? │ &amp;check</text>"##,
            mode_w + 12.0 * ui_scale, status_y + status_h / 2.0, palette.text_sub, font_status, escape_xml(&status_left),
            w - 12.0 * ui_scale, status_y + status_h / 2.0, palette.text_muted, font_status
        ));
    }

    fn render_telemetry(
        app_state: &AppState,
        w: f32,
        top_h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let card_w = 260.0 * ui_scale;
        let card_h = 138.0 * ui_scale;
        let card_x = w - card_w - 14.0 * ui_scale;
        let card_y = top_h + 10.0 * ui_scale;

        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="6" fill="{}" fill-opacity="0.95" stroke="{}" stroke-width="1.5"/>"##,
            card_x, card_y, card_w, card_h, palette.card_bg, palette.edge_stroke
        ));

        let telem_title_y = card_y + 18.0 * ui_scale;
        let telem_font = (10.5 * ui_scale).round() as u32;
        let line_gap = 18.0 * ui_scale;

        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">⚡ PERFORMANCE TELEMETRY</text>"##,
            card_x + 12.0 * ui_scale, telem_title_y, palette.edge_stroke, telem_font
        ));

        let fps_color = if app_state.telemetry.fps >= 100.0 {
            "#3fb950"
        } else if app_state.telemetry.fps >= 60.0 {
            "#d29922"
        } else {
            "#f85149"
        };

        let row1 = format!(
            "FPS: {:>5.1} │ Frame: {:>4.1}ms",
            app_state.telemetry.fps, app_state.telemetry.frame_time_ms
        );
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
            card_x + 12.0 * ui_scale,
            telem_title_y + line_gap,
            fps_color,
            telem_font,
            escape_xml(&row1)
        ));

        let row2 = format!(
            "Update: {:>4.1}ms │ Render: {:>4.1}ms",
            app_state.telemetry.update_time_ms, app_state.telemetry.render_time_ms
        );
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
            card_x + 12.0 * ui_scale,
            telem_title_y + line_gap * 2.0,
            palette.text_main,
            telem_font,
            escape_xml(&row2)
        ));

        let row3 = format!(
            "Nodes: {}/{} │ Cache: {:>4.1}%",
            app_state.telemetry.visible_nodes,
            app_state.telemetry.total_nodes,
            app_state.telemetry.cache_hit_rate
        );
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
            card_x + 12.0 * ui_scale,
            telem_title_y + line_gap * 3.0,
            palette.stereotype_color,
            telem_font,
            escape_xml(&row3)
        ));
    }

    fn render_node_edit(
        app_state: &AppState,
        w: f32,
        h: f32,
        ui_scale: f32,
        palette: &merm_core::ColorPalette,
        svg: &mut String,
    ) {
        let modal_w = (w - 140.0 * ui_scale).clamp(520.0 * ui_scale, 860.0 * ui_scale);
        let modal_h = (h - 100.0 * ui_scale).clamp(380.0 * ui_scale, 620.0 * ui_scale);
        let modal_x = (w - modal_w) / 2.0;
        let modal_y = (h - modal_h) / 2.0;

        // Dimmed Backdrop
        svg.push_str(&format!(
            r##"<rect x="0" y="0" width="{}" height="{}" fill="#000000" opacity="0.70"/>
            <rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}" stroke="{}" stroke-width="2"/>"##,
            w, h, modal_x, modal_y, modal_w, modal_h, palette.background, palette.edge_stroke
        ));

        let header_h = 38.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="{}"/>
            <line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" stroke-width="2"/>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">✏️ Node Editor: &lt;{}&gt;</text>
            <text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" text-anchor="end">[Enter] Add/Save │ [Esc] Cancel</text>"##,
            modal_x, modal_y, modal_w, header_h, palette.badge_bg,
            modal_x, modal_y + header_h, modal_x + modal_w, modal_y + header_h, palette.edge_stroke,
            modal_x + 16.0 * ui_scale, modal_y + 25.0 * ui_scale, palette.edge_stroke, (14.0 * ui_scale).round() as u32, escape_xml(&app_state.modal.edit_node_id),
            modal_x + modal_w - 16.0 * ui_scale, modal_y + 25.0 * ui_scale, palette.text_sub, (11.0 * ui_scale).round() as u32
        ));

        let mut cur_y = modal_y + header_h + 24.0 * ui_scale;
        let st_display = if !app_state.modal.edit_stereotype.is_empty() {
            format!(
                "&lt;&lt;{}&gt;&gt;",
                escape_xml(&app_state.modal.edit_stereotype)
            )
        } else {
            "&lt;&lt;class&gt;&gt; (default)".to_string()
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">Stereotype: {}</text>"##,
            modal_x + 20.0 * ui_scale, cur_y, palette.stereotype_color, (12.0 * ui_scale).round() as u32, st_display
        ));
        cur_y += 24.0 * ui_scale;

        // Members List
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}" font-weight="bold">Members ({}):</text>"##,
            modal_x + 20.0 * ui_scale, cur_y, palette.text_main, (12.0 * ui_scale).round() as u32, app_state.modal.edit_members.len()
        ));
        cur_y += 18.0 * ui_scale;

        let member_line_h = 20.0 * ui_scale;
        for (i, m) in app_state.modal.edit_members.iter().enumerate().take(6) {
            svg.push_str(&format!(
                r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">  {:2}. {}</text>"##,
                modal_x + 24.0 * ui_scale, cur_y, palette.method_color, (12.0 * ui_scale).round() as u32, i + 1, escape_xml(m)
            ));
            cur_y += member_line_h;
        }

        // Input Box
        cur_y += 12.0 * ui_scale;
        let input_h = 32.0 * ui_scale;
        svg.push_str(&format!(
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="4" fill="{}" stroke="{}" stroke-width="1.5"/>"##,
            modal_x + 20.0 * ui_scale, cur_y, modal_w - 40.0 * ui_scale, input_h, palette.card_bg, palette.edge_stroke
        ));
        let input_str = if app_state.modal.edit_input_buffer.is_empty() {
            "Type member and press Enter. Enter on empty line saves.█".to_string()
        } else {
            format!("{}█", escape_xml(&app_state.modal.edit_input_buffer))
        };
        svg.push_str(&format!(
            r##"<text x="{}" y="{}" fill="{}" font-family="monospace" font-size="{}">{}</text>"##,
            modal_x + 30.0 * ui_scale,
            cur_y + 21.0 * ui_scale,
            palette.text_main,
            (12.0 * ui_scale).round() as u32,
            input_str
        ));
    }
}

pub struct StudioHitTester;

impl StudioHitTester {
    pub fn handle_mouse_click(
        app_state: &mut AppState,
        cx: f64,
        cy: f64,
        win_w: u32,
        win_h: u32,
    ) -> Option<UiAction> {
        let w = win_w as f64;
        let h = win_h as f64;

        let ui_scale = if w >= 2500.0 || h >= 1500.0 {
            1.85f64
        } else if w >= 1800.0 || h >= 1000.0 {
            1.35f64
        } else {
            1.0f64
        };

        let top_h = 36.0 * ui_scale;
        let bottom_status_h = 24.0 * ui_scale;
        let sidebar_w = if app_state.show_left_sidebar {
            210.0 * ui_scale
        } else {
            0.0
        };
        let inspector_w = if app_state.show_right_panel {
            300.0 * ui_scale
        } else {
            0.0
        };

        // 1. Command Palette clicks (if visible)
        if app_state.command_palette_visible || app_state.modal.mode == UiMode::Command {
            let card_w = 480.0 * ui_scale;
            let card_h = 300.0 * ui_scale;
            let card_x = (w - card_w) / 2.0;
            let card_y = (h - card_h) / 2.0 - 40.0 * ui_scale;

            if cx < card_x || cx > card_x + card_w || cy < card_y || cy > card_y + card_h {
                app_state.close_command_palette();
                return Some(UiAction::None);
            }

            // Check click on items
            let input_h = 42.0 * ui_scale;
            let item_h = 36.0 * ui_scale;
            if cy >= card_y + input_h && cy <= card_y + card_h {
                let clicked_idx = ((cy - (card_y + input_h)) / item_h).floor() as usize;
                match clicked_idx {
                    0 => {
                        app_state.close_command_palette();
                        app_state.status_message = "Open workspace/file".to_string();
                    }
                    1 => {
                        app_state.close_command_palette();
                        app_state.execute_command_str("&check");
                    }
                    2 => {
                        app_state.close_command_palette();
                        if let Some(ref nid) = app_state.active_node_id.clone() {
                            app_state.execute_node_test(nid, "{}");
                        }
                    }
                    3 => {
                        app_state.close_command_palette();
                        if let Some(ref nid) = app_state.active_node_id.clone() {
                            app_state.execute_node_test(nid, "{}");
                        }
                    }
                    4 => {
                        app_state.close_command_palette();
                        app_state.theme = app_state.theme.next();
                        app_state.recalculate_diagram();
                    }
                    5 => {
                        app_state.close_command_palette();
                        app_state.execute_command_str(":help");
                    }
                    _ => {}
                }
                return Some(UiAction::None);
            }
            return Some(UiAction::None);
        }

        // 2. Top Window Bar clicks
        if cy <= top_h {
            // macOS Red dot
            if cx <= 24.0 * ui_scale {
                return Some(UiAction::Quit);
            }
            // macOS Yellow dot -> toggle left sidebar
            if cx > 24.0 * ui_scale && cx <= 38.0 * ui_scale {
                app_state.toggle_sidebar();
                return Some(UiAction::None);
            }
            // macOS Green dot -> toggle right inspector / fullscreen
            if cx > 38.0 * ui_scale && cx <= 56.0 * ui_scale {
                app_state.toggle_right_panel();
                return Some(UiAction::None);
            }
            // Brand Logo: ⬡ merm -> toggle command palette
            if cx > 56.0 * ui_scale && cx <= 135.0 * ui_scale {
                app_state.open_command_palette();
                return Some(UiAction::None);
            }
            // Breadcrumb -> switch between Diagrams and AST View
            if cx > 135.0 * ui_scale && cx <= 300.0 * ui_scale {
                if app_state.active_sidebar_tab == SidebarTab::AstView {
                    app_state.set_sidebar_tab(SidebarTab::Diagrams);
                } else {
                    app_state.set_sidebar_tab(SidebarTab::AstView);
                }
                return Some(UiAction::None);
            }
            // Zoom pill
            if cx >= w - 175.0 * ui_scale && cx <= w - 105.0 * ui_scale {
                if let Some(ref diag) = app_state.current_diagram {
                    app_state.transform.fit_to_viewport(
                        diag.width,
                        diag.height,
                        w as f32,
                        h as f32,
                    );
                }
                return Some(UiAction::None);
            }
            // Fit pill
            if cx >= w - 105.0 * ui_scale && cx <= w - 72.0 * ui_scale {
                if let Some(ref diag) = app_state.current_diagram {
                    app_state.transform.fit_to_viewport(
                        diag.width,
                        diag.height,
                        w as f32,
                        h as f32,
                    );
                }
                return Some(UiAction::None);
            }
            // Settings pill
            if cx >= w - 72.0 * ui_scale && cx <= w - 40.0 * ui_scale {
                app_state.open_command_palette();
                return Some(UiAction::None);
            }
            // Theme pill
            if cx >= w - 40.0 * ui_scale {
                app_state.theme = app_state.theme.next();
                app_state.recalculate_diagram();
                return Some(UiAction::None);
            }
            return Some(UiAction::None);
        }

        // 3. Left Sidebar clicks
        if app_state.show_left_sidebar && cx <= sidebar_w && cy < h - bottom_status_h {
            let item_h = 32.0 * ui_scale;
            let start_nav_y = top_h + 12.0 * ui_scale;

            if cy >= start_nav_y && cy <= start_nav_y + item_h * 5.0 {
                let idx = ((cy - start_nav_y) / item_h).floor() as usize;
                match idx {
                    0 => app_state.set_sidebar_tab(SidebarTab::Explorer),
                    1 => app_state.set_sidebar_tab(SidebarTab::Diagrams),
                    2 => app_state.set_sidebar_tab(SidebarTab::AstView),
                    3 => app_state.set_sidebar_tab(SidebarTab::Executions),
                    4 => app_state.set_sidebar_tab(SidebarTab::Settings),
                    _ => {}
                }
                return Some(UiAction::None);
            }

            // Workspaces clicks
            let ws_start_y = start_nav_y + item_h * 5.0 + 36.0 * ui_scale;
            if cy >= ws_start_y && cy <= ws_start_y + 80.0 * ui_scale {
                let ws_idx = ((cy - ws_start_y) / (24.0 * ui_scale)).floor() as usize;
                if ws_idx < app_state.workspaces.len() {
                    let target_ws = app_state.workspaces[ws_idx].clone();
                    app_state.select_workspace(&target_ws);
                }
                return Some(UiAction::None);
            }

            return Some(UiAction::None);
        }

        // 4. Right Inspector Panel clicks
        if app_state.show_right_panel && cx >= w - inspector_w && cy < h - bottom_status_h {
            let rx = w - inspector_w;
            let header_h = 44.0 * ui_scale;
            let tabs_y = top_h + header_h + 8.0 * ui_scale;
            let tabs_h = 26.0 * ui_scale;

            // Inspector Tabs Row: Overview │ Contract │ Code │ Runtime │ Logs
            if cy >= tabs_y && cy <= tabs_y + tabs_h {
                let tab_w = (inspector_w - 20.0 * ui_scale) / 5.0;
                let clicked_tab = ((cx - (rx + 10.0 * ui_scale)) / tab_w).floor() as usize;
                match clicked_tab {
                    0 => app_state.set_right_panel_tab(RightPanelTab::Overview),
                    1 => app_state.set_right_panel_tab(RightPanelTab::Contract),
                    2 => {
                        app_state.set_right_panel_tab(RightPanelTab::Code);
                        app_state.set_sidebar_tab(SidebarTab::AstView);
                    }
                    3 => {
                        app_state.set_right_panel_tab(RightPanelTab::Runtime);
                        if let Some(ref nid) = app_state.active_node_id.clone() {
                            app_state.execute_node_test(nid, "{}");
                        }
                    }
                    4 => app_state.set_right_panel_tab(RightPanelTab::Logs),
                    _ => {}
                }
                return Some(UiAction::None);
            }

            // Click on Source link button at bottom of Contract tab
            if app_state.right_panel_tab == RightPanelTab::Contract
                && cy >= h - bottom_status_h - 60.0 * ui_scale
            {
                if let Some(ref nid) = app_state.active_node_id.clone() {
                    app_state.open_code_editor_for_node(nid);
                } else {
                    app_state.set_sidebar_tab(SidebarTab::AstView);
                }
                return Some(UiAction::None);
            }

            // Mode B clicks (Zoom slider and Layout algorithm buttons when no node selected)
            if app_state.active_node_id.is_none() {
                let card_w = inspector_w - 24.0 * ui_scale;
                let card_x = rx + 12.0 * ui_scale;

                // 1. Zoom Level slider click/drag:
                let zoom_slider_y = top_h + 276.0 * ui_scale;
                if cy >= zoom_slider_y
                    && cy <= zoom_slider_y + 36.0 * ui_scale
                    && cx >= card_x
                    && cx <= card_x + card_w
                {
                    let track_w = card_w - 50.0 * ui_scale;
                    let ratio = ((cx - card_x) / track_w).clamp(0.0, 1.0) as f32;
                    let new_scale = (0.2 + ratio * (2.5 - 0.2)).clamp(0.2, 2.5);
                    let (screen_cx, screen_cy) = (
                        (w - inspector_w + sidebar_w) / 2.0,
                        (h + top_h - bottom_status_h) / 2.0,
                    );
                    let factor = new_scale / app_state.transform.scale;
                    app_state
                        .transform
                        .zoom_at(factor, screen_cx as f32, screen_cy as f32);
                    return Some(UiAction::None);
                }

                // 2. Layout algorithm buttons:
                let algo_y = top_h + 338.0 * ui_scale;
                let btn_h = 26.0 * ui_scale;
                if cy >= algo_y
                    && cy <= algo_y + btn_h * 3.0 + 20.0 * ui_scale
                    && cx >= card_x
                    && cx <= card_x + card_w
                {
                    let a_idx = ((cy - algo_y) / (btn_h + 8.0 * ui_scale)).floor() as usize;
                    match a_idx {
                        0 => app_state.set_layout_algorithm(LayoutAlgorithm::Hierarchical),
                        1 => app_state.set_layout_algorithm(LayoutAlgorithm::ForceDirected),
                        2 => app_state.set_layout_algorithm(LayoutAlgorithm::Grid),
                        _ => {}
                    }
                    return Some(UiAction::None);
                }
            }

            return Some(UiAction::None);
        }

        // 5. AST Split View clicks
        if app_state.active_sidebar_tab == SidebarTab::AstView {
            let tab_bar_h = 32.0 * ui_scale;
            let active_tab_w = 170.0 * ui_scale;
            // Close Tab click: [✕]
            if cy >= top_h
                && cy <= top_h + tab_bar_h
                && cx >= sidebar_w + active_tab_w - 28.0 * ui_scale
                && cx <= sidebar_w + active_tab_w
            {
                app_state.set_sidebar_tab(SidebarTab::Diagrams);
                return Some(UiAction::None);
            }

            // Click inside AST Tree list
            let center_w = w - sidebar_w - inspector_w;
            let left_w = center_w * 0.52;
            let rx = sidebar_w + left_w;
            let tree_start_y = top_h + tab_bar_h + 18.0 * ui_scale;
            let item_step = 24.0 * ui_scale;
            if cx >= rx
                && cx <= w - inspector_w
                && cy >= tree_start_y
                && cy <= tree_start_y + item_step * 8.0
            {
                let item_idx = ((cy - tree_start_y) / item_step).floor() as usize;
                match item_idx {
                    4 => {
                        app_state.selected_ast_symbol = Some("AuthRequest".to_string());
                        app_state.status_message =
                            "AST Symbol: struct AuthRequest (fields: username, password)"
                                .to_string();
                    }
                    5 => {
                        app_state.selected_ast_symbol = Some("AuthResponse".to_string());
                        app_state.status_message =
                            "AST Symbol: struct AuthResponse (fields: token, expires_in)"
                                .to_string();
                    }
                    6 => {
                        app_state.selected_ast_symbol = Some("verify_token".to_string());
                        app_state.status_message =
                            "AST Symbol: fn verify_token(&AuthRequest) -> AuthResponse".to_string();
                    }
                    7 => {
                        app_state.selected_ast_symbol = Some("test_token_valid".to_string());
                        app_state.status_message =
                            "AST Test: #[test] fn test_token_valid()".to_string();
                    }
                    _ => {}
                }
            }

            return Some(UiAction::None);
        }

        // 6. Floating Canvas Toolbar clicks: [ ↖ ✋ 🔍 ⛶ ⤢ ]
        let toolbar_w = 160.0 * ui_scale;
        let toolbar_h = 32.0 * ui_scale;
        let toolbar_x = sidebar_w + 20.0 * ui_scale;
        let toolbar_y = h - bottom_status_h - toolbar_h - 16.0 * ui_scale;

        if cx >= toolbar_x
            && cx <= toolbar_x + toolbar_w
            && cy >= toolbar_y
            && cy <= toolbar_y + toolbar_h
        {
            let tool_w = toolbar_w / 5.0;
            let tool_idx = ((cx - toolbar_x) / tool_w).floor() as usize;
            match tool_idx {
                0 => app_state.set_active_tool(CanvasTool::Pointer),
                1 => app_state.set_active_tool(CanvasTool::Pan),
                2 => app_state.set_active_tool(CanvasTool::Zoom),
                3 => {
                    app_state.set_active_tool(CanvasTool::Fit);
                    if let Some(ref diag) = app_state.current_diagram {
                        app_state.transform.fit_to_viewport(
                            diag.width,
                            diag.height,
                            w as f32,
                            h as f32,
                        );
                        app_state.transform.pan_x += sidebar_w as f32;
                        app_state.transform.pan_y += top_h as f32;
                    }
                }
                4 => app_state.set_active_tool(CanvasTool::Fullscreen),
                _ => {}
            }
            return Some(UiAction::None);
        }

        // 7. Floating Minimap clicks (click-to-pan)
        let minimap_w = 150.0 * ui_scale;
        let minimap_h = 98.0 * ui_scale;
        let minimap_x = w - inspector_w - minimap_w - 20.0 * ui_scale;
        let minimap_y = h - bottom_status_h - minimap_h - 16.0 * ui_scale;

        if cx >= minimap_x
            && cx <= minimap_x + minimap_w
            && cy >= minimap_y
            && cy <= minimap_y + minimap_h
        {
            if let Some(ref diag) = app_state.current_diagram {
                let rel_x = ((cx - minimap_x) / minimap_w).clamp(0.0, 1.0) as f32;
                let rel_y = ((cy - minimap_y) / minimap_h).clamp(0.0, 1.0) as f32;
                let target_world_x = diag.width * rel_x;
                let target_world_y = diag.height * rel_y;
                let center_w = w - sidebar_w - inspector_w;
                let center_h = h - top_h - bottom_status_h;
                let screen_cx = (sidebar_w + center_w / 2.0) as f32;
                let screen_cy = (top_h + center_h / 2.0) as f32;
                app_state.transform.pan_x = screen_cx - target_world_x * app_state.transform.scale;
                app_state.transform.pan_y = screen_cy - target_world_y * app_state.transform.scale;
            }
            return Some(UiAction::None);
        }

        // 7. Floating Action Pill below selected node: [ + 日 ❐ 🗑 ]
        let selected_node_pill = if let Some(ref sel_id) = app_state.active_node_id {
            if let Some(ref diag) = app_state.current_diagram {
                if let Some(node) = diag.nodes.iter().find(|n| &n.id == sel_id) {
                    let (sx, sy) = app_state.transform.world_to_screen(node.x, node.y);
                    let sw = node.width * app_state.transform.scale;
                    let sh = node.height * app_state.transform.scale;

                    let pill_w = 110.0 * app_state.transform.scale;
                    let pill_h = 24.0 * app_state.transform.scale;
                    let pill_x = sx + (sw - pill_w) / 2.0;
                    let pill_y = sy + sh + 8.0 * app_state.transform.scale;

                    if cx >= pill_x as f64
                        && cx <= (pill_x + pill_w) as f64
                        && cy >= pill_y as f64
                        && cy <= (pill_y + pill_h) as f64
                    {
                        let btn_w = pill_w / 4.0;
                        let btn_idx = ((cx - pill_x as f64) / btn_w as f64).floor() as usize;
                        Some((sel_id.clone(), btn_idx))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some((sel_id, btn_idx)) = selected_node_pill {
            match btn_idx {
                0 => {
                    // '+' -> connect
                    app_state.modal.mode = UiMode::Command;
                    app_state.modal.command_buffer = format!(":connect {} ", sel_id);
                }
                1 => {
                    // '日' -> node editor
                    app_state.open_node_editor(Some(&sel_id));
                }
                2 => {
                    // '❐' -> clone
                    app_state.add_node(merm_core::NodeKind::Class, &format!("{}_copy", sel_id));
                }
                3 => {
                    // '🗑' -> delete
                    app_state.remove_node(&sel_id);
                }
                _ => {}
            }
            return Some(UiAction::None);
        }

        // Not an overlay hit: click passed to canvas
        None
    }
}
