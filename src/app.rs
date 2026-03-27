use crate::data::{max_duration, sample_qualified, show_import_panel, Dataset};
use crate::table::{row_position, show_top_pane, Edge, Row, Glyph, GlyphShape, Mesh, TableTab, identity_mat3};
use crate::viewport::{show_viewport, MeshRenderData, Viewport3D};
use crate::persist::{UserPrefs, ModelFile, SavedView, DistanceUnit, show_options_window, show_views_window};
use crate::csys_builder::{
    CsysBuilder, CsysManager, show_csys_builder_window, show_csys_manager_panel,
};
use crate::fft::{FftPaneState, AnimMode, FilterMode, show_fft_panel, sample_freq_based, sample_filtered};
use crate::time_plot::{TimePlotState, show_time_plot_window};
use crate::create_nodes::{CreateNodesState, show_create_nodes_window};
use crate::export_video::{
    ExportVideoState, ExportPhase, ExportAction,
    show_export_video_window, process_screenshot, run_ffmpeg_encode,
};

// ─────────────────────────────────────────────────────────────────────────────
// Animation state
// ─────────────────────────────────────────────────────────────────────────────

pub struct AnimState {
    pub playing:      bool,
    pub time:         f64,   // current playback position (seconds)
    pub speed:        f32,   // multiplier (1.0 = real-time)
    pub fps:          f32,   // display frame rate (frames per second)
    pub looping:      bool,  // wrap at end of duration
    last_instant:     Option<std::time::Instant>,
    frame_accum:      f64,   // accumulated wall-clock time for frame stepping
}

impl Default for AnimState {
    fn default() -> Self {
        Self { playing: false, time: 0.0, speed: 1.0, fps: 30.0, looping: true, last_instant: None, frame_accum: 0.0 }
    }
}

impl AnimState {
    /// Advance the playback clock in discrete frame steps.
    pub fn tick(&mut self, duration: f64) -> f64 {
        let now = std::time::Instant::now();
        let dt = if let Some(prev) = self.last_instant {
            now.duration_since(prev).as_secs_f64()
        } else {
            0.0
        };
        self.last_instant = Some(now);

        if self.playing && duration > 0.0 {
            // Accumulate wall-clock time scaled by speed,
            // then advance in whole-frame increments.
            self.frame_accum += dt * self.speed as f64;
            let frame_dt = 1.0 / (self.fps.max(1.0) as f64);
            while self.frame_accum >= frame_dt {
                self.frame_accum -= frame_dt;
                self.time += frame_dt;
            }
            if self.time > duration {
                if self.looping {
                    self.time %= duration;
                } else {
                    self.time = duration;
                    self.playing = false;
                }
            }
        } else if !self.playing {
            // Reset clock edge so paused → play doesn't jump.
            self.last_instant = None;
            self.frame_accum = 0.0;
        }
        dt
    }

    /// Step forward by one frame.
    pub fn step_forward(&mut self, duration: f64) {
        let frame_dt = 1.0 / (self.fps.max(1.0) as f64);
        self.time += frame_dt;
        if duration > 0.0 && self.time > duration {
            if self.looping {
                self.time %= duration;
            } else {
                self.time = duration;
            }
        }
    }

    /// Step backward by one frame.
    pub fn step_back(&mut self, duration: f64) {
        let frame_dt = 1.0 / (self.fps.max(1.0) as f64);
        self.time -= frame_dt;
        if self.time < 0.0 {
            if duration > 0.0 && self.looping {
                self.time = duration + self.time; // wrap to end
            } else {
                self.time = 0.0;
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Interaction tools
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default)]
pub enum InteractionTool {
    #[default]
    None,
    Details,
    Select,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum SelectionFilter {
    #[default]
    Node,
    Edge,
    Glyph,
}

// ─────────────────────────────────────────────────────────────────────────────
// Visualisation mode
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum VisMode {
    #[default]
    None,
    ContourColor,
    SizeScale,
    ContourAndSize,
}

// ─────────────────────────────────────────────────────────────────────────────
// Colour palettes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub enum Palette {
    #[default]
    Viridis,
    Plasma,
    Cool,
    Hot,
    Turbo,
}

impl Palette {
    pub fn label(&self) -> &'static str {
        match self {
            Palette::Viridis => "Viridis",
            Palette::Plasma  => "Plasma",
            Palette::Cool    => "Cool",
            Palette::Hot     => "Hot",
            Palette::Turbo   => "Turbo",
        }
    }

    /// Map a value in [0, 1] to an RGBA colour.
    /// If `reverse` is true the palette direction is flipped.
    pub fn sample(&self, t: f32, reverse: bool) -> [f32; 4] {
        let t = if reverse { 1.0 - t.clamp(0.0, 1.0) } else { t.clamp(0.0, 1.0) };
        let stops: &[[f32; 3]] = match self {
            Palette::Viridis => &[
                [0.267, 0.005, 0.329],
                [0.128, 0.567, 0.551],
                [0.204, 0.788, 0.467],
                [0.769, 0.882, 0.216],
                [0.993, 0.906, 0.144],
            ],
            Palette::Plasma => &[
                [0.050, 0.030, 0.528],
                [0.558, 0.056, 0.654],
                [0.899, 0.219, 0.458],
                [0.980, 0.565, 0.163],
                [0.940, 0.975, 0.131],
            ],
            Palette::Cool => &[
                [0.0,  1.0, 1.0],
                [0.25, 0.75, 1.0],
                [0.5,  0.5, 1.0],
                [0.75, 0.25, 1.0],
                [1.0,  0.0, 1.0],
            ],
            Palette::Hot => &[
                [0.04, 0.0, 0.0],
                [0.6,  0.0, 0.0],
                [1.0,  0.4, 0.0],
                [1.0,  1.0, 0.0],
                [1.0,  1.0, 1.0],
            ],
            Palette::Turbo => &[
                [0.190, 0.072, 0.232],
                [0.065, 0.365, 0.860],
                [0.120, 0.724, 0.830],
                [0.450, 0.978, 0.347],
                [0.890, 0.860, 0.140],
                [0.976, 0.533, 0.084],
                [0.761, 0.028, 0.051],
            ],
        };

        let n    = stops.len() - 1;
        let idx  = (t * n as f32).floor() as usize;
        let idx  = idx.min(n - 1);
        let frac = t * n as f32 - idx as f32;
        let a    = stops[idx];
        let b    = stops[idx + 1];
        [
            a[0] + (b[0] - a[0]) * frac,
            a[1] + (b[1] - a[1]) * frac,
            a[2] + (b[2] - a[2]) * frac,
            1.0,
        ]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CsysTarget {
    None,
    Node(usize),
    Manager(String),
}

impl Default for CsysTarget {
    fn default() -> Self { Self::None }
}

pub struct App {
    datasets:      Vec<Dataset>,
    channel_names: Vec<String>,
    rows:          Vec<Row>,
    edges:         Vec<Edge>,
    glyphs:        Vec<Glyph>,
    meshes:        Vec<Mesh>,
    clipboard:     Option<Row>,
    active_tab:    TableTab,
    viewport:      Viewport3D,

    // Animation & visualisation
    anim:          AnimState,
    vis_mode:      VisMode,
    palette:       Palette,
    reverse_pal:   bool,
    node_size:     f32,
    disp_scale:    f32,
    auto_scale:    bool,
    size_vis_scale: f32,  // max node size multiplier for Size vis mode
    global_node_color: [f32; 3],
    global_edge_color: [f32; 3],
    edge_thickness:    f32,
    edge_contour:      bool,  // color edges by endpoint contour colors
    selected_node:     Option<usize>,
    selected_glyph:    Option<usize>,
    selected_edge:     Option<usize>,

    // Persistence
    prefs:            UserPrefs,
    show_options:     bool,
    show_views:       bool,
    saved_views:      Vec<SavedView>,
    current_file:     Option<std::path::PathBuf>,

    // CSYS Builder
    csys_builder:        CsysBuilder,
    show_csys_builder:   bool,
    /// Which target is being edited. None = standalone/global.
    csys_builder_target: CsysTarget,
    /// Written by show_csys_builder_window when Apply is clicked.
    csys_apply_result:   Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<crate::csys_builder::CsysOp>)>,
    /// Written by show_csys_builder_window when Save to Manager is clicked.
    csys_save_mgr_result: Option<(String, [[f32; 3]; 3], [[f32; 3]; 3], Vec<crate::csys_builder::CsysOp>)>,

    // CSYS Manager
    csys_manager:     CsysManager,
    /// Pending matrix to apply to all selected nodes from the manager.
    csys_mgr_apply:   Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<crate::csys_builder::CsysOp>)>,
    csys_mgr_edit:    Option<(String, [[f32; 3]; 3], Vec<crate::csys_builder::CsysOp>)>,

    // Create Nodes dialog
    show_create_nodes:   bool,
    create_nodes:        CreateNodesState,

    // Viewport interaction mode
    interaction_tool:    InteractionTool,
    selection_filter:    SelectionFilter,

    // FFT analysis
    fft_state: FftPaneState,

    // Wireframe overlay
    show_wireframe:      bool,

    // Export Animation
    show_export_video:   bool,
    export_video:        ExportVideoState,

    // Time Plot
    show_time_plot:      bool,
    time_plot:           TimePlotState,

    // Theme tracking (to avoid setting visuals every frame)
    last_dark_mode: bool,

    // Current model distance unit (per-session, not persisted)
    current_distance_unit: DistanceUnit,
}

impl App {
    /// Build a `Visuals` for the given dark/light mode with all our customisations.
    fn build_visuals(dark: bool) -> egui::Visuals {
        let mut v = if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        v.window_corner_radius = egui::CornerRadius::same(6);
        if dark {
            v.panel_fill = egui::Color32::from_rgb(28, 28, 32);
            v.faint_bg_color = egui::Color32::from_rgb(35, 35, 42);
            v.text_cursor.stroke.color = egui::Color32::WHITE;
            v.selection.stroke.color = egui::Color32::WHITE;
        } else {
            // Warm light-gray scheme so the white I-beam cursor has contrast
            let panel = egui::Color32::from_rgb(215, 218, 222);
            let window = egui::Color32::from_rgb(225, 228, 232);
            let faint  = egui::Color32::from_rgb(205, 208, 214);
            let edit_bg = egui::Color32::from_rgb(195, 198, 204);

            v.panel_fill     = panel;
            v.window_fill    = window;
            v.window_stroke  = egui::Stroke::new(1.0, egui::Color32::from_rgb(170, 174, 180));
            v.faint_bg_color = faint;
            v.extreme_bg_color = edit_bg;
            v.code_bg_color  = egui::Color32::from_rgb(200, 203, 210);
            v.text_cursor.stroke.color = egui::Color32::BLACK;
            v.selection.stroke.color = egui::Color32::BLACK;
        }
        v
    }

    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let wgpu_render_state = cc.wgpu_render_state.as_ref().expect("wgpu required");
        Viewport3D::init_renderer(wgpu_render_state);

