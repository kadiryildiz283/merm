# merm: High-Performance Native Linux Diagram Viewer

`merm` is a native, ultra-low latency diagram viewer designed specifically for terminal and modal editor workflows (Neovim, Helix). It replaces bloated web browser engines (Electron, WebKitGTK, CEF) with Safe Rust, QuickJS headless layout execution, `resvg` rasterization, and WGPU/CPU rendering pipelines.

## Key Features

- **Sub-50ms Cold-Start:** Near-instantaneous visual feedback loops for architectural diagramming.
- **Dual Rendering Pipeline:**
  - **Hardware Acceleration:** WGPU (Vulkan / OpenGL ES) providing 120 FPS fluid navigation.
  - **Automated CPU Fallback:** Software rasterization (`softbuffer`/`tiny-skia` compatible) for headless environments, CI/CD, and virtual machines.
- **IPC Hardening:**
  - Linux `SO_PEERCRED` socket UID matching prevents unauthorized local privilege escalation or socket hijacking on shared machines.
  - Unix Domain Sockets bound with strict `0600` POSIX file mode in `$XDG_RUNTIME_DIR`.
- **Lightweight Event Engine:** Direct non-blocking socket polling avoiding multithreaded runtime bloat.
- **Execution Watchdog:** Hard execution budgets (2000ms timeout) and memory quotas protecting against ReDoS / pathological grammars.
- **Bidirectional Modal Sync:** Full integration with Neovim (`editors/merm.nvim`) via JSON-RPC 2.0.

## Crate Architecture

```text
merm/
├── crates/
│   ├── merm-cli     # CLI entry point, argument parsing, process orchestration
│   ├── merm-core    # Diagram extraction, AST direction pivoting, watchdog engine
│   ├── merm-render  # 2D affine transform, LOD cache, WGPU + CPU fallback renderer
│   ├── merm-ui      # Modal navigation state machine, Vim keybindings, HUD
│   └── merm-ipc     # Non-blocking UDS JSON-RPC server with SO_PEERCRED verification
└── editors/
    └── merm.nvim    # Neovim Lua integration plugin
```

## Building and Testing

```bash
# Run all workspace tests
cargo test --workspace

# Run CLI binary
cargo run -p merm-cli -- --help

# Test CPU fallback mode explicitly
cargo run -p merm-cli -- --software-render
```
