use serde::{Serialize, Deserialize};
use crate::app::{VisMode, Palette};
use crate::data::Dataset;
use crate::table::{Row, Edge, Glyph, Mesh};

// ─────────────────────────────────────────────────────────────────────────────
// Saved View
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedView {
    pub name:         String,
    pub azimuth:      f32,
    pub elevation:    f32,
    pub distance:     f32,
    pub target:       [f32; 3],
    pub orthographic: bool,
    pub show_csys:    bool,
    pub show_xy:      bool,
    pub show_yz:      bool,
    pub show_zx:      bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Distance unit
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DistanceUnit {
    Millimeter,
    Meter,
    Kilometer,
    Inch,
    Foot,
    Mile,
}

impl Default for DistanceUnit {
    fn default() -> Self { Self::Millimeter }
}

impl DistanceUnit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Millimeter => "mm",
            Self::Meter      => "m",
            Self::Kilometer  => "km",
            Self::Inch       => "in",
            Self::Foot       => "ft",
            Self::Mile       => "mi",
        }
    }
    pub const ALL: &'static [DistanceUnit] = &[
        DistanceUnit::Millimeter,
        DistanceUnit::Meter,
        DistanceUnit::Kilometer,
        DistanceUnit::Inch,
        DistanceUnit::Foot,
        DistanceUnit::Mile,
    ];
    /// Meters per one of this unit (SI conversion factor).
    pub fn to_meters(&self) -> f64 {
        match self {
            Self::Millimeter => 0.001,
            Self::Meter      => 1.0,
            Self::Kilometer  => 1000.0,
            Self::Inch       => 0.0254,
            Self::Foot       => 0.3048,
            Self::Mile       => 1609.344,
        }
    }
    /// Conversion factor to multiply values when switching from `self` to `to`.
    pub fn convert_factor(&self, to: &DistanceUnit) -> f64 {
        self.to_meters() / to.to_meters()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// User Preferences (persistent across sessions)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrefs {
    pub default_palette:       Palette,
    pub default_reverse_pal:   bool,
    pub default_vis_mode:      VisMode,
    pub default_node_size:     f32,
    pub max_node_size_pct:     f32,   // % of bounding-box diagonal
    pub default_speed:         f32,
    pub default_fps:           f32,
    pub default_auto_scale:    bool,
    pub default_orthographic:  bool,
    /// Local CSYS arm length as % of global CSYS arm length (default 80, max 200).
    #[serde(default = "default_local_csys_scale_pct")]
    pub local_csys_scale_pct:  f32,
    /// Dark mode (true) or light mode (false).
    #[serde(default = "default_true")]
    pub dark_mode: bool,
    /// Viewport background colour [R, G, B] in 0..1 range.
    #[serde(default = "default_viewport_bg_dark")]
    pub viewport_bg_color: [f32; 3],
    /// Use middle-mouse-button for orbit (and RMB for pan) instead of LMB orbit / MMB pan.
    #[serde(default)]
    pub middle_button_orbit: bool,
    /// Default distance/displacement unit for new models.
    #[serde(default)]
    pub default_distance_unit: DistanceUnit,
    /// Lighting brightness multiplier (0.5 = dim, 1.0 = neutral, 2.0 = bright).
    #[serde(default = "default_light_brightness")]
    pub light_brightness: f32,
    /// Whether directional lighting is enabled.
    #[serde(default = "default_true")]
    pub lighting_enabled: bool,
    /// Default colour for vector arrows when contour colouring is off.
    #[serde(default = "default_arrow_color")]
    pub arrow_color: [f32; 3],
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            default_palette:      Palette::Turbo,
            default_reverse_pal:  false,
            default_vis_mode:     VisMode::ContourColor,
            default_node_size:    0.05,
            max_node_size_pct:    20.0,
            default_speed:        1.0,
            default_fps:          30.0,
            default_auto_scale:   true,
            default_orthographic: true,
            local_csys_scale_pct: 80.0,
            dark_mode: true,
            viewport_bg_color: [0.1, 0.1, 0.12],
            middle_button_orbit: true,
            default_distance_unit: DistanceUnit::default(),
            light_brightness: 1.5,
            lighting_enabled: true,
            arrow_color: [0.0, 0.0, 0.0],
        }
    }
}

fn default_local_csys_scale_pct() -> f32 { 80.0 }
fn default_true() -> bool { true }
fn default_viewport_bg_dark() -> [f32; 3] { [0.1, 0.1, 0.12] }
fn default_light_brightness() -> f32 { 1.5 }
fn default_arrow_color() -> [f32; 3] { [0.0, 0.0, 0.0] }