        // Load default example if it exists, otherwise start with empty data
        let default_path = std::path::Path::new("examples/4_column_structure.ods.json");
        let (datasets, channel_names, rows, edges, glyphs, meshes, current_file) = 
            match ModelFile::load_from_file(default_path) {
                Ok(mf) => (mf.datasets, mf.channel_names, mf.rows, mf.edges, mf.glyphs, mf.meshes, Some(default_path.to_path_buf())),
                Err(_) => (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), None),
            };

        let prefs = UserPrefs::load();

        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);

        let visuals = Self::build_visuals(prefs.dark_mode);
        cc.egui_ctx.set_visuals(visuals);

        let initial_dark_mode = prefs.dark_mode;
        let initial_distance_unit = prefs.default_distance_unit.clone();
        let mut app = Self {
            datasets,
            channel_names,
            rows,
            edges,
            glyphs,
            meshes,
            clipboard:     None,
            active_tab:    TableTab::default(),
            viewport:      Viewport3D::default(),

            anim:          AnimState {
                speed: prefs.default_speed,
                fps:   prefs.default_fps,
                ..Default::default()
            },
            vis_mode:      prefs.default_vis_mode.clone(),
            palette:       prefs.default_palette.clone(),
            reverse_pal:   prefs.default_reverse_pal,
            node_size:     prefs.default_node_size,
            disp_scale:    1.0,
            auto_scale:    prefs.default_auto_scale,
            size_vis_scale: 3.0,
            global_node_color: [1.0, 0.85, 0.1],
            global_edge_color: [0.3, 0.85, 1.0],
            edge_thickness:    0.15,
            edge_contour:      false,
            selected_node:     None,
            selected_glyph:    None,
            selected_edge:     None,

            prefs,
            show_options:  false,
            show_views:    false,
            saved_views:   Vec::new(),
            current_file,

            csys_builder:         CsysBuilder::default(),
            show_csys_builder:    false,
            csys_builder_target:  CsysTarget::None,
            csys_apply_result:    None,
            csys_save_mgr_result: None,

            csys_manager:     CsysManager::default(),
            csys_mgr_apply:   None,
            csys_mgr_edit:    None,

            show_create_nodes:  false,
            create_nodes:       CreateNodesState::default(),

            interaction_tool:    InteractionTool::None,
            selection_filter:    SelectionFilter::Node,
            fft_state:        FftPaneState::default(),

            show_wireframe:     false,

            show_export_video:  false,
            export_video:       ExportVideoState::default(),

            show_time_plot:     false,
            time_plot:          TimePlotState::default(),

            last_dark_mode:   initial_dark_mode,

            current_distance_unit: initial_distance_unit,
        };
        app.viewport.orthographic = app.prefs.default_orthographic;
        app.disp_scale = app.compute_auto_scale();
        app.fit_to_model();
        app
    }


    // ── Auto-scale heuristic ──────────────────────────────────────────────
    // Target: max displacement ≈ 10 % of bounding-box diagonal.
    fn compute_auto_scale(&self) -> f32 {
        let diag = self.bounding_diag();
        if diag < 1e-9 { return 1.0; }

        let max_d = self.global_max_disp();
        if max_d < 1e-12 { return 1.0; }

        // At render time: displacement_m * scale * si_to_model is added to positions.
        // Target: max rendered displacement = 10% of bounding diagonal.
        // So: max_d * scale * si_to_model = diag * 0.10
        //     scale = (diag * 0.10) / (max_d * si_to_model)
        let si_to_model = (1.0 / self.current_distance_unit.to_meters()) as f32;
        (diag * 0.10) / (max_d * si_to_model)
    }

    /// Bounding-box diagonal of all nodes with valid positions.
    fn bounding_diag(&self) -> f32 {
        let positions: Vec<[f32; 3]> = self.rows.iter().filter_map(row_position).collect();
        if positions.is_empty() { return 1.0; }

        let (mut mn, mut mx) = (positions[0], positions[0]);
        for p in &positions {
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }
        ((mx[0]-mn[0]).powi(2) + (mx[1]-mn[1]).powi(2) + (mx[2]-mn[2]).powi(2)).sqrt()
    }

    /// Adjust the viewport camera so the entire model is visible.
    /// Sets target to the bounding-box center and distance so the bounding
    /// sphere fits comfortably in the viewport.
    fn fit_to_model(&mut self) {
        let positions: Vec<[f32; 3]> = self.rows.iter().filter_map(row_position).collect();
        if positions.is_empty() { return; }

        // Bounding box min/max
        let (mut mn, mut mx) = (positions[0], positions[0]);
        for p in &positions {
            for k in 0..3 {
                mn[k] = mn[k].min(p[k]);
                mx[k] = mx[k].max(p[k]);
            }
        }

        // Center of bounding box
        let center = glam::Vec3::new(
            (mn[0] + mx[0]) * 0.5,
            (mn[1] + mx[1]) * 0.5,
            (mn[2] + mx[2]) * 0.5,
        );
        self.viewport.target = center;

        // Bounding sphere radius (max distance from center to any node)
        let radius = positions.iter().map(|p| {
            let dx = p[0] - center.x;
            let dy = p[1] - center.y;
            let dz = p[2] - center.z;
            (dx * dx + dy * dy + dz * dz).sqrt()
        }).fold(0.0_f32, f32::max).max(0.1);

        // Set distance so the sphere fits within the FOV (45°) with a small margin.
        // distance = radius / sin(fov/2)  ≈ radius / tan(fov/2) for small angles,
        // but we use tan for accuracy. Add 20% padding.
        let half_fov = 45_f32.to_radians() / 2.0;
        self.viewport.distance = radius / half_fov.tan() * 1.2;
    }

    /// Convert all world-space values when switching distance unit.
    fn convert_units(&mut self, new_unit: &DistanceUnit) {
        let f = self.current_distance_unit.convert_factor(new_unit) as f32;
        if (f - 1.0).abs() < 1e-9 { return; }

        // Scale node positions (stored as strings)
        let factor = self.current_distance_unit.convert_factor(new_unit);
        for row in &mut self.rows {
            for coord in [&mut row.x, &mut row.y, &mut row.z] {
                if let Ok(v) = coord.parse::<f64>() {
                    let new_v = v * factor;
                    *coord = format_coord(new_v);
                }
            }
        }

        // Scale viewport camera
        self.viewport.target *= f;
        self.viewport.distance *= f;

        // node_size is normalised (0..1 fraction of max), so it does NOT scale.

        // Scale glyph sizes and offsets (both are in world units)
        for g in &mut self.glyphs {
            g.size *= f;
            for k in 0..3 { g.position_offset[k] *= f; }
        }

        // disp_scale is unit-independent — si_to_model handles the
        // conversion at the render site, so no scaling is needed here.

        self.current_distance_unit = new_unit.clone();
    }
}

/// Format a coordinate value with up to 10 significant digits, stripping
/// trailing zeros so that e.g. 1.5 stays "1.5" rather than "1.5000000000".
fn format_coord(v: f64) -> String {
    if v == 0.0 { return "0".to_string(); }
    // Use 10 significant digits — enough to round-trip through most
    // unit conversions without visible floating-point noise.
    let s = format!("{:.10e}", v);
    // Parse back and format without scientific notation, trimming zeros.
    if let Ok(parsed) = s.parse::<f64>() {
        let abs = parsed.abs();
        let decimals = if abs >= 1000.0 { 6 }
            else if abs >= 1.0   { 8 }
            else if abs >= 0.001 { 10 }
            else { 12 };
        let formatted = format!("{:.prec$}", parsed, prec = decimals);
        formatted.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        format!("{}", v)
    }
}

