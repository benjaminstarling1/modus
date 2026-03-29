# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

**modus** is a desktop 3D visualization tool for structural vibration / modal analysis. It lets engineers import time-series sensor data (CSV/Parquet), assign channels to nodes in a 3D structural model, and animate mode shapes with real-time FFT, spectrogram, and contour-color rendering.

## Commands

```bash
cargo build              # dev build
cargo build --release    # optimized build (opt-level = 3)
cargo run                # run (dev)
cargo run --release      # run (release)
cargo test               # tests
cargo clippy             # lint
cargo fmt                # format
```

On Windows, `run.bat` wraps `cargo run`.

The binary is named `modus` (set in `Cargo.toml`).

## Architecture

The app is built on **eframe/egui** (immediate-mode UI) + **wgpu** (GPU 3D rendering).

### Module map

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point — constructs `App`, starts eframe event loop |
| `src/app.rs` | Central state (`App` struct, ~2500 lines). Owns all sub-state; drives the egui `update()` loop |
| `src/viewport.rs` | `Viewport3D`: wgpu render pipeline, camera, 3D scene (nodes/edges/glyphs/meshes) |
| `src/table.rs` | Egui table editor for rows (nodes), edges, glyphs, meshes |
| `src/time_plot.rs` | 2D plots: time-domain signal, FFT magnitude, spectrogram |
| `src/data.rs` | `Dataset` type, CSV/Parquet import, SI unit conversions, displacement integration |
| `src/persist.rs` | JSON serialization of user preferences and saved views (via `serde_json`) |
| `src/fft.rs` | FFT, windowing, filtering, spectrogram computation (uses `rustfft`) |
| `src/csys_builder.rs` | UI for defining local coordinate systems (CSYS) per node |
| `src/create_nodes.rs` | Procedural node/mesh generation dialog |
| `src/export_video.rs` | Screenshot capture + FFmpeg pipeline for video export |
| `src/shaders/viewport.wgsl` | WGSL shader: unlit pipeline (lines/grid), lit Phong pipeline (surfaces) |

### Data flow

1. **Import** — `data.rs` reads CSV/Parquet into `Dataset` (time vector + raw channel arrays). Velocity/acceleration channels are numerically integrated to displacement.
2. **Model** — `App` holds `rows` (nodes with x/y/z position + channel assignments), `edges`, `glyphs`, `meshes`. Edited via `table.rs`.
3. **Animation** — `AnimState` advances playback time. Each frame, `App` computes per-node displacements from the current datasets and pushes updated geometry to `Viewport3D`.
4. **Render** — `viewport.rs` uploads vertex buffers to the GPU each frame and runs the WGSL shader. The viewport lives inside an egui panel via `egui_wgpu`.
5. **Analysis** — `time_plot.rs` + `fft.rs` render 2D plots for a selected node/channel.
6. **Persist** — `persist.rs` saves/loads the full model + preferences as a JSON `.ods.json` file.

### Key types (all in `app.rs` or `data.rs`)

- `App` — top-level state; owns `Vec<Dataset>`, `Vec<Row>`, `Vec<Edge>`, `Vec<Glyph>`, `Vec<Mesh>`, `AnimState`, `Viewport3D`, UI flags.
- `Dataset` — time-series data: `times: Vec<f64>`, `channels: Vec<Channel>`, computed displacement arrays.
- `Row` — a node: position `(x,y,z)`, channel assignments `(dx/dy/dz, rx/ry/rz)`, color, local CSYS index.
- `AnimState` — playback position, speed, FPS, looping.
- `Viewport3D` — wgpu device/queue, render pipelines, camera (azimuth/elevation/distance, ortho or perspective).

### Persistence format

Files are saved as `*.ods.json` (custom JSON schema). The `examples/` directory contains several `.ods.json` files alongside matching `_data.csv` files used for development/testing.