impl UserPrefs {
    fn config_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|d| d.join("ods-animator").join("preferences.json"))
    }

    pub fn load() -> Self {
        Self::config_path()
            .and_then(|p| std::fs::read_to_string(&p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Some(p) = Self::config_path() {
            if let Some(parent) = p.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(json) = serde_json::to_string_pretty(self) {
                let _ = std::fs::write(&p, json);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Data reference (lightweight pointer to an external CSV / Parquet file)
// ─────────────────────────────────────────────────────────────────────────────

use std::collections::HashMap;
use crate::data::ChannelMeta;

/// A reference to an external data file, stored in the model JSON instead of
/// the full time-series arrays.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataRef {
    /// Relative path (from the model file's directory) to the CSV / Parquet.
    pub path: String,
    /// Per-channel physical-quantity + unit metadata, keyed by channel name.
    #[serde(default)]
    pub channel_meta: HashMap<String, ChannelMeta>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Model File (project save/load)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    /// Distance unit the model was authored in.
    #[serde(default)]
    pub distance_unit: DistanceUnit,

    /// References to external data files (new format).
    #[serde(default)]
    pub data_refs:     Vec<DataRef>,

    pub channel_names: Vec<String>,
    pub rows:          Vec<Row>,
    pub edges:         Vec<Edge>,
    #[serde(default)]
    pub glyphs:        Vec<Glyph>,
    #[serde(default)]
    pub meshes:        Vec<Mesh>,
    pub saved_views:   Vec<SavedView>,

    /// Legacy inline datasets — kept for backward compatibility on load,
    /// but never written on new saves.
    #[serde(default, skip_serializing)]
    pub datasets:      Vec<Dataset>,
}

impl ModelFile {
    /// Build `data_refs` from live datasets, computing paths relative to the
    /// model file's parent directory.
    pub fn save_to_file(
        &self,
        path: &std::path::Path,
        datasets: &[Dataset],
        distance_unit: &DistanceUnit,
    ) -> anyhow::Result<()> {
        let model_dir = path.parent().unwrap_or(std::path::Path::new("."));

        let model_dir_abs = std::fs::canonicalize(model_dir).unwrap_or_else(|_| model_dir.to_path_buf());

        let data_refs: Vec<DataRef> = datasets.iter().map(|ds| {
            // Canonicalize the dataset path so diff_paths works reliably.
            let ds_abs = std::fs::canonicalize(&ds.path).unwrap_or_else(|_| ds.path.clone());
            let rel = pathdiff::diff_paths(&ds_abs, &model_dir_abs)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| ds.path.to_string_lossy().into_owned());
            DataRef {
                path: rel,
                channel_meta: ds.channel_meta.clone(),
            }
        }).collect();

        let mf = ModelFile {
            distance_unit: distance_unit.clone(),
            data_refs,
            channel_names: self.channel_names.clone(),
            rows:          self.rows.clone(),
            edges:         self.edges.clone(),
            glyphs:        self.glyphs.clone(),
            meshes:        self.meshes.clone(),
            saved_views:   self.saved_views.clone(),
            datasets:      Vec::new(), // never written
        };
        let json = serde_json::to_string_pretty(&mf)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a model file. Data references are resolved relative to the model
    /// file's parent directory. If a referenced file cannot be found the
    /// dataset is skipped (geometry still loads).
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let mut model: Self = serde_json::from_str(&json)?;
        let model_dir = path.parent().unwrap_or(std::path::Path::new("."));

        // --- Resolve data references → live datasets -----------------------
        if !model.data_refs.is_empty() {
            let mut datasets: Vec<Dataset> = Vec::new();
            for dref in &model.data_refs {
                let data_path = model_dir.join(&dref.path);
                // Canonicalize so ds.path is always absolute — needed for
                // reliable relative-path computation during save.
                let data_path = std::fs::canonicalize(&data_path).unwrap_or(data_path);
                let ext = data_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let result = match ext.as_str() {
                    "csv"     => crate::data::load_csv(&data_path),
                    "parquet" => crate::data::load_parquet(&data_path),
                    _         => {
                        eprintln!("Warning: unsupported data ref extension '{}' for {:?}", ext, data_path);
                        continue;
                    }
                };

                match result {
                    Ok(mut ds) => {
                        // Restore saved channel metadata (data type + unit).
                        for (ch, meta) in &dref.channel_meta {
                            ds.channel_meta.insert(ch.clone(), meta.clone());
                        }
                        ds.rebuild_all_displacement();
                        datasets.push(ds);
                    }
                    Err(e) => {
                        eprintln!("Warning: could not load data ref {:?}: {e}", data_path);
                    }
                }
            }
            model.datasets = datasets;

            // Rebuild channel_names from the actual loaded datasets, then
            // remap row dx/dy/dz indices so they point to the correct
            // channels in the new list.
            let old_names = model.channel_names.clone();
            crate::data::rebuild_channels(&model.datasets, &mut model.channel_names);

            // Build a mapping: old_index (1-based) → new_index (1-based).
            // Match by the bare channel name (the part after "::").
            let mut remap: HashMap<usize, usize> = HashMap::new();
            for (old_i, old_qname) in old_names.iter().enumerate() {
                let old_bare = old_qname.split_once("::").map(|(_, b)| b).unwrap_or(old_qname);
                for (new_i, new_qname) in model.channel_names.iter().enumerate() {
                    let new_bare = new_qname.split_once("::").map(|(_, b)| b).unwrap_or(new_qname);
                    if old_bare == new_bare {
                        remap.insert(old_i + 1, new_i + 1);
                        break;
                    }
                }
            }

            for row in &mut model.rows {
                row.dx = remap.get(&row.dx).copied().unwrap_or(0);
                row.dy = remap.get(&row.dy).copied().unwrap_or(0);
                row.dz = remap.get(&row.dz).copied().unwrap_or(0);
                row.rx = remap.get(&row.rx).copied().unwrap_or(0);
                row.ry = remap.get(&row.ry).copied().unwrap_or(0);
                row.rz = remap.get(&row.rz).copied().unwrap_or(0);
            }
        }
        // else: legacy inline datasets are already populated by serde.

        Ok(model)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Options window
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_options_window(ctx: &egui::Context, open: &mut bool, prefs: &mut UserPrefs) {
    egui::Window::new("Options")
        .open(open)
        .resizable(false)
        .default_width(320.0)
        .collapsible(false)
        .show(ctx, |ui| {
            egui::Grid::new("prefs_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    // Default palette
                    ui.label("Default Palette:");
                    egui::ComboBox::from_id_salt("pref_palette")
                        .selected_text(prefs.default_palette.label())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut prefs.default_palette, Palette::Viridis, "Viridis");
                            ui.selectable_value(&mut prefs.default_palette, Palette::Plasma,  "Plasma");
                            ui.selectable_value(&mut prefs.default_palette, Palette::Cool,    "Cool");
                            ui.selectable_value(&mut prefs.default_palette, Palette::Hot,     "Hot");
                            ui.selectable_value(&mut prefs.default_palette, Palette::Turbo,   "Turbo");
                        });
                    ui.end_row();

                    // Reverse palette
                    ui.label("Reverse Palette:");
                    ui.checkbox(&mut prefs.default_reverse_pal, "");
                    ui.end_row();

                    // Default vis mode
                    ui.label("Default Vis Mode:");
                    egui::ComboBox::from_id_salt("pref_vis")
                        .selected_text(match &prefs.default_vis_mode {
                            VisMode::None           => "None",
                            VisMode::ContourColor   => "Contour",
                            VisMode::SizeScale      => "Size",
                            VisMode::ContourAndSize => "Both",
                        })
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut prefs.default_vis_mode, VisMode::None,           "None");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisMode::ContourColor,   "Contour");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisMode::SizeScale,      "Size");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisMode::ContourAndSize, "Both");
                        });
                    ui.end_row();

                    // Default node size
                    ui.label("Default Node Size:");
                    ui.add(egui::DragValue::new(&mut prefs.default_node_size)
                        .range(0.001..=1.0).speed(0.005));
                    ui.end_row();

                    // Max node size %
                    ui.label("Max Node Size (% bbox):");
                    ui.add(egui::DragValue::new(&mut prefs.max_node_size_pct)
                        .range(1.0..=100.0).speed(0.5).suffix("%"));
                    ui.end_row();

                    // Default speed
                    ui.label("Default Speed:");
                    ui.add(egui::DragValue::new(&mut prefs.default_speed)
                        .range(0.01..=20.0).speed(0.05).suffix("×"));
                    ui.end_row();

                    // Default FPS
                    ui.label("Default FPS:");
                    ui.add(egui::DragValue::new(&mut prefs.default_fps)
                        .range(1.0..=1000.0).speed(1.0));
                    ui.end_row();

                    // Default auto-scale
                    ui.label("Auto-Scale by Default:");
                    ui.checkbox(&mut prefs.default_auto_scale, "");
                    ui.end_row();

                    // Default projection
                    ui.label("Default Projection:");
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut prefs.default_orthographic, false, "Perspective");
                        ui.radio_value(&mut prefs.default_orthographic, true,  "Orthographic");
                    });
                    ui.end_row();

                    // Local CSYS scale
                    ui.label("Local CSYS Size (% global):");
                    ui.add(egui::DragValue::new(&mut prefs.local_csys_scale_pct)
                        .range(10.0..=200.0).speed(1.0).suffix("%"));
                    ui.end_row();

                    // Theme
                    ui.label("Theme:");
                    ui.horizontal(|ui| {
                        if ui.selectable_label(prefs.dark_mode, "Dark").clicked() && !prefs.dark_mode {
                            prefs.dark_mode = true;
                            prefs.viewport_bg_color = [0.1, 0.1, 0.12];
                        }
                        if ui.selectable_label(!prefs.dark_mode, "Light").clicked() && prefs.dark_mode {
                            prefs.dark_mode = false;
                            prefs.viewport_bg_color = [1.0, 1.0, 1.0];
                        }
                    });
                    ui.end_row();

                    // Viewport background colour
                    ui.label("Viewport Background:");
                    ui.color_edit_button_rgb(&mut prefs.viewport_bg_color);
                    ui.end_row();

                    // Mouse controls
                    ui.label("Orbit Button:");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut prefs.middle_button_orbit, false, "Left (LMB)");
                        ui.selectable_value(&mut prefs.middle_button_orbit, true,  "Middle (MMB)");
                    });
                    ui.end_row();

                    // Default distance unit
                    ui.label("Default Distance Unit:");
                    egui::ComboBox::from_id_salt("pref_dist_unit")
                        .selected_text(prefs.default_distance_unit.label())
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            for u in DistanceUnit::ALL {
                                ui.selectable_value(
                                    &mut prefs.default_distance_unit,
                                    u.clone(),
                                    u.label(),
                                );
                            }
                        });
                    ui.end_row();

                    // Directional lighting
                    ui.label("Directional Lighting:");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut prefs.lighting_enabled, "");
                        if prefs.lighting_enabled {
                            ui.add(egui::Slider::new(&mut prefs.light_brightness, 0.5..=3.0)
                                .step_by(0.05)
                                .text(""));
                        }
                    });
                    ui.end_row();

                    // Arrow colour
                    ui.label("Arrow Color:");
                    ui.color_edit_button_rgb(&mut prefs.arrow_color);
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save Preferences").clicked() {
                    prefs.save();
                }
                if ui.button("Reset to Defaults").clicked() {
                    *prefs = UserPrefs::default();
                }
            });
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// Saved Views window
// ─────────────────────────────────────────────────────────────────────────────