impl App {
    fn global_max_disp(&self) -> f32 {
        use crate::data::channel_max_displacement;
        let mut max_d = 0.0_f32;
        for row in &self.rows {
            for idx in [row.dx, row.dy, row.dz] {
                if idx == 0 { continue; }
                if let Some(ch) = self.channel_names.get(idx - 1) {
                    max_d = max_d.max(channel_max_displacement(&self.datasets, ch));
                }
            }
        }
        max_d
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ── Advance playback clock ────────────────────────────────────────
        let duration = max_duration(&self.datasets, &self.channel_names, &self.rows);
        self.anim.tick(duration);
        if self.anim.playing {
            ctx.request_repaint();
        }

        // ── Apply theme (only when changed) ──────────────────────────────
        if self.prefs.dark_mode != self.last_dark_mode {
            self.last_dark_mode = self.prefs.dark_mode;
            ctx.set_visuals(Self::build_visuals(self.prefs.dark_mode));
        }

        // ── Menu bar ──────────────────────────────────────────────────────
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New").clicked() {
                        self.datasets.clear();
                        self.channel_names.clear();
                        self.rows.clear();
                        self.edges.clear();
                        self.glyphs.clear();
                        self.meshes.clear();
                        self.saved_views.clear();
                        self.current_file  = None;
                        self.anim.time     = 0.0;
                        self.anim.playing  = false;
                        self.vis_mode      = self.prefs.default_vis_mode.clone();
                        self.palette       = self.prefs.default_palette.clone();
                        self.reverse_pal   = self.prefs.default_reverse_pal;
                        self.node_size     = self.prefs.default_node_size;
                        self.auto_scale    = self.prefs.default_auto_scale;
                        self.disp_scale    = 1.0;
                        ui.close_menu();
                    }
                    if ui.button("Open...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("ODS Model", &["ods.json"])
                            .set_title("Open Model")
                            .pick_file()
                        {
                            match ModelFile::load_from_file(&path) {
                                Ok(mf) => {
                                    self.datasets      = mf.datasets;
                                    self.channel_names = mf.channel_names;
                                    self.rows          = mf.rows;
                                    self.edges         = mf.edges;
                                    self.glyphs        = mf.glyphs;
                                    self.meshes        = mf.meshes;
                                    self.saved_views   = mf.saved_views;
                                    self.current_file  = Some(path);
                                    self.anim.time     = 0.0;
                                    self.anim.playing  = false;
                                    self.vis_mode      = self.prefs.default_vis_mode.clone();
                                    self.palette       = self.prefs.default_palette.clone();
                                    self.reverse_pal   = self.prefs.default_reverse_pal;
                                    self.node_size     = self.prefs.default_node_size;
                                    self.auto_scale    = self.prefs.default_auto_scale;
                                    self.disp_scale    = self.compute_auto_scale();
                                    self.fit_to_model();
                                }
                                Err(e) => eprintln!("Failed to open model: {e}"),
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    let can_save = self.current_file.is_some();
                    if ui.add_enabled(can_save, egui::Button::new("Save")).clicked() {
                        if let Some(path) = &self.current_file.clone() {
                            let mf = ModelFile {
                                datasets:      self.datasets.clone(),
                                channel_names: self.channel_names.clone(),
                                rows:          self.rows.clone(),
                                edges:         self.edges.clone(),
                                glyphs:        self.glyphs.clone(),
                                meshes:        self.meshes.clone(),
                                saved_views:   self.saved_views.clone(),
                            };
                            if let Err(e) = mf.save_to_file(path) {
                                eprintln!("Failed to save: {e}");
                            }
                        }
                        ui.close_menu();
                    }
                    if ui.button("Save As...").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("ODS Model", &["ods.json"])
                            .set_file_name("model.ods.json")
                            .set_title("Save Model As")
                            .save_file()
                        {
                            let mf = ModelFile {
                                datasets:      self.datasets.clone(),
                                channel_names: self.channel_names.clone(),
                                rows:          self.rows.clone(),
                                edges:         self.edges.clone(),
                                glyphs:        self.glyphs.clone(),
                                meshes:        self.meshes.clone(),
                                saved_views:   self.saved_views.clone(),
                            };
                            if let Err(e) = mf.save_to_file(&path) {
                                eprintln!("Failed to save: {e}");
                            } else {
                                self.current_file = Some(path);
                            }
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Options...").clicked() {
                        self.show_options = true;
                        ui.close_menu();
                    }
                });
                ui.menu_button("Tools", |ui| {
                    if ui.button("Views…").clicked() {
                        self.show_views = true;
                        ui.close_menu();
                    }
                    if ui.button("Create Nodes…").clicked() {
                        self.show_create_nodes = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("CSYS Builder…").clicked() {
                        self.csys_builder_target = CsysTarget::None;
                        self.csys_builder.load_from_matrix(crate::table::identity_mat3());
                        self.show_csys_builder = true;
                        ui.close_menu();
                    }
                    if ui.button("Export Animation…").clicked() {
                        self.export_video.fps = self.anim.fps;
                        self.show_export_video = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Time Plot…").clicked() {
                        self.show_time_plot = true;
                        ui.close_menu();
                    }
                });
            });
        });

        // ── Left panel: Import / FFT / CSYS Manager ───────────────────
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(280.0)
            .min_width(200.0)
            .max_width(420.0)
            .show(ctx, |ui| {
                let half = (ui.available_height() / 2.0).max(100.0);

                // Bottom section: CSYS manager (anchored to bottom, resizable)
                egui::TopBottomPanel::bottom("csys_mgr_panel")
                    .resizable(true)
                    .default_height(half * 0.5)
                    .min_height(60.0)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical().id_salt("csys_mgr_scroll").show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            let sel_count = self.rows.iter().filter(|r| r.selected).count();
                            show_csys_manager_panel(
                                ui,
                                &mut self.csys_manager,
                                sel_count,
                                &mut self.csys_mgr_apply,
                                &mut self.csys_mgr_edit,
                            );
                        });
                    });

                // Middle section: FFT analysis (anchored above CSYS, resizable)
                egui::TopBottomPanel::bottom("fft_mid_panel")
                    .resizable(true)
                    .default_height(half)
                    .min_height(60.0)
                    .show_inside(ui, |ui| {
                        ui.add_space(4.0);
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = 4.0;
                            show_fft_panel(
                                ui,
                                &mut self.fft_state,
                                &self.datasets,
                                &self.channel_names,
                            );
                        });
                    });

                // Top section: data import (fills remaining space, auto-sizes)
                ui.add_space(4.0);
                egui::ScrollArea::vertical().id_salt("import_scroll").show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    show_import_panel(ui, &mut self.datasets, &mut self.channel_names);
                });
            });

        // ── Bottom playback panel ─────────────────────────────────────────
        egui::TopBottomPanel::bottom("playback_panel")
            .resizable(false)
            .min_height(76.0)
            .show(ctx, |ui| {
                ui.add_space(4.0);
                // ── Row 1: Playback controls ──────────────────────────────
                ui.horizontal(|ui| {
                    let btn_size = egui::vec2(32.0, 28.0);
                    // Step back one frame
                    if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new(egui_phosphor::regular::SKIP_BACK).size(18.0)))
                        .on_hover_text("Step back one frame")
                        .clicked()
                    {
                        self.anim.playing = false;
                        self.anim.step_back(duration);
                    }
                    // Play / Pause
                    let icon = if self.anim.playing { egui_phosphor::regular::PAUSE } else { egui_phosphor::regular::PLAY };
                    if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new(icon).size(18.0))).clicked() {
                        if !self.anim.playing {
                            // If at end of a non-looping anim, restart
                            if !self.anim.looping && duration > 0.0 && self.anim.time >= duration - 1e-6 {
                                self.anim.time = 0.0;
                            }
                            self.anim.playing = true;
                        } else {
                            self.anim.playing = false;
                        }
                    }
                    // Stop / rewind
                    if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new(egui_phosphor::regular::STOP).size(18.0))).clicked() {
                        self.anim.playing = false;
                        self.anim.time    = 0.0;
                    }
                    // Step forward one frame
                    if ui.add_sized(btn_size, egui::Button::new(egui::RichText::new(egui_phosphor::regular::SKIP_FORWARD).size(18.0)))
                        .on_hover_text("Step forward one frame")
                        .clicked()
                    {
                        self.anim.playing = false;
                        self.anim.step_forward(duration);
                    }

                    ui.add_space(6.0);

                    // Time scrubber
                    let dur = duration as f32;
                    let mut t = self.anim.time as f32;
                    ui.label(format!("t: {:.3} s", t));
                    if ui.add(egui::Slider::new(&mut t, 0.0..=dur.max(0.01))
                        .show_value(false)
                    ).changed() {
                        self.anim.time = t as f64;
                    }

                    ui.add_space(4.0);

                    // Loop
                    ui.checkbox(&mut self.anim.looping, "Loop");

                    ui.add_space(4.0);

                    // Speed
                    ui.label("Speed:");
                    ui.add_sized([48.0, 18.0],
                        egui::DragValue::new(&mut self.anim.speed)
                            .range(0.01..=20.0).speed(0.05).suffix("x")
                    );

                    ui.add_space(4.0);

                    // Frame rate
                    ui.label("FPS:");
                    ui.add_sized([48.0, 18.0],
                        egui::DragValue::new(&mut self.anim.fps)
                            .range(1.0..=1000.0).speed(1.0)
                    );
                });
                ui.add_space(4.0);
                // ── Row 2: Scale, Vis, Wireframe ─────────────────────────
                ui.horizontal(|ui| {
                    // Displacement scale
                    ui.label("Disp scale:");
                    ui.checkbox(&mut self.auto_scale, "Auto");
                    if self.auto_scale {
                        self.disp_scale = self.compute_auto_scale();
                        ui.label(format!("{:.4}", self.disp_scale));
                    } else {
                        ui.add(
                            egui::DragValue::new(&mut self.disp_scale)
                                .range(0.0..=1e9).speed(0.01)
                        );
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Vis mode
                    ui.label("Vis:");
                    egui::ComboBox::from_id_salt("vis_mode")
                        .selected_text(match &self.vis_mode {
                            VisMode::None           => "None",
                            VisMode::ContourColor   => "Contour",
                            VisMode::SizeScale      => "Size",
                            VisMode::ContourAndSize => "Both",
                        })
                        .width(72.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.vis_mode, VisMode::None,           "None");
                            ui.selectable_value(&mut self.vis_mode, VisMode::ContourColor,   "Contour");
                            ui.selectable_value(&mut self.vis_mode, VisMode::SizeScale,      "Size");
                            ui.selectable_value(&mut self.vis_mode, VisMode::ContourAndSize, "Both");
                        });

                    // Palette selector (shown when contour active)
                    if self.vis_mode == VisMode::ContourColor || self.vis_mode == VisMode::ContourAndSize {
                        egui::ComboBox::from_id_salt("palette")
                            .selected_text(self.palette.label())
                            .width(72.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut self.palette, Palette::Viridis, "Viridis");
                                ui.selectable_value(&mut self.palette, Palette::Plasma,  "Plasma");
                                ui.selectable_value(&mut self.palette, Palette::Cool,    "Cool");
                                ui.selectable_value(&mut self.palette, Palette::Hot,     "Hot");
                                ui.selectable_value(&mut self.palette, Palette::Turbo,   "Turbo");
                            });
                        ui.checkbox(&mut self.reverse_pal, "Rev");
                    }

                    // Size scale (shown when size vis active)
                    if self.vis_mode == VisMode::SizeScale || self.vis_mode == VisMode::ContourAndSize {
                        ui.label("Size Scale:");
                        ui.add_sized([48.0, 18.0],
                            egui::DragValue::new(&mut self.size_vis_scale)
                                .range(1.0..=20.0).speed(0.1).suffix("x")
                        );
                    }

                    // Edge contour (only when contour vis active)
                    if self.vis_mode == VisMode::ContourColor || self.vis_mode == VisMode::ContourAndSize {
                        ui.checkbox(&mut self.edge_contour, "Edge Contour");
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Wireframe overlay
                    ui.checkbox(&mut self.show_wireframe, "Wireframe");
                });
                ui.add_space(2.0);
            });

        // ── Central area ──────────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Top pane: tabbed node / edge tables
            egui::TopBottomPanel::top("table_panel")
                .resizable(true)
                .default_height(240.0)
                .min_height(90.0)
                .show_inside(ui, |ui| {
                    show_top_pane(
                        ui,
                        &mut self.active_tab,
                        &mut self.rows,
                        &mut self.edges,
                        &mut self.glyphs,
                        &mut self.meshes,
                        &self.channel_names,
                        &mut self.clipboard,
                        self.current_distance_unit.label(),
                    );
                });

            // ── Viewport toolbar ─────────────────────────────────────────
            egui::TopBottomPanel::top("viewport_toolbar")
                .resizable(false)
                .min_height(28.0)
                .show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        // ── Node group ──
                        ui.label("Node Size:");
                        ui.add_sized([48.0, 18.0],
                            egui::DragValue::new(&mut self.node_size)
                                .range(0.01..=1.0).speed(0.005)
                        );
                        ui.label("Color:");
                        ui.color_edit_button_rgb(&mut self.global_node_color);

                        ui.separator();

                        // ── Edge group ──
                        ui.label("Edge Size:");
                        ui.add_sized([48.0, 18.0],
                            egui::DragValue::new(&mut self.edge_thickness)
                                .range(0.001..=1.0).speed(0.005)
                        );
                        ui.label("Color:");
                        ui.color_edit_button_rgb(&mut self.global_edge_color);
                    });
                });

            // ── Compute animated geometry ─────────────────────────────────
            let t = self.anim.time;
            let scale = self.disp_scale;

            // Imported displacement data is always in SI metres.
            // Convert to the model's current distance unit.
            let si_to_model = (1.0 / self.current_distance_unit.to_meters()) as f32;

            // Animated world positions + per-node displacement magnitude.
            let mut node_positions: Vec<[f32; 3]> = Vec::new();
            let mut magnitudes:     Vec<f32>        = Vec::new();

            for row in &self.rows {
                let Some([bx, by, bz]) = row_position(row) else { continue };

                let sample_ch = |ch_idx: usize| -> f32 {
                    if ch_idx == 0 { return 0.0; }
                    let qname = match self.channel_names.get(ch_idx - 1) {
                        Some(n) => n,
                        None => return 0.0,
                    };

                    // FFT filtering active?
                    if self.fft_state.active && self.fft_state.filter_mode != FilterMode::None {
                        if self.fft_state.filter_mode == FilterMode::SingleFreq
                            && self.fft_state.anim_mode == AnimMode::FreqBased
                            && self.fft_state.single_freq > 0.0
                        {
                            return sample_freq_based(
                                &self.datasets, qname,
                                self.fft_state.single_freq, t,
                            );
                        }
                        // Time-based filtered
                        if !self.fft_state.filtered_displacements.is_empty() {
                            // We need the time axis — find the dataset
                            if let Some((file, _ch)) = qname.split_once("::") {
                                if let Some(ds) = self.datasets.iter().find(|d| d.name == file) {
                                    return sample_filtered(
                                        &self.fft_state.filtered_displacements,
                                        &ds.time, qname, t,
                                    );
                                }
                            }
                        }
                    }
                    // Normal path
                    sample_qualified(&self.datasets, qname, t)
                };

                let ddx = sample_ch(row.dx);
                let ddy = sample_ch(row.dy);
                let ddz = sample_ch(row.dz);

                // Transform displacement from local CSYS to global coordinates.
                // local_csys rows are the local X, Y, Z axes in world space.
                let m = row.local_csys;
                let gx = m[0][0] * ddx + m[1][0] * ddy + m[2][0] * ddz;
                let gy = m[0][1] * ddx + m[1][1] * ddy + m[2][1] * ddz;
                let gz = m[0][2] * ddx + m[1][2] * ddy + m[2][2] * ddz;

                node_positions.push([
                    bx + gx * scale * si_to_model,
                    by + gy * scale * si_to_model,
                    bz + gz * scale * si_to_model,
                ]);
                magnitudes.push((ddx * ddx + ddy * ddy + ddz * ddz).sqrt());
            }

            // Per-node colour and sphere size scale for vis modes.
            let max_mag = self.global_max_disp().max(1e-12);

            let node_colors: Vec<[f32; 4]> = self.rows.iter().zip(magnitudes.iter()).map(|(row, &m)| {
                // Per-node override takes priority, then contour, then global
                if let Some(c) = row.color_override {
                    [c[0], c[1], c[2], 1.0]
                } else {
                    match self.vis_mode {
                        VisMode::ContourColor | VisMode::ContourAndSize
                            => self.palette.sample(m / max_mag, self.reverse_pal),
                        _   => {
                            let c = self.global_node_color;
                            [c[0], c[1], c[2], 1.0]
                        }
                    }
                }
            }).collect();

            let node_scales: Vec<f32> = magnitudes.iter().map(|&m| {
                match self.vis_mode {
                    VisMode::SizeScale | VisMode::ContourAndSize
                        => 1.0 + (self.size_vis_scale - 1.0) * (m / max_mag),
                    _   => 1.0,
                }
            }).collect();

            // Build edge segments using animated positions.
            let id_to_pos: std::collections::HashMap<&str, [f32; 3]> = self
                .rows
                .iter()
                .zip(node_positions.iter())
                .filter(|(r, _)| !r.id.is_empty())
                .map(|(r, pos)| (r.id.as_str(), *pos))
                .collect();

            // Build edge ID→color lookup for contour-on-edges
            let id_to_color: std::collections::HashMap<&str, [f32; 4]> = self
                .rows
                .iter()
                .zip(node_colors.iter())
                .filter(|(r, _)| !r.id.is_empty())
                .map(|(r, c)| (r.id.as_str(), *c))
                .collect();

            let global_ec: [f32; 4] = {
                let c = self.global_edge_color;
                [c[0], c[1], c[2], 1.0]
            };

            let mut edge_segments: Vec<([f32; 3], [f32; 3])> = Vec::new();
            let mut edge_colors:   Vec<([f32; 4], [f32; 4])> = Vec::new();
            let mut edge_orig_idx: Vec<usize> = Vec::new(); // maps edge_segments index -> self.edges index

            for (ei, edge) in self.edges.iter().enumerate() {
                if edge.from.is_empty() || edge.to.is_empty() { continue; }
                let Some(&a) = id_to_pos.get(edge.from.as_str()) else { continue };
                let Some(&b) = id_to_pos.get(edge.to.as_str()) else { continue };
                edge_segments.push((a, b));
                edge_orig_idx.push(ei);

                // Determine color for this edge
                if let Some(c) = edge.color_override {
                    let ec = [c[0], c[1], c[2], 1.0];
                    edge_colors.push((ec, ec));
                } else if self.edge_contour && self.vis_mode != VisMode::None {
                    let ca = id_to_color.get(edge.from.as_str()).copied().unwrap_or(global_ec);
                    let cb = id_to_color.get(edge.to.as_str()).copied().unwrap_or(global_ec);
                    edge_colors.push((ca, cb));
                } else {
                    edge_colors.push((global_ec, global_ec));
                }
            }

            // Collect per-node selection flags
            let selected_nodes: Vec<bool> = self.rows.iter()
                .filter(|r| row_position(r).is_some())
                .map(|r| r.selected)
                .collect();

            // Collect local CSYS per node (parallel with node_positions)
            let node_csys: Vec<[[f32; 3]; 3]> = self.rows.iter()
                .filter(|r| row_position(r).is_some())
                .map(|r| r.local_csys)
                .collect();

            // Collect per-node CSYS-visible flags
            let node_csys_visible: Vec<bool> = self.rows.iter()
                .filter(|r| row_position(r).is_some())
                .map(|r| r.show_csys_axes)
                .collect();

            // ── Details / Select mode toolbar above the viewport ────────────────
            ui.horizontal(|ui| {
                // Tool buttons (toggle on/off, only one at a time)
                let detail_on = self.interaction_tool == InteractionTool::Details;
                if ui.add(egui::SelectableLabel::new(detail_on, "🔍  Details"))
                    .on_hover_text("Click on an object to view its properties")
                    .clicked()
                {
                    self.interaction_tool = if detail_on { InteractionTool::None } else { InteractionTool::Details };
                }
                let select_on = self.interaction_tool == InteractionTool::Select;
                if ui.add(egui::SelectableLabel::new(select_on, format!("{}  Select", egui_phosphor::regular::SELECTION_PLUS)))
                    .on_hover_text("Drag rect or Ctrl+click to select objects")
                    .clicked()
                {
                    if select_on {
                        // Turning off Select: clear all selections
                        for r in &mut self.rows { r.selected = false; }
                        for g in &mut self.glyphs { g.selected = false; }
                    }
                    self.interaction_tool = if select_on { InteractionTool::None } else { InteractionTool::Select };
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                // Selection filter buttons (always visible when a tool is active)
                if self.interaction_tool != InteractionTool::None {
                    ui.label("Filter:");
                    for (filter, label) in [
                        (SelectionFilter::Node,  format!("{} Node", egui_phosphor::regular::HEXAGON)),
                        (SelectionFilter::Edge,  format!("{} Edge", egui_phosphor::regular::LINE_SEGMENT)),
                        (SelectionFilter::Glyph, format!("{} Glyph", egui_phosphor::regular::DIAMOND)),
                    ] {
                        let active = self.selection_filter == filter;
                        if ui.add(egui::SelectableLabel::new(active, label)).clicked() {
                            self.selection_filter = filter;
                        }
                    }
                }

                if select_on {
                    ui.add_space(8.0);
                    let n = match self.selection_filter {
                        SelectionFilter::Node  => self.rows.iter().filter(|r| r.selected).count(),
                        SelectionFilter::Glyph => self.glyphs.iter().filter(|g| g.selected).count(),
                        SelectionFilter::Edge  => 0,
                    };
                    ui.label(
                        egui::RichText::new(format!("{} selected", n))
                            .color(egui::Color32::from_rgb(140, 200, 140))
                            .size(11.0),
                    );
                    if ui.small_button("Clear").clicked() {
                        match self.selection_filter {
                            SelectionFilter::Node  => { for r in &mut self.rows { r.selected = false; } }
                            SelectionFilter::Glyph => { for g in &mut self.glyphs { g.selected = false; } }
                            SelectionFilter::Edge  => {}
                        }
                    }
                }
            });

            // ── Compute glyph positions ──────────────────────────────────
            let glyph_data: Vec<([f32; 3], GlyphShape, [f32; 4], f32, [f32; 3], f32)> = self.glyphs.iter().map(|g| {
                // Average the displaced positions of scoped nodes
                let mut sum = [0.0f32; 3];
                let mut count = 0usize;
                for nid in &g.node_ids {
                    if let Some(ni) = self.rows.iter().position(|r| &r.id == nid) {
                        if let Some(np) = node_positions.get(ni) {
                            sum[0] += np[0];
                            sum[1] += np[1];
                            sum[2] += np[2];
                            count += 1;
                        }
                    }
                }
                let base = if count > 0 {
                    let c = count as f32;
                    [sum[0] / c, sum[1] / c, sum[2] / c]
                } else {
                    [0.0, 0.0, 0.0]
                };
                let pos = [
                    base[0] + g.position_offset[0],
                    base[1] + g.position_offset[1],
                    base[2] + g.position_offset[2],
                ];
                (pos, g.shape.clone(), [g.color[0], g.color[1], g.color[2], 1.0], g.size, g.stretch, g.tube_ratio)
            }).collect();
            let glyph_sel: Vec<bool> = self.glyphs.iter().map(|g| g.selected).collect();

            // ── Compute mesh surfaces ────────────────────────────────────────
            let mesh_surfaces: Vec<MeshRenderData> = self.meshes.iter().map(|m| {
                // Resolve node IDs to displaced 3D positions
                let mut pts3d: Vec<[f32; 3]> = Vec::new();
                for nid in &m.node_ids {
                    if let Some(ni) = self.rows.iter().position(|r| &r.id == nid) {
                        if let Some(&np) = node_positions.get(ni) {
                            pts3d.push(np);
                        }
                    }
                }

                if pts3d.len() < 3 {
                    return MeshRenderData { verts: Vec::new(), indices: Vec::new() };
                }

                // Compute centroid
                let n = pts3d.len() as f32;
                let cx = pts3d.iter().map(|p| p[0]).sum::<f32>() / n;
                let cy = pts3d.iter().map(|p| p[1]).sum::<f32>() / n;
                let cz = pts3d.iter().map(|p| p[2]).sum::<f32>() / n;

                // Compute 3x3 covariance matrix
                let mut cov = [[0.0f32; 3]; 3];
                for p in &pts3d {
                    let dx = p[0] - cx;
                    let dy = p[1] - cy;
                    let dz = p[2] - cz;
                    cov[0][0] += dx * dx;
                    cov[0][1] += dx * dy;
                    cov[0][2] += dx * dz;
                    cov[1][1] += dy * dy;
                    cov[1][2] += dy * dz;
                    cov[2][2] += dz * dz;
                }
                cov[1][0] = cov[0][1];
                cov[2][0] = cov[0][2];
                cov[2][1] = cov[1][2];

                // Power iteration to find the two largest eigenvectors
                // (the plane axes). Start with a random-ish vector.
                let normalize3 = |v: [f32; 3]| -> [f32; 3] {
                    let l = (v[0]*v[0] + v[1]*v[1] + v[2]*v[2]).sqrt();
                    if l < 1e-12 { return [1.0, 0.0, 0.0]; }
                    [v[0]/l, v[1]/l, v[2]/l]
                };
                let mat_mul = |m: &[[f32;3];3], v: [f32;3]| -> [f32;3] {
                    [
                        m[0][0]*v[0] + m[0][1]*v[1] + m[0][2]*v[2],
                        m[1][0]*v[0] + m[1][1]*v[1] + m[1][2]*v[2],
                        m[2][0]*v[0] + m[2][1]*v[1] + m[2][2]*v[2],
                    ]
                };
                let dot3 = |a: [f32;3], b: [f32;3]| -> f32 {
                    a[0]*b[0] + a[1]*b[1] + a[2]*b[2]
                };

                // Find first eigenvector (largest eigenvalue) via power iteration
                let mut e1 = [0.6, 0.7, 0.4];
                for _ in 0..30 {
                    e1 = normalize3(mat_mul(&cov, e1));
                }

                // Deflate: cov2 = cov - lambda1 * e1*e1^T
                let lambda1 = dot3(mat_mul(&cov, e1), e1);
                let mut cov2 = cov;
                for i in 0..3 {
                    for j in 0..3 {
                        cov2[i][j] -= lambda1 * e1[i] * e1[j];
                    }
                }

                // Find second eigenvector
                let mut e2 = [0.4, 0.6, 0.7];
                for _ in 0..30 {
                    e2 = normalize3(mat_mul(&cov2, e2));
                }
                // Ensure orthogonal to e1
                let d = dot3(e2, e1);
                e2 = normalize3([e2[0] - d*e1[0], e2[1] - d*e1[1], e2[2] - d*e1[2]]);

                // Project 3D points onto e1, e2 to get 2D coords
                let pts2d: Vec<[f64; 2]> = pts3d.iter().map(|p| {
                    let v = [p[0] - cx, p[1] - cy, p[2] - cz];
                    [dot3(v, e1) as f64, dot3(v, e2) as f64]
                }).collect();

                // Bowyer-Watson Delaunay triangulation
                let tri_indices = delaunay_2d(&pts2d);

                // Build vertex + index data
                let color = [m.color[0], m.color[1], m.color[2], m.opacity];
                let verts: Vec<[f32; 7]> = pts3d.iter().map(|p| {
                    [p[0], p[1], p[2], color[0], color[1], color[2], color[3]]
                }).collect();

                MeshRenderData { verts, indices: tri_indices }
            }).collect();

            // ── Build wireframe edges (undeformed positions) ────────────
            let wireframe_edges: Vec<([f32; 3], [f32; 3])> = if self.show_wireframe {
                // Build base-position lookup (no displacement)
                let base_pos: std::collections::HashMap<&str, [f32; 3]> = self
                    .rows.iter()
                    .filter_map(|r| {
                        row_position(r).map(|p| (r.id.as_str(), p))
                    })
                    .collect();
                self.edges.iter().filter_map(|e| {
                    if e.from.is_empty() || e.to.is_empty() { return None; }
                    let a = base_pos.get(e.from.as_str())?;
                    let b = base_pos.get(e.to.as_str())?;
                    Some((*a, *b))
                }).collect()
            } else {
                Vec::new()
            };

            // Compute actual world-space node size from normalised fraction
            let actual_node_size = self.node_size * self.bounding_diag()
                * (self.prefs.max_node_size_pct / 100.0);
            let actual_edge_thickness = self.edge_thickness * actual_node_size;

            // Remaining area: 3D viewport
            let vp_resp = show_viewport(
                ui,
                &mut self.viewport,
                &node_positions,
                &edge_segments,
                &edge_colors,
                &node_colors,
                &node_scales,
                &node_csys,
                &node_csys_visible,
                &selected_nodes,
                actual_node_size,
                self.prefs.local_csys_scale_pct / 100.0,
                self.interaction_tool == InteractionTool::Select,
                actual_edge_thickness,
                self.prefs.viewport_bg_color,
                self.prefs.middle_button_orbit,
                &glyph_data,
                &glyph_sel,
                mesh_surfaces,
                &wireframe_edges,
                self.current_distance_unit.label(),
                &self.current_distance_unit,
            );

            // ── Handle unit change from viewport context menu ─────────────
            if let Some(new_unit) = vp_resp.unit_change.clone() {
                self.convert_units(&new_unit);
            }

            // ── Node picking / selection ──────────────────────────────────────
            let vp_mat  = vp_resp.vp;
            let rect    = vp_resp.rect;
            let half_w  = rect.width()  * 0.5;
            let half_h  = rect.height() * 0.5;

            // Feed viewport rect to export state for screenshot cropping
            self.export_video.viewport_rect = Some(rect);

            let screen_positions: Vec<Option<egui::Pos2>> = node_positions.iter()
                .map(|&[px, py, pz]| {
                    let clip = vp_mat * glam::Vec4::new(px, py, pz, 1.0);
                    if clip.w <= 0.0 { return None; }
                    let sx = rect.left() + half_w * (1.0 + clip.x / clip.w);
                    let sy = rect.top()  + half_h * (1.0 - clip.y / clip.w);
                    Some(egui::pos2(sx, sy))
                })
                .collect();

            let pick_nearest = |pos: egui::Pos2, threshold: f32| -> Option<usize> {
                let mut best = None;
                let mut best_d = threshold;
                for (i, sp) in screen_positions.iter().enumerate() {
                    if let Some(sp) = sp {
                        let d = (*sp - pos).length();
                        if d < best_d { best_d = d; best = Some(i); }
                    }
                }
                best
            };

            // ── Picking based on tool + filter ─────────────────────────

            // Always compute glyph screen positions for picking
            let glyph_screen: Vec<Option<egui::Pos2>> = glyph_data.iter()
                .map(|&(pos, _, _, _, _, _)| {
                    let clip = vp_mat * glam::Vec4::new(pos[0], pos[1], pos[2], 1.0);
                    if clip.w <= 0.0 { return None; }
                    let sx = rect.left() + half_w * (1.0 + clip.x / clip.w);
                    let sy = rect.top()  + half_h * (1.0 - clip.y / clip.w);
                    Some(egui::pos2(sx, sy))
                })
                .collect();

            // Edge midpoints for edge picking
            let edge_screen: Vec<Option<egui::Pos2>> = edge_segments.iter()
                .map(|&([ax,ay,az], [bx,by,bz])| {
                    let mx = (ax + bx) * 0.5;
                    let my = (ay + by) * 0.5;
                    let mz = (az + bz) * 0.5;
                    let clip = vp_mat * glam::Vec4::new(mx, my, mz, 1.0);
                    if clip.w <= 0.0 { return None; }
                    let sx = rect.left() + half_w * (1.0 + clip.x / clip.w);
                    let sy = rect.top()  + half_h * (1.0 - clip.y / clip.w);
                    Some(egui::pos2(sx, sy))
                })
                .collect();

            let pick_nearest_glyph = |pos: egui::Pos2, threshold: f32| -> Option<usize> {
                let mut best = None;
                let mut best_d = threshold;
                for (i, sp) in glyph_screen.iter().enumerate() {
                    if let Some(sp) = sp {
                        let d = (*sp - pos).length();
                        if d < best_d { best_d = d; best = Some(i); }
                    }
                }
                best
            };

            let pick_nearest_edge = |pos: egui::Pos2, threshold: f32| -> Option<usize> {
                let mut best = None;
                let mut best_d = threshold;
                for (i, sp) in edge_screen.iter().enumerate() {
                    if let Some(sp) = sp {
                        let d = (*sp - pos).length();
                        if d < best_d { best_d = d; best = Some(i); }
                    }
                }
                // Remap from edge_segments index to self.edges index
                best.and_then(|i| edge_orig_idx.get(i).copied())
            };

            match self.interaction_tool {
                InteractionTool::Details => {
                    if let Some(cpos) = vp_resp.clicked_pos {
                        match self.selection_filter {
                            SelectionFilter::Node => {
                                self.selected_glyph = None;
                                self.selected_node = pick_nearest(cpos, 15.0);
                            }
                            SelectionFilter::Glyph => {
                                self.selected_node = None;
                                self.selected_glyph = pick_nearest_glyph(cpos, 15.0);
                            }
                            SelectionFilter::Edge => {
                                self.selected_node = None;
                                self.selected_glyph = None;
                                self.selected_edge = pick_nearest_edge(cpos, 20.0);
                            }
                        }
                    }
                }
                InteractionTool::Select => {
                    match self.selection_filter {
                        SelectionFilter::Node => {
                            if let Some(sel_rect) = vp_resp.rect_selection {
                                let ctrl = ctx.input(|i| i.modifiers.ctrl);
                                if !ctrl { for r in &mut self.rows { r.selected = false; } }
                                let mut pi = 0usize;
                                for r in &mut self.rows {
                                    if row_position(r).is_none() { continue; }
                                    if let Some(sp) = screen_positions.get(pi).and_then(|x| *x) {
                                        if sel_rect.contains(sp) { r.selected = true; }
                                    }
                                    pi += 1;
                                }
                            }
                            if let Some(cpos) = vp_resp.ctrl_clicked_pos {
                                if let Some(idx) = pick_nearest(cpos, 20.0) {
                                    let mut pi = 0usize;
                                    for r in &mut self.rows {
                                        if row_position(r).is_none() { continue; }
                                        if pi == idx { r.selected = !r.selected; break; }
                                        pi += 1;
                                    }
                                }
                            }
                            if let Some(cpos) = vp_resp.clicked_pos {
                                for r in &mut self.rows { r.selected = false; }
                                if let Some(idx) = pick_nearest(cpos, 20.0) {
                                    let mut pi = 0usize;
                                    for r in &mut self.rows {
                                        if row_position(r).is_none() { continue; }
                                        if pi == idx { r.selected = true; break; }
                                        pi += 1;
                                    }
                                }
                            }
                        }
                        SelectionFilter::Glyph => {
                            if let Some(sel_rect) = vp_resp.rect_selection {
                                let ctrl = ctx.input(|i| i.modifiers.ctrl);
                                if !ctrl { for g in &mut self.glyphs { g.selected = false; } }
                                for (gi, sp) in glyph_screen.iter().enumerate() {
                                    if let Some(sp) = sp {
                                        if sel_rect.contains(*sp) {
                                            self.glyphs[gi].selected = true;
                                        }
                                    }
                                }
                            }
                            if let Some(cpos) = vp_resp.ctrl_clicked_pos {
                                if let Some(gi) = pick_nearest_glyph(cpos, 20.0) {
                                    self.glyphs[gi].selected = !self.glyphs[gi].selected;
                                }
                            }
                            if let Some(cpos) = vp_resp.clicked_pos {
                                for g in &mut self.glyphs { g.selected = false; }
                                if let Some(gi) = pick_nearest_glyph(cpos, 20.0) {
                                    self.glyphs[gi].selected = true;
                                }
                            }
                        }
                        SelectionFilter::Edge => {
                            // Edge selection: basic click-to-select
                            // (no rubber-band for now since edges are lines, not points)
                        }
                    }
                }
                InteractionTool::None => {
                    // No picking in orbit-only mode
                }
            }
        });

        // ── Node properties window ──────────────────────────────────────
        // Capture "Edit CSYS" click outside the borrow of self.rows.
        let mut open_csys_for: Option<usize> = None;

        if let Some(ni) = self.selected_node {
            if ni < self.rows.len() {
                let mut open = true;
                egui::Window::new(format!("Node: {}", self.rows[ni].id))
                    .open(&mut open)
                    .default_width(310.0)
                    .resizable(false)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::Grid::new("node_props")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                let row = &mut self.rows[ni];

                                ui.label("ID:");
                                ui.add(egui::TextEdit::singleline(&mut row.id).desired_width(100.0));
                                ui.end_row();

                                ui.label("X:");
                                ui.add(egui::TextEdit::singleline(&mut row.x).desired_width(80.0));
                                ui.end_row();

                                ui.label("Y:");
                                ui.add(egui::TextEdit::singleline(&mut row.y).desired_width(80.0));
                                ui.end_row();

                                ui.label("Z:");
                                ui.add(egui::TextEdit::singleline(&mut row.z).desired_width(80.0));
                                ui.end_row();

                                // Channel mappings
                                let ch_names = &self.channel_names;
                                let ch_label = |idx: usize| -> &str {
                                    if idx == 0 { "—" }
                                    else { ch_names.get(idx - 1).map(|s| s.as_str()).unwrap_or("?") }
                                };

                                for (label, ch) in [
                                    ("DX:", &mut row.dx),
                                    ("DY:", &mut row.dy),
                                    ("DZ:", &mut row.dz),
                                ] {
                                    ui.label(label);
                                    egui::ComboBox::from_id_salt(format!("ch_{}", label))
                                        .selected_text(ch_label(*ch))
                                        .width(150.0)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(ch, 0, "—");
                                            for (ci, name) in ch_names.iter().enumerate() {
                                                ui.selectable_value(ch, ci + 1, name);
                                            }
                                        });
                                    ui.end_row();
                                }

                                // Color override
                                ui.label("Color:");
                                ui.horizontal(|ui| {
                                    let mut use_custom = row.color_override.is_some();
                                    if ui.checkbox(&mut use_custom, "").changed() {
                                        if use_custom {
                                            row.color_override = Some(row.stored_color);
                                        } else {
                                            row.color_override = None;
                                        }
                                    }
                                    if use_custom {
                                        if ui.color_edit_button_rgb(&mut row.stored_color).changed() {
                                            row.color_override = Some(row.stored_color);
                                        }
                                    } else {
                                        ui.label("(global)");
                                    }
                                });
                                ui.end_row();
                            });

                        // ── Local CSYS ──────────────────────────────────────
                        ui.separator();
                        ui.horizontal(|ui| {
                            ui.strong("Local CSYS");
                            ui.checkbox(&mut self.rows[ni].show_csys_axes, "Show axes");
                            if ui.button("✏  Edit CSYS…").clicked() {
                                open_csys_for = Some(ni);
                            }
                        });

                        let m = self.rows[ni].local_csys;
                        if m == identity_mat3() {
                            ui.label(
                                egui::RichText::new("  Identity (aligned with global)")
                                    .color(egui::Color32::from_rgb(100, 100, 120))
                                    .italics()
                                    .size(11.0),
                            );
                        } else {
                            egui::Grid::new("node_csys_grid")
                                .num_columns(4)
                                .spacing([4.0, 2.0])
                                .show(ui, |ui| {
                                    let axis_labels = ["X′", "Y′", "Z′"];
                                    let axis_colors = [
                                        egui::Color32::from_rgb(220, 80, 80),
                                        egui::Color32::from_rgb(80, 200, 80),
                                        egui::Color32::from_rgb(80, 130, 220),
                                    ];
                                    for (col_idx, (lbl, col)) in axis_labels.iter().zip(m.iter()).enumerate() {
                                        ui.label(egui::RichText::new(*lbl).color(axis_colors[col_idx]).size(11.0));
                                        for &v in col.iter() {
                                            ui.label(
                                                egui::RichText::new(format!("{:+.3}", v))
                                                    .monospace()
                                                    .size(10.0),
                                            );
                                        }
                                        ui.end_row();
                                    }
                                });
                        }
                    });
                if !open {
                    self.selected_node = None;
                }
            } else {
                self.selected_node = None;
            }
        }

        // ── Glyph detail popup ───────────────────────────────────────────
        if let Some(gi) = self.selected_glyph {
            if gi < self.glyphs.len() {
                let mut open = true;
                let title = if self.glyphs[gi].id.is_empty() {
                    format!("Glyph #{}", gi + 1)
                } else {
                    format!("Glyph: {}", self.glyphs[gi].id)
                };
                egui::Window::new(title)
                    .open(&mut open)
                    .default_width(310.0)
                    .resizable(false)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::Grid::new("glyph_props")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                let g = &mut self.glyphs[gi];

                                ui.label("ID:");
                                ui.add(egui::TextEdit::singleline(&mut g.id).desired_width(100.0));
                                ui.end_row();

                                ui.label("Shape:");
                                egui::ComboBox::from_id_salt("glyph_detail_shape")
                                    .selected_text(g.shape.label())
                                    .show_ui(ui, |ui| {
                                        for s in GlyphShape::ALL {
                                            ui.selectable_value(&mut g.shape, s.clone(), s.label());
                                        }
                                    });
                                ui.end_row();

                                ui.label("Nodes:");
                                ui.label(if g.node_ids.is_empty() { "(none)".to_string() } else { g.node_ids.join(", ") });
                                ui.end_row();

                                ui.label("Size:");
                                ui.add(egui::DragValue::new(&mut g.size).speed(0.005).range(0.001..=100.0));
                                ui.end_row();

                                ui.label("Color:");
                                ui.color_edit_button_rgb(&mut g.color);
                                ui.end_row();
                            });

                        // ── Shape-specific deformation controls ──────────────
                        ui.separator();
                        ui.strong("Shape Deformation");
                        egui::Grid::new("glyph_deform")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                let g = &mut self.glyphs[gi];

                                let (lx, ly, lz) = match g.shape {
                                    GlyphShape::Cube     => ("Width (X):",  "Height (Y):", "Depth (Z):"),
                                    GlyphShape::Sphere   => ("Stretch X:", "Stretch Y:",  "Stretch Z:"),
                                    GlyphShape::Cylinder => ("Radius X:",  "Height (Y):", "Radius Z:"),
                                    GlyphShape::Torus    => ("Stretch X:", "Stretch Y:",  "Stretch Z:"),
                                };

                                ui.label(lx);
                                ui.add(egui::DragValue::new(&mut g.stretch[0]).speed(0.01).range(0.01..=100.0));
                                ui.end_row();

                                ui.label(ly);
                                ui.add(egui::DragValue::new(&mut g.stretch[1]).speed(0.01).range(0.01..=100.0));
                                ui.end_row();

                                ui.label(lz);
                                ui.add(egui::DragValue::new(&mut g.stretch[2]).speed(0.01).range(0.01..=100.0));
                                ui.end_row();

                                if g.shape == GlyphShape::Torus {
                                    ui.label("Tube Ratio:");
                                    ui.add(egui::DragValue::new(&mut g.tube_ratio).speed(0.005).range(0.01..=0.99));
                                    ui.end_row();
                                }
                            });

                        // ── Offset controls ──────────────────────────────────
                        ui.separator();
                        ui.strong("Position Offset");
                        egui::Grid::new("glyph_offset")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                let g = &mut self.glyphs[gi];

                                ui.label("Offset X:");
                                ui.add(egui::DragValue::new(&mut g.position_offset[0]).speed(0.01));
                                ui.end_row();
                                ui.label("Offset Y:");
                                ui.add(egui::DragValue::new(&mut g.position_offset[1]).speed(0.01));
                                ui.end_row();
                                ui.label("Offset Z:");
                                ui.add(egui::DragValue::new(&mut g.position_offset[2]).speed(0.01));
                                ui.end_row();
                            });
                    });
                if !open { self.selected_glyph = None; }
            } else {
                self.selected_glyph = None;
            }
        }

        // ── Edge detail popup ───────────────────────────────────────────────
        if let Some(ei) = self.selected_edge {
            if ei < self.edges.len() {
                let mut open = true;
                egui::Window::new(format!("Edge #{}", ei + 1))
                    .open(&mut open)
                    .default_width(310.0)
                    .resizable(false)
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::Grid::new("edge_props")
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .show(ui, |ui| {
                                let e = &mut self.edges[ei];

                                ui.label("From Node:");
                                ui.add(egui::TextEdit::singleline(&mut e.from).desired_width(100.0));
                                ui.end_row();

                                ui.label("To Node:");
                                ui.add(egui::TextEdit::singleline(&mut e.to).desired_width(100.0));
                                ui.end_row();

                                // Color override
                                ui.label("Color:");
                                ui.horizontal(|ui| {
                                    let mut use_custom = e.color_override.is_some();
                                    let mut c = e.color_override.unwrap_or(self.global_edge_color);
                                    if ui.checkbox(&mut use_custom, "").changed() {
                                        e.color_override = if use_custom { Some(c) } else { None };
                                    }
                                    if use_custom {
                                        if ui.color_edit_button_rgb(&mut c).changed() {
                                            e.color_override = Some(c);
                                        }
                                    } else {
                                        ui.label("(global)");
                                    }
                                });
                                ui.end_row();

                                // Thickness override
                                ui.label("Thickness:");
                                ui.horizontal(|ui| {
                                    let mut use_custom = e.thickness_override.is_some();
                                    let mut t = e.thickness_override.unwrap_or(self.edge_thickness);
                                    if ui.checkbox(&mut use_custom, "").changed() {
                                        e.thickness_override = if use_custom { Some(t) } else { None };
                                    }
                                    if use_custom {
                                        if ui.add(egui::DragValue::new(&mut t).speed(0.005).range(0.001..=10.0)).changed() {
                                            e.thickness_override = Some(t);
                                        }
                                    } else {
                                        ui.label("(global)");
                                    }
                                });
                                ui.end_row();
                            });
                    });
                if !open { self.selected_edge = None; }
            } else {
                self.selected_edge = None;
            }
        }

        // Open CSYS builder from the node popup "Edit CSYS…" button
        if let Some(ni) = open_csys_for {
            let row = &self.rows[ni];
            self.csys_builder.load_with_ops(row.local_csys_base, row.local_csys_ops.clone());
            self.csys_builder_target = CsysTarget::Node(ni);
            self.show_csys_builder = true;
        }

        // Open CSYS builder from Manager "✎" Edit button
        if let Some((name, base_mat, ops)) = self.csys_mgr_edit.take() {
            self.csys_builder.load_with_ops(base_mat, ops);
            self.csys_builder_target = CsysTarget::Manager(name);
            self.show_csys_builder = true;
        }

        // Apply CSYS result if the builder produced one (single-node apply or manager edit)
        if let Some((mat, base_mat, ops)) = self.csys_apply_result.take() {
            match &self.csys_builder_target {
                CsysTarget::Node(ni) => {
                    if *ni < self.rows.len() {
                        self.rows[*ni].local_csys = mat;
                        self.rows[*ni].local_csys_base = base_mat;
                        self.rows[*ni].local_csys_ops = ops;
                    }
                }
                CsysTarget::Manager(name) => {
                    self.csys_manager.add_or_replace(name.clone(), mat, base_mat, ops);
                }
                CsysTarget::None => {}
            }
        }

        // Save-to-Manager result from the builder
        if let Some((name, mat, base_mat, ops)) = self.csys_save_mgr_result.take() {
            self.csys_manager.add_or_replace(name, mat, base_mat, ops);
        }

        // Apply from manager to all selected nodes
        if let Some((mat, base_mat, ops)) = self.csys_mgr_apply.take() {
            for r in &mut self.rows {
                if r.selected {
                    r.local_csys = mat;
                    r.local_csys_base = base_mat;
                    r.local_csys_ops = ops.clone();
                }
            }
        }

        // ── Floating windows ──────────────────────────────────────────────
        show_options_window(ctx, &mut self.show_options, &mut self.prefs);
        show_views_window(ctx, &mut self.show_views, &mut self.saved_views, &mut self.viewport);
        show_time_plot_window(
            ctx,
            &mut self.show_time_plot,
            &mut self.time_plot,
            &self.datasets,
            &self.channel_names,
            self.anim.time,
        );

        let target_label = match &self.csys_builder_target {
            CsysTarget::Node(ni) => self.rows.get(*ni).map(|r| r.id.as_str()),
            CsysTarget::Manager(name) => Some(name.as_str()),
            CsysTarget::None => None,
        };
        show_csys_builder_window(
            ctx,
            &mut self.show_csys_builder,
            &mut self.csys_builder,
            target_label,
            &mut self.csys_apply_result,
            &mut self.csys_save_mgr_result,
        );

        // ── Create Nodes dialog ───────────────────────────────────────────
        if let Some(new_rows) = show_create_nodes_window(
            ctx,
            &mut self.show_create_nodes,
            &mut self.create_nodes,
            &self.rows,
            &self.csys_manager,
        ) {
            self.rows.extend(new_rows);
        }

        // ── Export Animation dialog + export loop ─────────────────────────
        let duration = max_duration(&self.datasets, &self.channel_names, &self.rows);
        let export_action = show_export_video_window(
            ctx,
            &mut self.show_export_video,
            &mut self.export_video,
            duration,
        );
        match export_action {
            ExportAction::StartCapture { total_frames } => {
                // Save current animation state
                self.export_video.saved_time = self.anim.time;
                self.export_video.saved_playing = self.anim.playing;
                self.anim.playing = false;
                self.anim.time = 0.0;
                self.export_video.phase = ExportPhase::Capturing {
                    frame: 0, total: total_frames, waiting: false,
                };
                ctx.request_repaint();
            }
            ExportAction::Cancel => {
                self.anim.time = self.export_video.saved_time;
                self.anim.playing = self.export_video.saved_playing;
                self.export_video.phase = ExportPhase::Idle;
            }
            ExportAction::None => {}
        }

        // Drive capture state machine
        match &self.export_video.phase {
            ExportPhase::Capturing { frame, total, waiting: false } => {
                // Set animation time for this frame and request screenshot
                let fps = self.export_video.fps.max(1.0) as f64;
                self.anim.time = *frame as f64 / fps;
                // We need one render pass with the correct time before capturing.
                // Request screenshot — result arrives next frame.
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.export_video.phase = ExportPhase::Capturing {
                    frame: *frame, total: *total, waiting: true,
                };
                ctx.request_repaint();
            }
            ExportPhase::Encoding => {
                run_ffmpeg_encode(&mut self.export_video);
                // Restore animation state
                self.anim.time = self.export_video.saved_time;
                self.anim.playing = self.export_video.saved_playing;
            }
            _ => {}
        }

        // Check for screenshot events (captures arrive one frame late)
        let mut got_screenshot = false;
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    let done = process_screenshot(&mut self.export_video, image);
                    got_screenshot = true;
                    if done && self.export_video.phase != ExportPhase::Encoding {
                        // Restore animation state (PNG sequence done)
                        self.anim.time = self.export_video.saved_time;
                        self.anim.playing = self.export_video.saved_playing;
                    }
                }
            }
        });
        if got_screenshot {
            ctx.request_repaint();
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Bowyer-Watson Delaunay triangulation (2D)
// ─────────────────────────────────────────────────────────────────────────────

/// 2D Bowyer-Watson Delaunay triangulation.
/// Returns a flat list of triangle vertex indices (groups of 3) referencing the
/// input `pts` slice.  Points that are nearly coincident are deduplicated.
fn delaunay_2d(pts: &[[f64; 2]]) -> Vec<u32> {
    if pts.len() < 3 { return Vec::new(); }

    // Bounding box
    let (mut min_x, mut min_y) = (f64::MAX, f64::MAX);
    let (mut max_x, mut max_y) = (f64::MIN, f64::MIN);
    for p in pts {
        min_x = min_x.min(p[0]); min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]); max_y = max_y.max(p[1]);
    }
    let dx = (max_x - min_x).max(1e-10);
    let dy = (max_y - min_y).max(1e-10);
    let d_max = dx.max(dy);
    let mid_x = (min_x + max_x) * 0.5;
    let mid_y = (min_y + max_y) * 0.5;

    // Super-triangle vertices (indices: n, n+1, n+2)
    let n = pts.len();
    let mut all_pts: Vec<[f64; 2]> = pts.to_vec();
    all_pts.push([mid_x - 20.0 * d_max, mid_y - d_max]);
    all_pts.push([mid_x + 20.0 * d_max, mid_y - d_max]);
    all_pts.push([mid_x, mid_y + 20.0 * d_max]);

    // Triangle: (a, b, c) indices into all_pts
    let mut triangles: Vec<[usize; 3]> = vec![[n, n + 1, n + 2]];

    // Incrementally insert each point
    for pi in 0..n {
        let p = all_pts[pi];
        let mut bad_triangles: Vec<usize> = Vec::new();

        for (ti, &[a, b, c]) in triangles.iter().enumerate() {
            if in_circumcircle(&all_pts[a], &all_pts[b], &all_pts[c], &p) {
                bad_triangles.push(ti);
            }
        }

        // Find the boundary polygon (edges that are not shared by two bad triangles)
        let mut polygon: Vec<[usize; 2]> = Vec::new();
        for &ti in &bad_triangles {
            let [a, b, c] = triangles[ti];
            for edge in [[a, b], [b, c], [c, a]] {
                let shared = bad_triangles.iter().any(|&oti| {
                    oti != ti && {
                        let [oa, ob, oc] = triangles[oti];
                        let tri_edges = [[oa, ob], [ob, oc], [oc, oa]];
                        tri_edges.iter().any(|oe| {
                            (oe[0] == edge[0] && oe[1] == edge[1])
                            || (oe[0] == edge[1] && oe[1] == edge[0])
                        })
                    }
                });
                if !shared {
                    polygon.push(edge);
                }
            }
        }

        // Remove bad triangles (in reverse order to keep indices valid)
        bad_triangles.sort_unstable();
        for &ti in bad_triangles.iter().rev() {
            triangles.swap_remove(ti);
        }

        // Re-triangulate the polygon hole with the new point
        for edge in &polygon {
            triangles.push([edge[0], edge[1], pi]);
        }
    }

    // Collect triangles that don't reference super-triangle vertices
    let mut result: Vec<u32> = Vec::new();
    for &[a, b, c] in &triangles {
        if a < n && b < n && c < n {
            result.push(a as u32);
            result.push(b as u32);
            result.push(c as u32);
        }
    }
    result
}

/// Test whether point `p` is inside the circumcircle of triangle (a, b, c).
fn in_circumcircle(a: &[f64; 2], b: &[f64; 2], c: &[f64; 2], p: &[f64; 2]) -> bool {
    let ax = a[0] - p[0];
    let ay = a[1] - p[1];
    let bx = b[0] - p[0];
    let by = b[1] - p[1];
    let cx_ = c[0] - p[0];
    let cy = c[1] - p[1];

    let det = ax * (by * (cx_ * cx_ + cy * cy) - cy * (bx * bx + by * by))
            - ay * (bx * (cx_ * cx_ + cy * cy) - cx_ * (bx * bx + by * by))
            + (ax * ax + ay * ay) * (bx * cy - by * cx_);
    det > 0.0
}
