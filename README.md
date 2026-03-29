# modus

**modus** is a desktop 3D visualisation tool for structural vibration and modal analysis. It is almost entirely written using [Claude](https://claude.ai/claude-code) (Anthropic's AI coding assistant).

---

## What it does

modus lets structural engineers and analysts import time-series sensor data, map it onto a 3D model, and explore how a structure moves.

- **Import data** — load CSV or Parquet files containing time-series channels. Channels can represent displacement, velocity, or acceleration in any common unit; modus integrates or differentiates as required to produce consistent displacement data.
- **Build a model** — define nodes (points in 3D space), edges (connections between nodes), glyphs (visual markers), and mesh surfaces. Models are saved as `.ods.json` files that travel alongside their data files.
- **Animate** — play back the measured motion in real time, with adjustable speed, frame rate, and displacement scale. An auto-scale option keeps the animation visually clear regardless of actual signal magnitudes.
- **Visualise** — colour nodes and edges by displacement magnitude using built-in palettes (Viridis, Plasma, Cool, Hot, Turbo), scale node sizes by magnitude, or use both together.
- **Vector arrows** — overlay velocity or acceleration vectors as 3D arrows at each node, with magnitude-proportional lengths, a size slider, and optional contour colouring.
- **Analyse** — click any node to open time-domain, FFT magnitude, and spectrogram plots. Apply bandpass or single-frequency filters that update the animation in real time.
- **Coordinate systems** — define per-node local coordinate systems (CSYS) by composing rotations, save them to a manager, and apply them in bulk. Displacement channels are transformed through the local CSYS before being displayed.
- **Export** — record an animated video via an FFmpeg pipeline, or export nodes/edges to CSV.

---

## Features at a glance

| Category | Details |
|---|---|
| Data formats | CSV, Parquet |
| Channel types | Displacement, Velocity, Acceleration |
| Units (displacement) | mm, m, km, in, ft, mi |
| Units (velocity) | mm/s, m/s, km/h, mph, in/s, ft/s |
| Units (acceleration) | mm/s², m/s², g, in/s², ft/s² |
| 3D rendering | wgpu — lit Phong shading, unlit overlays |
| Glyph shapes | Sphere, Cube, Cylinder, Torus |
| Palettes | Viridis, Plasma, Cool, Hot, Turbo |
| Camera | Orbit / pan / zoom, orthographic or perspective, axis triad |
| Analysis plots | Time-domain, FFT magnitude, spectrogram |
| Number labels | Toggleable per-object labels in the 3D viewport |
| Video export | Screenshot capture + FFmpeg encoding |

---

## Building and running

Requires a Rust toolchain (stable, 1.75+) and, for video export, [FFmpeg](https://ffmpeg.org) on the system PATH.

```bash
# Development build and run
cargo run

# Optimised build
cargo build --release
./target/release/modus

# Windows convenience wrapper
run.bat
```

---

## Project structure

```
src/
  main.rs          — entry point
  app.rs           — top-level state and UI loop
  viewport.rs      — wgpu 3D render pipeline
  table.rs         — node / edge / glyph / mesh editors
  data.rs          — CSV/Parquet import, unit conversion, integration
  fft.rs           — FFT, windowing, filtering, spectrogram
  time_plot.rs     — 2D signal plots
  persist.rs       — JSON save/load, user preferences, options window
  csys_builder.rs  — local coordinate system editor
  create_nodes.rs  — procedural node generation
  export_video.rs  — screenshot capture and FFmpeg pipeline
  shaders/
    viewport.wgsl  — unlit line + lit Phong WGSL shader

examples/
  model.ods.json               — simple beam example
  vibrating_beam.ods.json      — vibrating beam with sensor data
  4_column_structure.ods.json  — four-column portal frame
```

---

## File format

Models are saved as `*.ods.json` — a plain JSON file containing the node/edge/glyph/mesh geometry, channel assignments, saved views, and references to the data files (stored as paths relative to the model file). Data files are not embedded, so large datasets stay separate.

---

## Technology

| Library | Purpose |
|---|---|
| [eframe / egui](https://github.com/emilk/egui) | Immediate-mode GUI |
| [wgpu](https://wgpu.rs) | GPU rendering (WebGPU API) |
| [glam](https://github.com/bitshifter/glam-rs) | Linear algebra |
| [polars](https://pola.rs) | Parquet and CSV parsing |
| [rustfft](https://github.com/ejmahler/RustFFT) | FFT computation |
| [rfd](https://github.com/PolyMC/rfd) | Native file dialogs |
| [egui-phosphor](https://github.com/amPerl/egui-phosphor) | Icon font |

---

## Status

This is an experimental/personal tool under active development. The file format and APIs are not yet stable.

---

*Almost entirely written with [Claude Code](https://claude.ai/claude-code) by Anthropic.*
