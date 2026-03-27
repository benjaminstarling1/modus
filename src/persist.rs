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
    Centimeter,
    Meter,
    Inch,
    Foot,
}

impl Default for DistanceUnit {
    fn default() -> Self { Self::Millimeter }
}

impl DistanceUnit {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Millimeter => "mm",
            Self::Centimeter => "cm",
            Self::Meter      => "m",
            Self::Inch       => "in",
            Self::Foot       => "ft",
        }
    }
    pub const ALL: &'static [DistanceUnit] = &[
        DistanceUnit::Millimeter,
        DistanceUnit::Centimeter,
        DistanceUnit::Meter,
        DistanceUnit::Inch,
        DistanceUnit::Foot,
    ];
    /// Meters per one of this unit (SI conversion factor).
    pub fn to_meters(&self) -> f64 {
        match self {
            Self::Millimeter => 0.001,
            Self::Centimeter => 0.01,
            Self::Meter      => 1.0,
            Self::Inch       => 0.0254,
            Self::Foot       => 0.3048,
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
        }
    }
}

fn default_local_csys_scale_pct() -> f32 { 80.0 }
fn default_true() -> bool { true }
fn default_viewport_bg_dark() -> [f32; 3] { [0.1, 0.1, 0.12] }

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
// Model File (project save/load)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFile {
    pub datasets:      Vec<Dataset>,
    pub channel_names: Vec<String>,
    pub rows:          Vec<Row>,
    pub edges:         Vec<Edge>,
    #[serde(default)]
    pub glyphs:        Vec<Glyph>,
    #[serde(default)]
    pub meshes:        Vec<Mesh>,
    pub saved_views:   Vec<SavedView>,
}

impl ModelFile {
    pub fn save_to_file(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let json = std::fs::read_to_string(path)?;
        let model: Self = serde_json::from_str(&json)?;
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
