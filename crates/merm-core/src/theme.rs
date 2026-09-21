use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ThemeId {
    #[default]
    StudioDark,
    Monokai,
    MonokaiTerminal,
    CatppuccinMocha,
    TokyoNight,
    Nord,
    GruvboxDark,
    Dracula,
    CatppuccinLatte,
}

impl ThemeId {
    pub fn next(&self) -> Self {
        match self {
            ThemeId::StudioDark => ThemeId::Monokai,
            ThemeId::Monokai => ThemeId::MonokaiTerminal,
            ThemeId::MonokaiTerminal => ThemeId::CatppuccinMocha,
            ThemeId::CatppuccinMocha => ThemeId::TokyoNight,
            ThemeId::TokyoNight => ThemeId::Nord,
            ThemeId::Nord => ThemeId::GruvboxDark,
            ThemeId::GruvboxDark => ThemeId::Dracula,
            ThemeId::Dracula => ThemeId::CatppuccinLatte,
            ThemeId::CatppuccinLatte => ThemeId::StudioDark,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().replace(['-', '_'], "").as_str() {
            "studio" | "studiodark" | "linear" | "dark" | "merm" => Some(ThemeId::StudioDark),
            "catppuccin" | "catppuccinmocha" | "mocha" => Some(ThemeId::CatppuccinMocha),
            "tokyonight" | "tokyo" => Some(ThemeId::TokyoNight),
            "nord" => Some(ThemeId::Nord),
            "gruvbox" | "gruvboxdark" => Some(ThemeId::GruvboxDark),
            "dracula" => Some(ThemeId::Dracula),
            "monokai" | "monokairemastered" => Some(ThemeId::Monokai),
            "monokaiterminal" | "terminal" | "term" | "transparent" | "monokaitransparent" => {
                Some(ThemeId::MonokaiTerminal)
            }
            "latte" | "catppuccinlatte" | "light" => Some(ThemeId::CatppuccinLatte),
            _ => None,
        }
    }

    pub fn palette(&self) -> ColorPalette {
        match self {
            ThemeId::StudioDark => ColorPalette {
                name: "Merm Studio Dark".to_string(),
                background: "#0e1117".to_string(),
                card_bg: "#161b26".to_string(),
                card_header: "#1f2430".to_string(),
                border: "#283141".to_string(),
                divider: "#262f3d".to_string(),
                text_main: "#f0f6fc".to_string(),
                text_sub: "#8b949e".to_string(),
                text_muted: "#484f58".to_string(),
                text_accent: "#f0883e".to_string(),
                public_vis: "#3fb950".to_string(),
                private_vis: "#f85149".to_string(),
                protected_vis: "#d29922".to_string(),
                package_vis: "#bc8cff".to_string(),
                edge_stroke: "#388bfd".to_string(),
                badge_bg: "#11151e".to_string(),
                var_color: "#79c0ff".to_string(),
                type_color: "#39c5cf".to_string(),
                method_color: "#58a6ff".to_string(),
                comment_color: "#8b949e".to_string(),
                stereotype_color: "#bc8cff".to_string(),
            },
            ThemeId::CatppuccinMocha => ColorPalette {
                name: "Catppuccin Mocha".to_string(),
                background: "#1e1e2e".to_string(),
                card_bg: "#313244".to_string(),
                card_header: "#45475a".to_string(),
                border: "#89b4fa".to_string(),
                divider: "#585b70".to_string(),
                text_main: "#cdd6f4".to_string(),
                text_sub: "#bac2de".to_string(),
                text_muted: "#6c7086".to_string(),
                text_accent: "#f9e2af".to_string(),
                public_vis: "#a6e3a1".to_string(),
                private_vis: "#f38ba8".to_string(),
                protected_vis: "#fab387".to_string(),
                package_vis: "#cba6f7".to_string(),
                edge_stroke: "#89b4fa".to_string(),
                badge_bg: "#181825".to_string(),
                var_color: "#cdd6f4".to_string(),
                type_color: "#f9e2af".to_string(),
                method_color: "#89b4fa".to_string(),
                comment_color: "#a6adc8".to_string(),
                stereotype_color: "#f5c2e7".to_string(),
            },
            ThemeId::TokyoNight => ColorPalette {
                name: "Tokyo Night".to_string(),
                background: "#1a1b26".to_string(),
                card_bg: "#24283b".to_string(),
                card_header: "#2f354f".to_string(),
                border: "#7aa2f7".to_string(),
                divider: "#414868".to_string(),
                text_main: "#c0caf5".to_string(),
                text_sub: "#a9b1d6".to_string(),
                text_muted: "#565f89".to_string(),
                text_accent: "#e0af68".to_string(),
                public_vis: "#9ece6a".to_string(),
                private_vis: "#f7768e".to_string(),
                protected_vis: "#ff9e64".to_string(),
                package_vis: "#bb9af7".to_string(),
                edge_stroke: "#7aa2f7".to_string(),
                badge_bg: "#16161e".to_string(),
                var_color: "#c0caf5".to_string(),
                type_color: "#7dcfff".to_string(),
                method_color: "#7aa2f7".to_string(),
                comment_color: "#737aa2".to_string(),
                stereotype_color: "#bb9af7".to_string(),
            },
            ThemeId::Nord => ColorPalette {
                name: "Nord".to_string(),
                background: "#2e3440".to_string(),
                card_bg: "#3b4252".to_string(),
                card_header: "#434c5e".to_string(),
                border: "#88c0d0".to_string(),
                divider: "#4c566a".to_string(),
                text_main: "#eceff4".to_string(),
                text_sub: "#e5e9f0".to_string(),
                text_muted: "#d8dee9".to_string(),
                text_accent: "#ebcb8b".to_string(),
                public_vis: "#a3be8c".to_string(),
                private_vis: "#bf616a".to_string(),
                protected_vis: "#d08770".to_string(),
                package_vis: "#b48ead".to_string(),
                edge_stroke: "#81a1c1".to_string(),
                badge_bg: "#242933".to_string(),
                var_color: "#eceff4".to_string(),
                type_color: "#ebcb8b".to_string(),
                method_color: "#88c0d0".to_string(),
                comment_color: "#7b88a1".to_string(),
                stereotype_color: "#b48ead".to_string(),
            },
            ThemeId::GruvboxDark => ColorPalette {
                name: "Gruvbox Dark".to_string(),
                background: "#282828".to_string(),
                card_bg: "#3c3836".to_string(),
                card_header: "#504945".to_string(),
                border: "#83a598".to_string(),
                divider: "#665c54".to_string(),
                text_main: "#ebdbb2".to_string(),
                text_sub: "#d5c4a1".to_string(),
                text_muted: "#928374".to_string(),
                text_accent: "#fabd2f".to_string(),
                public_vis: "#b8bb26".to_string(),
                private_vis: "#fb4934".to_string(),
                protected_vis: "#fe8019".to_string(),
                package_vis: "#d3869b".to_string(),
                edge_stroke: "#83a598".to_string(),
                badge_bg: "#1d2021".to_string(),
                var_color: "#ebdbb2".to_string(),
                type_color: "#fabd2f".to_string(),
                method_color: "#b8bb26".to_string(),
                comment_color: "#928374".to_string(),
                stereotype_color: "#d3869b".to_string(),
            },
            ThemeId::Dracula => ColorPalette {
                name: "Dracula".to_string(),
                background: "#282a36".to_string(),
                card_bg: "#44475a".to_string(),
                card_header: "#6272a4".to_string(),
                border: "#bd93f9".to_string(),
                divider: "#6272a4".to_string(),
                text_main: "#f8f8f2".to_string(),
                text_sub: "#e2e2dc".to_string(),
                text_muted: "#6272a4".to_string(),
                text_accent: "#f1fa8c".to_string(),
                public_vis: "#50fa7b".to_string(),
                private_vis: "#ff5555".to_string(),
                protected_vis: "#ffb86c".to_string(),
                package_vis: "#ff79c6".to_string(),
                edge_stroke: "#bd93f9".to_string(),
                badge_bg: "#21222c".to_string(),
                var_color: "#f8f8f2".to_string(),
                type_color: "#8be9fd".to_string(),
                method_color: "#50fa7b".to_string(),
                comment_color: "#6272a4".to_string(),
                stereotype_color: "#ff79c6".to_string(),
            },
            ThemeId::Monokai => ColorPalette {
                name: "Monokai".to_string(),
                background: "#0c0c0c".to_string(),
                card_bg: "#1a1a1a".to_string(),
                card_header: "#272822".to_string(),
                border: "#58d1eb".to_string(),
                divider: "#3e3d32".to_string(),
                text_main: "#f6f6ef".to_string(),
                text_sub: "#c4c5b5".to_string(),
                text_muted: "#75715e".to_string(),
                text_accent: "#e0d561".to_string(),
                public_vis: "#98e024".to_string(),
                private_vis: "#f4005f".to_string(),
                protected_vis: "#fd971f".to_string(),
                package_vis: "#9d65ff".to_string(),
                edge_stroke: "#58d1eb".to_string(),
                badge_bg: "#141414".to_string(),
                var_color: "#f6f6ef".to_string(),
                type_color: "#58d1eb".to_string(),
                method_color: "#98e024".to_string(),
                comment_color: "#75715e".to_string(),
                stereotype_color: "#f4005f".to_string(),
            },
            ThemeId::MonokaiTerminal => ColorPalette {
                name: "Monokai Terminal".to_string(),
                background: "transparent".to_string(),
                card_bg: "#181816f0".to_string(),
                card_header: "#242420".to_string(),
                border: "#58d1eb".to_string(),
                divider: "#383834".to_string(),
                text_main: "#f6f6ef".to_string(),
                text_sub: "#c4c5b5".to_string(),
                text_muted: "#75715e".to_string(),
                text_accent: "#e0d561".to_string(),
                public_vis: "#98e024".to_string(),
                private_vis: "#f4005f".to_string(),
                protected_vis: "#fd971f".to_string(),
                package_vis: "#9d65ff".to_string(),
                edge_stroke: "#58d1eb".to_string(),
                badge_bg: "#141414".to_string(),
                var_color: "#f6f6ef".to_string(),
                type_color: "#58d1eb".to_string(),
                method_color: "#98e024".to_string(),
                comment_color: "#75715e".to_string(),
                stereotype_color: "#f4005f".to_string(),
            },
            ThemeId::CatppuccinLatte => ColorPalette {
                name: "Catppuccin Latte".to_string(),
                background: "#eff1f5".to_string(),
                card_bg: "#e6e9ef".to_string(),
                card_header: "#dce0e8".to_string(),
                border: "#1e66f5".to_string(),
                divider: "#bcc0cc".to_string(),
                text_main: "#4c4f69".to_string(),
                text_sub: "#5c5f77".to_string(),
                text_muted: "#9ca0b0".to_string(),
                text_accent: "#df8e1d".to_string(),
                public_vis: "#40a02b".to_string(),
                private_vis: "#d20f39".to_string(),
                protected_vis: "#fe640b".to_string(),
                package_vis: "#8839ef".to_string(),
                edge_stroke: "#1e66f5".to_string(),
                badge_bg: "#ccd0da".to_string(),
                var_color: "#4c4f69".to_string(),
                type_color: "#df8e1d".to_string(),
                method_color: "#1e66f5".to_string(),
                comment_color: "#8c8fa1".to_string(),
                stereotype_color: "#ea76cb".to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPalette {
    pub name: String,
    pub background: String,
    pub card_bg: String,
    pub card_header: String,
    pub border: String,
    pub divider: String,
    pub text_main: String,
    pub text_sub: String,
    pub text_muted: String,
    pub text_accent: String,
    pub public_vis: String,
    pub private_vis: String,
    pub protected_vis: String,
    pub package_vis: String,
    pub edge_stroke: String,
    pub badge_bg: String,

    pub var_color: String,
    pub type_color: String,
    pub method_color: String,
    pub comment_color: String,
    pub stereotype_color: String,
}

impl Default for ColorPalette {
    fn default() -> Self {
        ThemeId::Monokai.palette()
    }
}

impl ColorPalette {
    pub fn surface_base(&self) -> &str {
        &self.card_bg
    }

    pub fn surface_elevated(&self) -> &str {
        &self.card_header
    }

    pub fn surface_selected(&self) -> &str {
        &self.badge_bg
    }

    pub fn border_subtle(&self) -> &str {
        &self.divider
    }

    pub fn border_focused(&self) -> &str {
        &self.border
    }

    pub fn text_primary(&self) -> &str {
        &self.text_main
    }

    pub fn text_secondary(&self) -> &str {
        &self.text_sub
    }

    pub fn accent_primary(&self) -> &str {
        &self.method_color
    }

    pub fn accent_secondary(&self) -> &str {
        &self.stereotype_color
    }

    pub fn status_ok(&self) -> &str {
        &self.public_vis
    }

    pub fn status_warn(&self) -> &str {
        &self.protected_vis
    }

    pub fn status_err(&self) -> &str {
        &self.private_vis
    }

    pub fn edge_idle(&self) -> &str {
        &self.divider
    }

    pub fn edge_active(&self) -> &str {
        &self.edge_stroke
    }

    pub fn edge_divergent(&self) -> &str {
        &self.private_vis
    }

    pub fn role_color(&self, role: &str) -> &'static str {
        match role.to_lowercase().trim_matches(['<', '>', ' ', '"', '\'']) {
            "actor" | "user" => "#58a6ff",
            "app" | "frontend" | "mobile" | "client" => "#bc8cff",
            "service" | "backend" | "api" | "microservice" => "#3fb950",
            "auth" | "security" => "#d29922",
            "database" | "db" | "store" | "sql" | "postgres" | "postgresql" => "#39c5cf",
            "cache" | "redis" | "memory" => "#ff7b72",
            "queue" | "message" | "kafka" | "broker" | "mq" => "#79c0ff",
            _ => "#58a6ff",
        }
    }

    pub fn role_icon_symbol(&self, role: &str) -> &'static str {
        match role.to_lowercase().trim_matches(['<', '>', ' ', '"', '\'']) {
            "actor" | "user" => "👤",
            "app" | "frontend" | "mobile" => "💻",
            "service" | "backend" | "api" => "⚙",
            "auth" | "security" => "🛡",
            "database" | "db" | "postgres" | "postgresql" => "🗄",
            "cache" | "redis" => "⚡",
            "queue" | "message" | "mq" => "📨",
            _ => "⬡",
        }
    }

    pub fn node_role_color(&self, role: &str, title: &str) -> &'static str {
        let t = title.to_lowercase();
        let r = role.to_lowercase();
        if t.contains("auth") {
            "#e5a93c"
        } else if t.contains("notification") {
            "#f43f5e"
        } else if t.contains("gateway") {
            "#3fb950"
        } else if t.contains("user service") || (t.contains("user") && r.contains("service")) {
            "#388bfd"
        } else if t.contains("mobile") || t.contains("web") || t.contains("frontend") {
            "#bc8cff"
        } else if t.contains("postgres") || r.contains("database") || r.contains("db") {
            "#39c5cf"
        } else if t.contains("redis") || r.contains("cache") {
            "#ff6b6b"
        } else if t.contains("queue") || r.contains("queue") || t.contains("kafka") {
            "#79c0ff"
        } else if t.contains("user") || r.contains("actor") {
            "#388bfd"
        } else {
            self.role_color(role)
        }
    }

    pub fn node_role_icon(&self, role: &str, title: &str) -> &'static str {
        let t = title.to_lowercase();
        let r = role.to_lowercase();
        if t.contains("auth") {
            "⚙"
        } else if t.contains("notification") {
            "🔔"
        } else if t.contains("gateway") {
            "⚙"
        } else if t.contains("user service") || (t.contains("user") && r.contains("service")) {
            "👤"
        } else if t.contains("mobile") {
            "📱"
        } else if t.contains("web") || t.contains("frontend") {
            "💻"
        } else if t.contains("postgres") || r.contains("database") || r.contains("db") {
            "🗄"
        } else if t.contains("redis") || r.contains("cache") {
            "⚡"
        } else if t.contains("queue") || r.contains("queue") || t.contains("kafka") {
            "⇄"
        } else if t.contains("user") || r.contains("actor") {
            "👤"
        } else {
            self.role_icon_symbol(role)
        }
    }
}
