<div align="center">

# `merm`

### Blazingly Fast Native Linux Architecture Studio & Mermaid Diagram Viewer with Vim Modal Navigation, Project-Code Binding & Executable Nodes

[![CI](https://github.com/kadiryildiz/merm/actions/workflows/ci.yml/badge.svg)](https://github.com/kadiryildiz/merm/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust: 1.80+](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Platform: Linux](https://img.shields.io/badge/Platform-Wayland%20%7C%20X11-purple.svg)](#-installation)
[![Themes: 8 Themes + Terminal Transparent](https://img.shields.io/badge/Themes-8%20Themes%20%2B%20Terminal%20Transparent-pink.svg)](#-designer-themes)

<br/>

```text
       ┌────────────────────────┐
       │      merm (v0.1)       │◀─── High-Performance Native Rust Studio
       └───┬──────────────┬─────┘
           │              │
           ▼              ▼
     ┌───────────┐  ┌───────────┐
     │  Diagram  │  │  Project  │
     │  Mermaid  │◀─┼▶   Rust    │
     │  Canvas   │  │   AST     │
     └───────────┘  └───────────┘
           │              │
           ▼              ▼
    [ WGPU 120FPS ]  [ Executable Nodes ]
    Vector Studio    Input ➔ Run ➔ Output
```

</div>

---

## ⚡ Why `merm`?

Most existing Mermaid diagram tools rely on heavy web stacks: WebKitGTK wrappers, headless Chromium, or Electron apps that take seconds to launch, consume hundreds of megabytes of RAM, and don't integrate with actual codebases.

**`merm`** is built from the ground up in safe, modern Rust for developers who live in terminal editors (Neovim, Helix, Kakoune) and want a **living, executable architecture canvas**:

- 🚀 **Instant Launch:** Renders and opens in **<50ms** (`merm arch.md`, `merm .`, or `cat doc.md | merm -`).
- 💎 **Zero WebKit / Electron:** 100% native Rust binary using `winit`, `resvg`, `softbuffer`, and `wgpu`.
- 🎮 **120 FPS Fluid Interactivity:** Smooth GPU-accelerated canvas panning, zooming, and **interactive node drag-and-drop**.
- 🔗 **Project-to-Diagram Binding (`&set`):** Binds your Mermaid diagram directly to a Rust project root (`<root>/.merm/manifest.json`). Every Rust file/module maps to an architecture class node!
- ⚡ **Executable Diagram Nodes (`t` / `:test`):** Every class node in the diagram is executable! Enter input into the node harness drawer, press `Enter`, and observe real-time output, exit codes, execution duration, and stdout/stderr!
- 🧪 **Deterministic & LLM Architecture Verification (`&check`):** Verifies full compatibility between your Rust codebase and diagram symbols via `syn` AST analysis and LLM validation.
- 💡 **AI Architecture Advisor & Safe Mutations (`&advice` & `&ok`):** Request architectural recommendations (`&advice`). When you approve with `&ok`, `merm` creates atomic rollback snapshots in `.merm/snapshots/`, applies code/diagram changes, and verifies them with `cargo check`—rolling back automatically on error!
- 🤖 **Autonomous Multi-File Refactoring (`&ai`):** Run full feature additions or refactorings with synchronized diagram and code updates.
- 📐 **Deep UML Class & Struct Diagram Support:** Full 3-compartment UML cards with syntax-highlighted visibility tokens (`+`, `-`, `#`, `~`), types, variables, methods, comments, and stereotypes.
- ⌨️ **Vim Modal Navigation & Command Bar:** Muscle-memory navigation with `hjkl`, `Tab` / `p` direction pivoting, `0` fit-to-view, `:` and `&` interactive command bar, and `a`/`o`/`c`/`e` node authoring.
- 🪟 **Terminal-First Transparent Mode:** Canvas transparency with Wayland alpha compositing (`with_transparent(true)`). No unwanted background is forced—your terminal's background, opacity (e.g. Ghostty `0.90`), blur, or desktop shows directly behind the diagram!
- 🎨 **8 Designer Themes:** Catppuccin Mocha, Tokyo Night, Nord, Gruvbox Dark, Dracula, Monokai, Monokai Terminal, and Catppuccin Latte.
- 🔌 **Bidirectional Editor IPC:** Real-time sync with Neovim (`merm.nvim`) via secure Unix Domain Sockets authenticated by the Linux kernel (`SO_PEERCRED`).

---

## 🛠️ Interactive Command Protocol (`&` / `:` Prefix)

Open the command bar anytime by pressing `:` or `&` in the GUI:

| Command | Description |
| :--- | :--- |
| `&set [PATH]` | Binds the current Mermaid diagram to a target Rust project root (creates/loads `.merm/manifest.json` and maps symbols). |
| `&check` | Tests compatibility between the project and diagram using `syn` AST inspection, `cargo check`, and LLM critique. Displays a full diagnostic report modal. |
| `&advice <QUERY>` | Asks the LLM for architectural advice or refactoring strategy (read-only proposal preview). |
| `&ok` | Applies the pending recommendation from `&advice`. Creates an atomic rollback backup in `.merm/snapshots/`, applies mutations, and verifies build. |
| `&ai <PROMPT>` | Autonomous multi-file code generation and diagram update with automatic build verification and rollback protection. |
| `:add <class\|struct\|enum> <Name>` | Adds a new node to the diagram and automatically scaffolds the corresponding Rust module (`src/<name>.rs`) with executable entrypoints. |
| `:connect <From> <To> [label]` | Adds a dependency arrow/relation between two nodes in the diagram. |
| `:test [Node] [Input]` | Opens the interactive Node Test drawer for the target or currently selected node. |
| `:help` | Opens the in-app interactive command and keybinding reference. |

---

## 🚀 Executable Class Nodes (Input ➔ Run ➔ Output)

When a Rust project is bound to `merm`:
1. Every Rust source file or module corresponds to an architecture class in the diagram.
2. Select any node in the diagram and press **`t`** (or type `:test`).
3. An interactive vector test drawer slides up from the bottom of the window:
   - **Target:** `<NodeName> (src/module.rs)`
   - **Input:** Type or paste JSON, string, or test parameters.
   - **Run:** Press `Enter` to run the node through `merm`'s test harness.
   - **Output:** Live display of execution status (`✔ PASS` / `✖ FAIL`), duration (e.g. `12ms`), output payload, and full stdout/stderr!

---

## ⌨️ Modal Controls & Keybindings

| Key | Mode | Action |
| :--- | :--- | :--- |
| `:` or `&` | Normal | Open interactive command bar (`&set`, `&check`, `&advice`, `&ok`, `&ai`, etc.) |
| `t` | Normal (Node selected) | Open Node Test drawer for selected class |
| `i` or `K` | Normal (Node selected) | Open Node Inspector modal (type, file binding, fields, methods, relations, test status) |
| `T` | Normal | Cycle color themes (Mocha → Tokyo Night → Nord → Gruvbox → Dracula → Monokai → Terminal → Latte) |
| `a` or `o` | Normal | Quick add new class or struct node |
| `c` | Normal | Quick connect nodes |
| `e` | Normal (Node selected) | Open bound Rust source file in `$EDITOR` (Neovim, Helix, VSCode) |
| `h`, `j`, `k`, `l` | Normal | Pan canvas left, down, up, right |
| `+` / `-` | Normal | Zoom in / Zoom out |
| `0` | Normal | Fit diagram to viewport |
| `Tab` or `p` | Normal | Pivot layout direction (`TD` ↔ `LR` ↔ `RL` ↔ `BT`) |
| `n` / `N` | Normal | Select and focus next / previous node |
| `/` or `f` | Normal | Enter Search / Jump mode |
| **Left Click + Drag** | Canvas | Pan canvas |
| **Left Click on Node** | Node | **Drag & drop node** (connected relationship arrows dynamically bend!) |
| **Mouse Wheel** | Canvas | Zoom in / out centered at cursor position |
| `q` or `Esc` | Any | Close modal drawer / Quit application |

---

## 🎨 Designer Themes

Switch themes on-the-fly using `T` in the GUI or start with `--theme <NAME>`:

| Theme Name | CLI Identifier | Description |
| :--- | :--- | :--- |
| **Monokai Terminal** | `terminal`, `term`, `monokai-terminal`, `transparent` | **Terminal-native transparent canvas.** Imposes no canvas background—inherits your terminal's background, opacity (e.g. Ghostty `0.90`), blur, or desktop wallpaper. Cards feature high-contrast translucent fills (`#181816f0`) and vibrant Monokai syntax highlighting. |
| **Monokai** | `monokai`, `monokai-remastered` | Classic Monokai Remastered dark theme with deep `#0c0c0c` background and crisp neon syntax. |
| **Catppuccin Mocha** | `catppuccin`, `catppuccin-mocha`, `mocha` | Soothing developer dark theme (Default). |
| **Tokyo Night** | `tokyo-night`, `tokyo` | Vibrant Japanese neon aesthetics with deep blues and purples. |
| **Nord** | `nord` | Arctic, north-bluish clean and calm palette. |
| **Gruvbox Dark** | `gruvbox`, `gruvbox-dark` | Warm retro groove contrast for warm-palette lovers. |
| **Dracula** | `dracula` | Gothic high-contrast dark theme with pink and green accents. |
| **Catppuccin Latte** | `latte`, `catppuccin-latte`, `light` | High-legibility light theme for bright environments. |

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

### Opening Files, Directories & Piping Stdin
```bash
# Open with default showcase diagram
merm

# Bind directly to a Rust project directory
merm /path/to/rust/project
merm .

# Open with Terminal Transparent mode (inherits Ghostty/terminal opacity & blur)
merm -t terminal diagram.md

# Open with Monokai Remastered
merm -t monokai diagram.md

# Pipe from stdin (ideal for fzf, git diff, or cat)
cat class_diagram.mmd | merm -

# Headless syntax check & batch verification
merm --headless path/to/diagram.md
```

### FreeDesktop App Launcher
`merm` includes a FreeDesktop-compliant desktop file (`packaging/merm.desktop`) and vector icon (`packaging/merm.svg`), allowing it to appear directly in your application launcher (Rofi, Wofi, GNOME, KDE, Hyprland, Sway).

```bash
install -Dm644 packaging/merm.desktop ~/.local/share/applications/merm.desktop
install -Dm644 packaging/merm.svg ~/.local/share/icons/hicolor/scalable/apps/merm.svg
```

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
      theme = "terminal", -- Use terminal transparent mode
    })

    -- Keybindings
    vim.keymap.set("n", "<leader>mv", merm.open, { desc = "Open in Merm" })
    vim.keymap.set("n", "<leader>mp", merm.pivot_direction, { desc = "Pivot Diagram Direction" })
  end,
}
```

---

## 🏗️ Workspace Architecture

```text
merm/
├── Cargo.toml               # Workspace manifest with dual MIT/Apache-2.0 licenses
├── crates/
│   ├── merm-core/           # AST parser, syn Rust scanner, manifest, node runner, advisor, transactions
│   ├── merm-render/         # WGPU pipeline, SIMD SvgRasterizer, alpha compositing overlay
│   ├── merm-ui/             # winit 0.30, softbuffer, modal controller, vector overlay widgets
│   ├── merm-ipc/            # SO_PEERCRED authenticated Unix socket server
│   └── merm-cli/            # Binary entry point, CLI arguments, and config loader
├── editors/
│   └── merm.nvim/           # Neovim plugin for live editor sync
├── examples/
│   └── class_diagram.md     # Rich UML showcase diagram
└── packaging/
    ├── merm.desktop         # FreeDesktop app launcher entry
    ├── merm.svg             # Application vector icon
    └── PKGBUILD             # Arch / CachyOS package script
```

---

## 🤝 Contributing & Standards

All code is 100% safe Rust, formatted with `cargo fmt`, and passes strict linter verification:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

---

## 📄 License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.
