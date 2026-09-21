<div align="center">

# `merm`

### Blazingly Fast Native Linux Architecture Studio & Mermaid Diagram Viewer with Vim Modal Navigation, Project-Code Binding, Executable Nodes & Google Antigravity (AGY) Integration

[![CI](https://github.com/kadiryildiz283/merm/actions/workflows/ci.yml/badge.svg)](https://github.com/kadiryildiz283/merm/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust: 1.80+](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![Platform: Linux](https://img.shields.io/badge/Platform-Wayland%20%7C%20X11-purple.svg)](#-installation)
[![LLM: AGY + OpenAI](https://img.shields.io/badge/AI-Antigravity%20(AGY)%20%7C%20OpenAI%20%7C%20Ollama-teal.svg)](#-llm-engines--google-antigravity-agy-binding)
[![WGPU 120 FPS](https://img.shields.io/badge/Renderer-WGPU%20120%20FPS-green.svg)](#-why-merm)
[![Themes: 8 Themes + Terminal Transparent](https://img.shields.io/badge/Themes-8%20Themes%20%2B%20Terminal%20Transparent-pink.svg)](#-designer-themes)

<br/>

```text
       ┌────────────────────────┐
       │      merm (v0.1)       │◀─── High-Performance Native Rust Studio
       └───┬───────────┬────┬───┘
           │           │    │
           ▼           ▼    ▼
     ┌───────────┐   ┌────────────┐   ┌────────────────────────┐
     │  Diagram  │   │  Project   │   │     AI Co-Architect    │
     │  Mermaid  │◀─▶│    Rust    │◀─▶│  Google Antigravity    │
     │  Canvas   │   │    AST     │   │  OpenAI / Ollama       │
     └───────────┘   └────────────┘   └────────────────────────┘
           │               │
           ▼               ▼
    [ WGPU 120FPS ]  [ Executable Nodes ]
    Vector Studio    Input ➔ Run ➔ Output
```

</div>

---

## ⚡ Why `merm`?

Most existing Mermaid diagram tools rely on heavy web stacks: WebKitGTK wrappers, headless Chromium, or Electron apps that take seconds to launch, consume hundreds of megabytes of RAM, and don't integrate with actual codebases.

**`merm`** is built from the ground up in safe, modern Rust for developers who live in terminal editors (Neovim, Helix, Kakoune) and want a **living, executable architecture canvas**:

- 🚀 **Instant Launch:** Renders and opens in **<50ms** (`merm arch.md`, `merm .`, or `cat doc.md | merm -`).
- 💎 **Zero WebKit / Electron:** 100% native safe Rust binary using `winit`, `resvg`, `softbuffer`, and hardware-accelerated `wgpu`.
- 📁 **Standalone Diagram Mode & Clean Starter Templates:** Opening an empty file (`touch arch.md && merm arch.md`) loads an instant, clean starter template. Built-in filesystem boundary guards prevent accidental root scans of `$HOME` or non-project parent directories.
- 🤖 **Google Antigravity CLI (`agy`) Native Binding (`&agy`):** Deep integration with Google Antigravity CLI. Runs non-interactive architectural queries, intent reconstruction, and code reasoning directly from the in-app command bar.
- 🔑 **OpenAI & Custom LLM Endpoint Support (`:config`):** Native API key authentication for OpenAI (`gpt-4o`, `gpt-4o-mini`), custom OpenAI-compatible endpoints (vLLM, Ollama, OpenRouter), and interactive runtime configuration.
- 🎮 **120 FPS Fluid Interactivity:** Smooth GPU-accelerated canvas panning, zooming, and **interactive node drag-and-drop** with dynamic relation arrow recalculation.
- 🔗 **Project-to-Diagram Binding (`&set`):** Binds your Mermaid diagram directly to a Rust project root (`<root>/.merm/manifest.json`). Every Rust file/module maps to an architecture class node!
- ⚡ **Executable Diagram Nodes (`t` / `:test`):** Every node in the diagram is executable with **guaranteed default input (`"{}"`) and default output (`"1"` even if void `()`)**! Enter input into the node harness drawer or press `Enter`, and observe real-time output, exit codes, execution duration, and stdout/stderr.
- ✏️ **Interactive In-App Node & Diagram Editor (`E` / Click):** Click any node or text in the diagram (or press `E` or `:edit <Node>`) to open the floating modal editor. Add fields, methods, or stereotypes, and save instantly with live SVG recalculation.
- 📋 **Persistent Split Buffer with Native Clipboard (`[📋 Kopyala]` / `y` / `Ctrl+C`):** View AI advice and diagnostic reports in a bottom split buffer. Copy report text directly to your system clipboard (Wayland `wl-copy` and X11 supported).
- 🧭 **Intuitive Selection & Arrow Navigation:** If no node is currently selected, pressing any Arrow key automatically focuses the first node in the diagram with a glowing cyan selection ring (`🎯 SELECTED`).
- 🧪 **Deterministic & LLM Architecture Verification (`&check`):** Verifies full compatibility between your Rust codebase and diagram symbols via `syn` AST analysis and LLM validation.
- 💡 **AI Architecture Advisor & Safe Mutations (`&advice` & `&ok`):** Request architectural recommendations (`&advice`). When you approve with `&ok`, `merm` creates atomic rollback snapshots in `.merm/snapshots/`, applies code/diagram changes, and verifies them with `cargo check`—rolling back automatically on error!
- 🤖 **Autonomous Multi-File Refactoring (`&ai`):** Run full feature additions or refactorings with synchronized diagram and code updates.
- 📐 **Deep UML Class & Struct Diagram Support:** Full 3-compartment UML cards with syntax-highlighted visibility tokens (`+`, `-`, `#`, `~`), types, variables, methods, comments, and stereotypes (`<<struct>>`, `<<enum>>`, `<<module>>`).
- ⌨️ **Vim Modal Navigation & Smart Prompt Auto-Completion (`Tab`):** Muscle-memory navigation with `hjkl`, `Tab` / `p` direction pivoting, `0` / `:fit` readable diagram auto-fit, full-width `:` and `&` command line, and authentic Airline/Lualine statusline. Pressing `Tab` on `&advice`, `&ai`, `&agy`, or `:test` auto-fills context-aware prompts!
- 🪟 **Terminal-First Transparent Mode:** Canvas transparency with Wayland alpha compositing (`with_transparent(true)`). No unwanted background is forced—your terminal's background, opacity (e.g. Ghostty `0.90`), blur, or desktop shows directly behind the diagram!
- 🎨 **8 Designer Themes (Default: Monokai):** Monokai, Monokai Terminal (Transparent), Catppuccin Mocha, Tokyo Night, Nord, Gruvbox Dark, Dracula, and Catppuccin Latte.
- 🔌 **Bidirectional Editor IPC:** Real-time sync with Neovim (`merm.nvim`) via secure Unix Domain Sockets authenticated by the Linux kernel (`SO_PEERCRED`).

---

## 🛠️ Interactive Command Protocol (`:` / `&` Prefix)

Open the command line anytime by pressing `:` or `&` in Normal mode:

| Command | Type | Description |
| :--- | :---: | :--- |
| `&agy [PROMPT]` | AI | Invokes Google Antigravity CLI (`agy`) directly. Has intelligent default prompt or expands with `Tab`. |
| `&check` | AI / AST | Tests compatibility between the project and diagram using `syn` AST inspection, `cargo check`, and LLM critique. Opens the scrollable Vim split report. |
| `&ai [PROMPT]` | AI | Autonomous multi-file code generation and diagram update with automatic build verification and rollback protection. |
| `&advice [QUERY]` | AI | Asks the LLM for architectural advice or refactoring strategy (defaults to active node analysis). |
| `&ok` | AI / Mutation | Applies the pending recommendation from `&advice`. Creates an atomic rollback backup in `.merm/snapshots/`, applies mutations, and verifies build. |
| `&set [PATH]` | Project | Binds the current Mermaid diagram to a target Rust project root (creates/loads `.merm/manifest.json` and maps symbols). |
| `:test [Node] [Input]` | Execution | Executes node test harness with default input `"{}"` and default output `"1"`. (Shortcut: `t`). |
| `:edit [Node]` | Editor | Opens interactive floating Node Editor to modify fields, methods, or stereotypes. (Shortcut: `E` or click node). |
| `:copy` / `:yank` | Clipboard | Copies active split buffer content to system clipboard. (Shortcut: `y`, `Ctrl+C`, or `[📋 Kopyala]` button). |
| `:split` | Buffer | Toggles persistent bottom split buffer open or closed (maximizing diagram). |
| `:config [KEY] [VAL]` | Settings | Views or updates LLM provider, API keys, models, and endpoints (e.g., `:config provider agy`, `:config api_key sk-...`). |
| `:theme <NAME>` | View | Switches active color theme (`monokai`, `terminal`, `mocha`, `dracula`, `nord`, `tokyo`, `gruvbox`, `latte`). |
| `:dir <DIR>` | View | Changes diagram layout direction (`TD`, `LR`, `BT`, `RL`). |
| `:fit` / `:reset` | View | Re-fits diagram with comfortable readable scale (`>= 0.75x`) or resets canvas offset. |
| `:clear` | Buffer | Closes the AI chat / diagnostic report split buffer. |
| `:add <kind> <Name>` | Scaffolding | Adds a new node to the diagram and automatically scaffolds the corresponding Rust module (`src/<name>.rs`) with executable entrypoints. |
| `:connect <A> <B> [label]` | Diagram | Adds a dependency arrow/relation between two nodes in the diagram. |
| `:w` / `:write` | File | Saves diagram state and manifests to `merm.mmd`. |
| `:q` / `:quit` | App | Exits the application (`:wq` saves and exits). |
| `:help` | Help | Displays command and keybinding reference. |

---

## 🤖 LLM Engines & Google Antigravity (`agy`) Binding

`merm` supports three primary LLM backends:

```text
               ┌───────────────────────┐
               │    merm LLM Client    │
               └───┬────────┬────────┬─┘
                   │        │        │
         ┌─────────┘        │        └──────────┐
         ▼                  ▼                   ▼
┌──────────────────┐ ┌─────────────┐ ┌────────────────────┐
│ Google           │ │ Official    │ │ Local Ollama       │
│ Antigravity CLI  │ │ OpenAI API  │ │ or OpenAI-         │
│ (agy)            │ │ (gpt-4o)    │ │ Compatible Server  │
└──────────────────┘ └─────────────┘ └────────────────────┘
```

### 1. Google Antigravity CLI (`agy`)
When `agy` is installed on your system (in `PATH` or `~/.local/bin/agy`), `merm` automatically detects it and sets it as the default LLM provider!
- Query AGY directly from the canvas: `&agy analyze coupling between auth and billing`
- Use AGY for full compatibility checks (`&check`), architectural proposals (`&advice`), and autonomous refactoring (`&ai`).

### 2. OpenAI API (`OPENAI_API_KEY`)
You can supply your OpenAI API key in three convenient ways:
1. **Environment Variable:** `export OPENAI_API_KEY="sk-..."`
2. **Project `.env` file:** Place `OPENAI_API_KEY=sk-...` in your project root.
3. **In-App Interactive Config:**
   ```text
   :config provider openai
   :config api_key sk-your-api-key-here
   :config model gpt-4o
   ```

### 3. Local Ollama or Custom Endpoints
Works seamlessly with local models like `qwen2.5-coder`, `deepseek-coder`, or `llama3`:
```text
:config provider ollama
:config endpoint http://localhost:11434/v1
:config model qwen2.5-coder
```

To view current settings at any time, run `:config` or click the `:cfg` pill!

---

## 🧠 Fabric Pattern Integration & Divergence Auto-Healing (`&advice`, `&check`, `&ok`)

`merm` integrates battle-tested **Fabric system patterns** and knowledge methodologies directly into the architectural co-pilot:

1. **`improve_prompt`**: Clarifies the user's intent, formalizes domain boundaries, and specifies explicit inputs, outputs, and constraints.
2. **`task_planner`**: Decomposes complex refactorings into verifiable micro-steps, checking Rust invariants (ownership, borrowing, lifetimes, `Result`/`Option` error propagation).
3. **`create_design_document`**: Synthesizes C4 Context/Container models and compliant Mermaid class diagrams with verified stereotypes (`<<struct>>`, `<<enum>>`).
4. **`.merm/system.md` Blueprint**: Automatically created in your project root upon initialization. You can inspect, modify, or add custom prompt rules, which `merm` uses immediately for all `&advice` and `&ai` queries.

### 🛡️ Smart Project Binding & Divergence Self-Healing (`&check` ➔ `&ok`)
- **Real Physical File Binding**: `&set` scans your directory and maps every diagram node to physical Rust source files, reporting live disk verification counts.
- **Autonomous Divergence Healing**: When `&check` detects missing files or mismatched AST symbols between the Mermaid diagram and the codebase:
  1. An actionable auto-healing proposal is formulated in real time.
  2. Scaffolding code for missing modules is generated with test harness entrypoints (`pub fn run(input: &str) -> String`).
  3. Simply press **`'o'`** or run **`&ok`**: `merm` creates an atomic rollback snapshot in `.merm/snapshots/`, writes the files, updates manifest bindings, and verifies with `cargo check`!

### 👾 OpenAI Web GUI-Style Animated Pacman Spinner & Tool Call Timeline
During background AI execution (`&check`, `&advice`, `&ok`, `&ai`):
- **Pacman Spinner Animation**: The statusline and split buffer header display a retro cyber-Pacman animation (`ᗧ • • • • 👻` ➔ ` ᗤ • • • 👻` ➔ `     ᗤ💥`) at 60-120 FPS.
- **OpenAI Web-Style Cards**: Live tool calls (`AST Scanner`, `Compiler Verification`, `Snapshot Engine`) and thoughts stream directly into the split buffer with high-contrast syntax highlighting:

```text
┌─ ⚙️  Tool Call: AST Scanner ───────────────────────────────
│  Status: ⚡ In Progress...
│  Detail: Scanning Rust symbols, structs, and modules...
└─────────────────────────────────────────────────────────────
┌─ 💭  Fabric Methodology: task_planner ──────────────────────
│  Micro-Step 1: Scaffold missing OrderService module
│  Micro-Step 2: Add executable run(input: &str) -> String entrypoint
│  Micro-Step 3: Verify build with cargo check rollback gate
└─────────────────────────────────────────────────────────────
👉 Press 'o' or enter `&ok` to automatically scaffold code and heal divergence!
```

---

## 📂 Standalone Diagram Mode & Safe Project Isolation

`merm` is engineered for both standalone Mermaid drafting and deep Rust project co-design:

### 1. Instant Clean Starter Template
When opening or creating a new empty diagram file:
```bash
mkdir my-design
touch architecture.md
merm architecture.md
```
`merm` detects that the file is empty and immediately initializes an uncluttered, single-class starter template (`class App { +run() }`), letting you begin modeling immediately.

### 2. Strict Boundary & `$HOME` Safety Guard
Unlike naive tools that blindly traverse parent directories looking for any `.rs` files or project manifests, `merm` enforces strict root safety:
- **`$HOME` Protection:** `merm` never treats your user home directory (`/home/user`, `~`), root (`/`), or generic folders (`Desktop`, `Downloads`, etc.) as a Rust project root.
- **Zero Canvas Pollution:** Drafting a diagram inside a folder in your home directory will never accidentally scan hundreds of unrelated Rust files or dump 400+ classes into your canvas.
- **Explicit Binding:** Deep AST scanning and `.merm/manifest.json` generation only activate when you run `merm .` inside a Cargo project root, target a project with `merm /path/to/project`, or run `&set /path/to/project` directly inside the canvas.

---

## 🚀 Executable Class Nodes (Input ➔ Run ➔ Output)

When a Rust project is bound to `merm`:
1. Every Rust source file or module corresponds to an architecture class in the diagram.
2. Select any node in the diagram and press **`t`** (or type `:test`).
3. An interactive vector test drawer slides up from the bottom of the window:
   - **Target:** `<NodeName> (src/module.rs)`
   - **Default Input:** Pre-populated with default input `"{}"` (or custom parameters).
   - **Run:** Press `Enter` to execute the node through `merm`'s test harness.
   - **Default Output:** Returns live execution status (`✔ PASS`), duration (e.g. `4ms`), and payload. If the function is void `()` or empty, it **guarantees returning `"1"`**!

```text
┌────────────────────────────────────────────────────────────────────────┐
│ 🚀 Test Node Harness: <AuthService>                                    │
│ Default Input: {}                                                      │
│ Status: SUCCESS (0) | Duration: 3ms | Output Payload: 1                │
└────────────────────────────────────────────────────────────────────────┘
```

---

## ⌨️ Modal Controls & Keybindings

| Key | Mode | Action |
| :--- | :--- | :--- |
| `:` or `&` | Normal | Open interactive command bar (`&agy`, `&check`, `&advice`, `&ok`, `&ai`, `:config`, etc.) |
| `Tab` | Command Mode | Auto-complete or expand smart default prompts for `&advice`, `&ai`, `&agy`, `:test` |
| `t` | Normal (Node selected) | Open Node Test drawer for selected class (default input: `{}`) |
| `E` | Normal (Node selected) | Open interactive floating Node Editor to modify fields, methods, or stereotypes |
| `i` or `K` | Normal (Node selected) | Open Node Inspector modal (type, file binding, fields, methods, relations, test status) |
| `T` | Normal | Cycle color themes (Monokai → Terminal → Mocha → Tokyo Night → Nord → Gruvbox → Dracula → Latte) |
| `a` | Normal | Quick add new class or struct node (`:add class `) |
| `c` | Normal | Quick connect nodes (`:connect `) |
| `e` | Normal (Node selected) | Open bound Rust source file in `$EDITOR` (Neovim, Helix, VSCode) |
| `h`, `j`, `k`, `l` (or `←↓↑→`) | Normal | Pan canvas; if no node is selected, Arrow keys automatically select the first node |
| `+` / `-` | Normal | Zoom in / Zoom out |
| `0` | Normal | Fit diagram to viewport (guarantees >= 0.75x readable scale) |
| `p` | Normal | Pivot layout direction (`TD` ↔ `LR` ↔ `RL` ↔ `BT`) |
| `n` / `N` | Normal | Select and focus next / previous node (shows glowing cyan selection ring) |
| `y` / `Y` or `Ctrl+C` | Normal / Split | Copy active split buffer text to system clipboard (Wayland / X11) |
| `s` | Normal / Split | Toggle persistent split buffer (bottom 48% or maximized diagram) |
| `/` or `f` | Normal | Enter Search / Jump mode |
| `j` / `k` (or `↓` / `↑`) | Split Buffer | Scroll AI chat & diagnostic report down / up by 1 line |
| `d` / `u` (or `PgDn` / `PgUp`) | Split Buffer | Scroll report down / up by 10 lines |
| `g` / `G` | Split Buffer | Jump to top / bottom of report buffer |
| `o` | Split Buffer | Quick-apply proposed recommendation (`&ok`) |
| **Left Click on Node** | Canvas | **Click opens Node Editor**; Click + Drag repositions node on canvas |
| **Mouse Wheel** | Canvas / Split | Zoom canvas in/out, or scroll active split buffer |
| `q` or `Esc` | Any | Close modal/split drawer, or quit application |

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
git clone https://github.com/kadiryildiz283/merm.git
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

# Create or open a clean, isolated standalone diagram
touch my_arch.md && merm my_arch.md

# Open existing Mermaid architecture or markdown notes
merm architecture.md

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
  "kadiryildiz283/merm",
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
│   ├── merm-core/           # AST parser, syn Rust scanner, manifest, node runner, advisor, AGY & OpenAI client
│   ├── merm-render/         # WGPU pipeline, SIMD SvgRasterizer, alpha compositing overlay
│   ├── merm-ui/             # winit 0.30, softbuffer, modal controller, vector overlay widgets, command dock
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

## 🤝 Contributing & Quality Verification

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
