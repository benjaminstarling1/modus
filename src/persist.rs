use serde::{Serialize, Deserialize};
use crate::app::VisualizationMode;
use crate::palette::Palette;
use crate::data::{Dataset, Unit};
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
    #[serde(rename = "show_csys")]
    pub show_coord_sys: bool,
    pub show_xy:      bool,
    pub show_yz:      bool,
    pub show_zx:      bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// User Preferences (persistent across sessions)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPrefs {
    pub default_palette:       Palette,
    pub default_reverse_pal:   bool,
    pub default_vis_mode:      VisualizationMode,
    pub default_node_size:     f32,
    pub max_node_size_pct:     f32,   // % of bounding-box diagonal
    pub default_speed:         f32,
    pub default_fps:           f32,
    pub default_auto_scale:    bool,
    pub default_orthographic:  bool,
    /// Local coord sys arm length as % of global coord sys arm length (default 80, max 200).
    #[serde(rename = "local_csys_scale_pct", default = "default_local_csys_scale_pct")]
    pub local_coord_sys_scale_pct: f32,
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
    pub default_distance_unit: Unit,
    /// Lighting brightness multiplier (0.5 = dim, 1.0 = neutral, 2.0 = bright).
    #[serde(default = "default_light_brightness")]
    pub light_brightness: f32,
    /// Whether directional lighting is enabled.
    #[serde(default = "default_true")]
    pub lighting_enabled: bool,
    /// Default colour for vector arrows when contour colouring is off.
    #[serde(default = "default_arrow_color")]
    pub arrow_color: [f32; 3],
    /// Default colour for nodes (when no per-node override or contour is active).
    #[serde(default = "default_node_color")]
    pub default_node_color: [f32; 3],
    /// Default colour for edges (when no per-edge override or contour is active).
    #[serde(default = "default_edge_color")]
    pub default_edge_color: [f32; 3],
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            default_palette:      Palette::Turbo,
            default_reverse_pal:  false,
            default_vis_mode:     VisualizationMode::ContourColor,
            default_node_size:    0.05,
            max_node_size_pct:    20.0,
            default_speed:        1.0,
            default_fps:          30.0,
            default_auto_scale:   true,
            default_orthographic: true,
            local_coord_sys_scale_pct: 80.0,
            dark_mode: true,
            viewport_bg_color: [0.1, 0.1, 0.12],
            middle_button_orbit: true,
            default_distance_unit: Unit::default(),
            light_brightness: 1.5,
            lighting_enabled: true,
            arrow_color: [0.0, 0.0, 0.0],
            default_node_color: [1.0, 0.85, 0.1],
            default_edge_color: [0.3, 0.85, 1.0],
        }
    }
}

fn default_local_csys_scale_pct() -> f32 { 80.0 }
fn default_true() -> bool { true }
fn default_viewport_bg_dark() -> [f32; 3] { [0.1, 0.1, 0.12] }
fn default_light_brightness() -> f32 { 1.5 }
fn default_arrow_color() -> [f32; 3] { [0.0, 0.0, 0.0] }
fn default_node_color() -> [f32; 3] { [1.0, 0.85, 0.1] }
fn default_edge_color() -> [f32; 3] { [0.3, 0.85, 1.0] }

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
    pub distance_unit: Unit,

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
        distance_unit: &Unit,
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
                row.channel_dx = remap.get(&row.channel_dx).copied().unwrap_or(0);
                row.channel_dy = remap.get(&row.channel_dy).copied().unwrap_or(0);
                row.channel_dz = remap.get(&row.channel_dz).copied().unwrap_or(0);
                row.channel_rx = remap.get(&row.channel_rx).copied().unwrap_or(0);
                row.channel_ry = remap.get(&row.channel_ry).copied().unwrap_or(0);
                row.channel_rz = remap.get(&row.channel_rz).copied().unwrap_or(0);
            }
        }
        // else: legacy inline datasets are already populated by serde.

        Ok(model)
    }
}

// Re-export UI windows (now defined in app_ui.rs).
pub use crate::app_ui::{show_options_window, show_views_window};
