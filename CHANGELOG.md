# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.1.0] - 2026-09-20

### Added
- **Native GUI Window:** High-performance Linux desktop window built with `winit` 0.30 and `softbuffer`.
- **WGPU & Software Fallback:** 120 FPS hardware-accelerated WGPU rendering with automatic, seamless fallback to CPU software rasterization (tiny-skia SIMD).
- **Deep Mermaid Class Diagram Engine:**
  - Standard UML compartments (Header, Attributes, Methods).
  - Semantic syntax highlighting tokens: visibility (`+`, `-`, `#`, `~`), types, variable names, method names, comments (`//`, `%%`), and stereotypes (`«interface»`, `«struct»`, `«abstract»`, `«enum»`).
  - Class-level docstrings and inline comments.
  - Complete UML relationship marker set: Inheritance (`<|--`), Realization (`<|..`), Composition (`*--`), Aggregation (`o--`), Association (`-->`), Dependency (`..>`).
- **Interactive Node Drag & Drop:** Click and freely reposition any class or node on the canvas with dynamic Bézier relation arrows bending and tracking in real time.
- **Vim Modal Navigation State Machine:**
  - `Normal Mode`: `h, j, k, l` panning, `+ / -` zooming, `0` fit-to-screen, `c` cycle themes, `Tab` pivot layout direction (TD <-> LR), `n / N` cycle nodes.
  - `Pan Mode`: Mouse drag and wheel support.
  - `Search Mode`: Fuzzy node jumping.
- **6 Designer Developer Color Themes:**
  - Catppuccin Mocha (default)
  - Tokyo Night
  - Nord
  - Gruvbox Dark
  - Dracula
  - Catppuccin Latte (Light)
- **Zero-Overhead IPC Server:**
  - Unix Domain Socket JSON-RPC 2.0 interface.
  - Linux `SO_PEERCRED` kernel credential verification.
  - Bidirectional Neovim plugin (`editors/merm.nvim`) for cursor tracking and live file reloading.
- **Headless Mode:** CLI option `--headless` for batch rendering, CI verification, and diagram syntax checks.
- **Config Loader:** TOML configuration support at `~/.config/merm/config.toml`.

### Performance Optimizations
- Cached `usvg::Tree` XML parser and pixel buffers across frames to eliminate heap allocations and XML re-parsing during panning and dragging.
- Cached softbuffer surface dimensions to eliminate buffer reallocation thrashing on mouse motion.
- Replaced polling control flow with OS-driven event loop (0% idle CPU).
- Vectorized SIMD pixel copy loop in `SvgRasterizer`.
