<div align="center">

# `merm`

### Blazingly Fast Native Linux Desktop Viewer for Mermaid Diagrams with Vim Modal Navigation & Deep Class Diagram Support

[![CI](https://github.com/kadiryildiz/merm/actions/workflows/ci.yml/badge.svg)](https://github.com/kadiryildiz/merm/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust: 1.80+](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Platform: Linux](https://img.shields.io/badge/Platform-Wayland%20%7C%20X11-purple.svg)](#installation)
[![Themes: Catppuccin | Tokyo Night | Nord](https://img.shields.io/badge/Themes-Catppuccin%20%7C%20Tokyo%20Night%20%7C%20Nord-pink.svg)](#-designer-themes)

<br/>

```text
       ┌───────────────┐
       │   merm (v0.1) │◀─── Blazingly Fast Native Rust Binary
       └───┬───────────┘
           │
     ┌─────┴───────────────┐
     ▼                     ▼
[ WGPU 120FPS ]   [ Softbuffer SIMD ]
GPU Acceleration    Zero-Glitch CPU Fallback
```

</div>

---

## ⚡ Why `merm`?

Most existing Mermaid diagram tools rely on heavy web stacks: WebKitGTK wrappers, headless Chromium, or Electron apps that take seconds to launch, consume hundreds of megabytes of RAM, and don't integrate smoothly with modal terminal workflows.

**`merm`** is built from the ground up in safe, modern Rust for developers who live in terminal editors (Neovim, Helix, Kakoune):

- 🚀 **Instant Launch:** Renders and opens in **<50ms** (`merm arch.md` or `cat doc.md | merm -`).
- 💎 **Zero WebKit / Electron:** 100% native Rust binary using `winit`, `resvg`, and `softbuffer` / `wgpu`.
- 🎮 **120 FPS Fluid Interactivity:** Smooth GPU-accelerated canvas panning, zooming, and **interactive node drag-and-drop**.
- 📐 **Deep UML Class & Struct Diagram Support:** Full 3-compartment UML cards with syntax-highlighted visibility tokens (`+`, `-`, `#`, `~`), types, variables, methods, comments, and stereotypes.
- ⌨️ **Vim Modal Navigation:** Muscle-memory navigation with `hjkl`, `Tab` direction pivoting, `0` fit-to-view, and `/` node fuzzy jump.
- 🎨 **6 Designer Themes:** Catppuccin Mocha, Tokyo Night, Nord, Gruvbox Dark, Dracula, and Catppuccin Latte.
- 🔌 **Bidirectional Editor IPC:** Real-time sync with Neovim (`merm.nvim`) via secure Unix Domain Sockets authenticated by the Linux kernel (`SO_PEERCRED`).
- 💤 **0% Idle CPU:** Event-driven architecture with zero polling thrash.

---

## 🏛️ Class & Struct Diagram Typography

`merm` treats UML and Struct diagrams as first-class citizens with code-editor quality syntax highlighting:

```mermaid
classDiagram
    class OrderService {
        %% Core order processing service
        <<interface>>
        +UUID orderId // Unique order identifier
        -BigDecimal totalAmount
        #OrderStatus status
        +processOrder(cart) Result~Order~
        -validateInventory(items) bool // Check stock availability
    }
```

### Visual Breakdown:
- **Header:** Stereotype pill (`«interface»`, `«struct»`), bold centered class title, and italic class docstring (`// Core order processing service`).
- **Visibility Tokens:** `+` (Public: Green), `-` (Private: Red), `#` (Protected: Orange), `~` (Package: Purple).
- **Data Types:** Distinct, high-contrast syntax color (e.g. Yellow in Catppuccin, Cyan in Tokyo Night, Mint in Nord).
- **Variable Names:** Dedicated field color (`orderId`, `totalAmount`).
- **Method Signatures:** Dedicated function color (`processOrder(cart)`), return types, and parameter lists.
- **Comments (`//` or `%%`):** Clean, italicized muted comments that never collide with code.
- **UML Relationships:** Full support for Inheritance (`<|--`), Realization (`<|..`), Composition (`*--`), Aggregation (`o--`), Association (`-->`), and Dependency (`..>`).

---

## ⌨️ Modal Controls & Keybindings

| Key | Mode | Action |
| :--- | :--- | :--- |
| `h`, `j`, `k`, `l` | Normal | Pan canvas left, down, up, right |
| `+` / `-` | Normal | Zoom in / Zoom out |
| `0` | Normal | Fit diagram to viewport |
| `Tab` or `p` | Normal | Pivot layout direction (`TD` ↔ `LR` ↔ `RL` ↔ `BT`) |
| `c` | Normal | Cycle color themes |
| `n` / `N` | Normal | Select and focus next / previous node |
| `/` or `f` | Normal | Enter Search / Jump mode |
| **Left Click + Drag** | Canvas | Pan canvas |
| **Left Click on Node** | Node | **Drag & drop node** (connected arrows dynamically bend!) |
| **Mouse Wheel** | Canvas | Zoom in / out centered at cursor |
| `q` or `Esc` | Any | Quit application |

---

## 🎨 Designer Themes

Switch themes on-the-fly using the `c` key or start with `--theme <NAME>`:

1. **Catppuccin Mocha** (Default developer dark theme)
2. **Tokyo Night** (Vibrant Japanese neon aesthetics)
3. **Nord** (Arctic, north-bluish clean palette)
4. **Gruvbox Dark** (Warm, retro groove contrast)
5. **Dracula** (Gothic dark contrast)
6. **Catppuccin Latte** (High-legibility light theme)

---

## 📦 Installation

### Arch Linux / CachyOS (AUR / PKGBUILD)
```bash
cd packaging
makepkg -si
```

### From Source (Cargo)
```bash
git clone https://github.com/kadiryildiz/merm.git
cd merm
cargo build --release --locked
install -Dm755 target/release/merm ~/.local/bin/merm
```

---

## 🚀 Usage

### Opening Files & Piping Stdin
```bash
# Open a diagram file
merm diagram.md

# Open with a specific theme
merm -t tokyo-night architecture.md

# Pipe from stdin (ideal for fzf, git diff, or cat)
cat class_diagram.mmd | merm -

# Headless syntax check & batch verification
merm --headless path/to/diagram.md
```

### FreeDesktop App Launcher
`merm` includes a FreeDesktop-compliant desktop file (`packaging/merm.desktop`) and vector icon (`packaging/merm.svg`), allowing it to appear directly in your application launcher (Rofi, Wofi, GNOME, KDE, Hyprland).

---

## 🔌 Neovim Integration (`merm.nvim`)

`merm` ships with a native Lua plugin located in `editors/merm.nvim`:

```lua
-- lazy.nvim specification
{
  "kadiryildiz/merm",
  ft = { "markdown", "mermaid" },
  config = function()
    local merm = require("merm")
    merm.setup({
      auto_open = false,
      theme = "catppuccin-mocha",
    })

    -- Keybindings
    vim.keymap.set("n", "<leader>mv", merm.open, { desc = "Open in Merm" })
    vim.keymap.set("n", "<leader>mp", merm.pivot_direction, { desc = "Pivot Diagram Direction" })
  end,
}
```

---

## ⚙️ Configuration

`merm` reads user preferences from `~/.config/merm/config.toml`:

```toml
# merm configuration file
theme = "catppuccin-mocha"

# Default diagram direction: "TD", "LR", "RL", "BT"
direction = "TD"
```

---

## 🏗️ Workspace Architecture

```text
merm/
├── Cargo.toml               # Workspace manifest
├── crates/
│   ├── merm-core/           # AST parser, direction pivoting, UML engine
│   ├── merm-render/         # WGPU pipeline, SIMD SvgRasterizer, LOD cache
│   ├── merm-ui/             # winit 0.30, softbuffer, modal state machine
│   ├── merm-ipc/            # SO_PEERCRED authenticated Unix socket server
│   └── merm-cli/            # Binary entry point and config loader
├── editors/
│   └── merm.nvim/           # Neovim plugin for live editor sync
└── packaging/
    ├── merm.desktop         # FreeDesktop app launcher entry
    ├── merm.svg             # Application vector icon
    └── PKGBUILD             # Arch / CachyOS package script
```

---

## 🤝 Contributing

Contributions are welcome! Please review [CONTRIBUTING.md](CONTRIBUTING.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) before opening a pull request.

All submissions must pass:
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

---

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
