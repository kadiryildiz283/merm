# Contributing to `merm`

Thank you for your interest in contributing to **`merm`**! We welcome contributions from developers of all skill levels.

---

## 🛠️ Development Setup

### Prerequisites
- **Rust Toolchain:** Stable Rust 1.80+ (`rustup default stable`)
- **Linux Environment:** Wayland or X11 compositor (tested on Arch, CachyOS, Fedora, Ubuntu)
- **C Libraries:** Standard libc, pkg-config

### Clone and Build
```bash
git clone https://github.com/kadiryildiz/merm.git
cd merm

# Build debug binary
cargo build --workspace

# Run all tests
cargo test --workspace

# Run linter
cargo clippy --all-targets -- -D warnings

# Format check
cargo fmt --check
```

---

## 🏗️ Architecture Overview

The `merm` codebase is structured as a modular Cargo workspace:

- **`crates/merm-core`**: Core layout engine, Mermaid AST parsing, direction pivoting (TD <-> LR), XML sanitization, and UML class diagram parser with semantic tokens.
- **`crates/merm-render`**: WGPU hardware-accelerated pipeline, LOD cache, SvgRasterizer (resvg + tiny-skia SIMD), and 2D view transforms.
- **`crates/merm-ui`**: `winit` 0.30 window management, `softbuffer` presentation, Vim modal navigation state machine (`Normal`, `Pan`, `Search`, `Jump`), and interactive node drag-and-drop.
- **`crates/merm-ipc`**: Zero-overhead Unix Domain Socket IPC server with Linux `SO_PEERCRED` credential validation and JSON-RPC 2.0 protocol.
- **`crates/merm-cli`**: CLI entry point, argument parsing, config loader, and headless mode runner.
- **`editors/merm.nvim`**: Native Neovim Lua plugin for bidirectional cursor tracking and live reload.

---

## 🧪 Testing & Verification Protocol

Before submitting a pull request, ensure all validation gates pass:

1. **Unit & Integration Tests:**
   ```bash
   cargo test --workspace
   ```
2. **Clippy Strict Lints:**
   ```bash
   cargo clippy --all-targets -- -D warnings
   ```
3. **Format Enforcement:**
   ```bash
   cargo fmt -- --check
   ```
4. **Headless Diagram Test:**
   ```bash
   cargo run -p merm-cli -- --headless path/to/sample.md
   ```

---

## 🌿 Pull Request Guidelines

1. Fork the repository and create a feature branch (`git checkout -b feat/my-new-feature`).
2. Adhere to **Conventional Commits** format:
   - `feat: add support for ER diagrams`
   - `fix(engine): resolve edge label quote sanitization`
   - `perf(render): vectorize softbuffer pixel copy`
   - `docs: update Neovim setup instructions`
3. Include tests for any new features or bug fixes.
4. Ensure zero warnings under `cargo clippy`.
5. Open a Pull Request with a clear description of the problem solved and the implementation approach.