use crate::viewport::Viewport3D;

pub fn show_views_window(
    ctx: &egui::Context,
    open: &mut bool,
    views: &mut Vec<SavedView>,
    viewport: &mut Viewport3D,
) {
    egui::Window::new("Saved Views")
        .open(open)
        .resizable(true)
        .default_width(300.0)
        .default_height(250.0)
        .show(ctx, |ui| {
            // Save current view button
            ui.horizontal(|ui| {
                if ui.button("📷  Save Current View").clicked() {
                    let n = views.len() + 1;
                    views.push(SavedView {
                        name:         format!("View {}", n),
                        azimuth:      viewport.azimuth,
                        elevation:    viewport.elevation,
                        distance:     viewport.distance,
                        target:       viewport.target.into(),
                        orthographic: viewport.orthographic,
                        show_csys:    viewport.show_csys,
                        show_xy:      viewport.show_xy,
                        show_yz:      viewport.show_yz,
                        show_zx:      viewport.show_zx,
                    });
                }
            });

            ui.separator();

            let mut delete_idx: Option<usize> = None;

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, view) in views.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            // Editable name
                            ui.add(
                                egui::TextEdit::singleline(&mut view.name)
                                    .desired_width(120.0)
                            );

                            // Restore button
                            if ui.button("Restore").clicked() {
                                viewport.azimuth      = view.azimuth;
                                viewport.elevation    = view.elevation;
                                viewport.distance     = view.distance;
                                viewport.target       = glam::Vec3::from(view.target);
                                viewport.orthographic = view.orthographic;
                                viewport.show_csys    = view.show_csys;
                                viewport.show_xy      = view.show_xy;
                                viewport.show_yz      = view.show_yz;
                                viewport.show_zx      = view.show_zx;
                            }

                            // Update button — overwrite this view with current camera
                            if ui.button("Update").clicked() {
                                view.azimuth      = viewport.azimuth;
                                view.elevation    = viewport.elevation;
                                view.distance     = viewport.distance;
                                view.target       = viewport.target.into();
                                view.orthographic = viewport.orthographic;
                                view.show_csys    = viewport.show_csys;
                                view.show_xy      = viewport.show_xy;
                                view.show_yz      = viewport.show_yz;
                                view.show_zx      = viewport.show_zx;
                            }

                            // Delete
                            if ui.button(egui_phosphor::regular::TRASH).clicked() {
                                delete_idx = Some(i);
                            }
                        });
                    });
                }
            });

            if let Some(idx) = delete_idx {
                views.remove(idx);
            }
        });
}
