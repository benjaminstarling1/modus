use std::{collections::HashMap, path::PathBuf};
use serde::{Serialize, Deserialize};

// ─────────────────────────────────────────────────────────────────────────────
// Channel metadata
// ─────────────────────────────────────────────────────────────────────────────

/// The physical quantity a channel represents.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub enum DataType {
    #[default]
    Displacement,
    Velocity,
    Acceleration,
}

impl DataType {
    pub fn label(&self) -> &'static str {
        match self {
            DataType::Displacement  => "Disp",
            DataType::Velocity      => "Vel",
            DataType::Acceleration  => "Accel",
        }
    }
}

/// Physical unit for a channel — determines the SI conversion factor applied
/// before integration so that the animation is always in metres.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Unit {
    // --- Displacement ---
    Millimeter,
    Meter,
    Kilometer,
    Inch,
    Foot,
    Mile,
    // --- Velocity ---
    MillimeterPerSec,
    MeterPerSec,
    KilometerPerHour,
    MilePerHour,
    InchPerSec,
    FootPerSec,
    // --- Acceleration ---
    MillimeterPerSec2,
    MeterPerSec2,
    StandardGravity,
    InchPerSec2,
    FootPerSec2,
}

impl Default for Unit {
    fn default() -> Self { Self::Millimeter }
}

impl Unit {
    /// Human-readable label shown in the dropdown.
    pub fn label(&self) -> &'static str {
        match self {
            Unit::Millimeter         => "mm",
            Unit::Meter              => "m",
            Unit::Kilometer          => "km",
            Unit::Inch               => "in",
            Unit::Foot               => "ft",
            Unit::Mile               => "mi",
            Unit::MillimeterPerSec   => "mm/s",
            Unit::MeterPerSec        => "m/s",
            Unit::KilometerPerHour   => "km/h",
            Unit::MilePerHour        => "mph",
            Unit::InchPerSec         => "in/s",
            Unit::FootPerSec         => "ft/s",
            Unit::MillimeterPerSec2  => "mm/s²",
            Unit::MeterPerSec2       => "m/s²",
            Unit::StandardGravity    => "g",
            Unit::InchPerSec2        => "in/s²",
            Unit::FootPerSec2        => "ft/s²",
        }
    }

    /// Multiply raw data values by this factor to convert to SI base units
    /// (metres for displacement, m/s for velocity, m/s² for acceleration)
    /// before integration.
    pub fn to_si_factor(&self) -> f32 {
        match self {
            Unit::Millimeter         => 0.001,
            Unit::Meter              => 1.0,
            Unit::Kilometer          => 1000.0,
            Unit::Inch               => 0.0254,
            Unit::Foot               => 0.3048,
            Unit::Mile               => 1609.344,
            Unit::MillimeterPerSec   => 0.001,
            Unit::MeterPerSec        => 1.0,
            Unit::KilometerPerHour   => 1.0 / 3.6,
            Unit::MilePerHour        => 0.44704,
            Unit::InchPerSec         => 0.0254,
            Unit::FootPerSec         => 0.3048,
            Unit::MillimeterPerSec2  => 0.001,
            Unit::MeterPerSec2       => 1.0,
            Unit::StandardGravity    => 9.80665,
            Unit::InchPerSec2        => 0.0254,
            Unit::FootPerSec2        => 0.3048,
        }
    }

    /// Default unit for a given DataType.
    pub fn default_for(dt: &DataType) -> Self {
        match dt {
            DataType::Displacement  => Unit::Meter,
            DataType::Velocity      => Unit::MeterPerSec,
            DataType::Acceleration  => Unit::MeterPerSec2,
        }
    }

    /// All distance unit variants (in display order), for use where only
    /// distance units are appropriate (e.g. model coordinate unit selector).
    pub const DISTANCE_UNITS: &'static [Unit] = &[
        Unit::Millimeter, Unit::Meter, Unit::Kilometer,
        Unit::Inch, Unit::Foot, Unit::Mile,
    ];

    /// Meters per one of this unit — used when converting model coordinates.
    /// Only valid for distance variants; panics on velocity/acceleration variants.
    pub fn to_meters(&self) -> f64 {
        match self {
            Unit::Millimeter => 0.001,
            Unit::Meter      => 1.0,
            Unit::Kilometer  => 1000.0,
            Unit::Inch       => 0.0254,
            Unit::Foot       => 0.3048,
            Unit::Mile       => 1609.344,
            _ => panic!("to_meters() called on non-distance Unit variant"),
        }
    }

    /// Conversion factor: multiply by this to convert from `self` to `to`.
    pub fn convert_factor(&self, to: &Unit) -> f64 {
        self.to_meters() / to.to_meters()
    }

    /// All valid units for a given DataType (in display order).
    pub fn options_for(dt: &DataType) -> &'static [Unit] {
        match dt {
            DataType::Displacement => &[
                Unit::Millimeter, Unit::Meter, Unit::Kilometer,
                Unit::Inch, Unit::Foot, Unit::Mile,
            ],
            DataType::Velocity => &[
                Unit::MillimeterPerSec, Unit::MeterPerSec,
                Unit::KilometerPerHour, Unit::MilePerHour,
                Unit::InchPerSec, Unit::FootPerSec,
            ],
            DataType::Acceleration => &[
                Unit::MillimeterPerSec2, Unit::MeterPerSec2,
                Unit::StandardGravity, Unit::InchPerSec2, Unit::FootPerSec2,
            ],
        }
    }
}

/// Per-channel user-editable metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeta {
    pub data_type: DataType,
    pub unit:      Unit,
}

impl Default for ChannelMeta {
    fn default() -> Self {
        Self { data_type: DataType::Displacement, unit: Unit::Meter }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dataset
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dataset {
    pub name:         String,
    pub path:         PathBuf,
    pub channels:     Vec<String>,
    /// Time axis (seconds), read from column 0.
    pub time:         Vec<f32>,
    /// Raw channel values, indexed by channel name.
    pub values:       HashMap<String, Vec<f32>>,
    /// Per-channel physical-quantity + units metadata.
    pub channel_meta: HashMap<String, ChannelMeta>,
    /// Pre-integrated displacement for each channel (integrated from raw as needed).
    /// Always in the same units ‥ m if the user specifics SI, otherwise raw-integrated.
    pub displacement: HashMap<String, Vec<f32>>,
}

impl Dataset {
    /// Recompute the displacement array for one channel based on its current
    /// `ChannelMeta`. Call whenever the user changes a channel's data type.
    pub fn rebuild_displacement_for(&mut self, ch: &str) {
        let values = match self.values.get(ch) {
            Some(v) => v.clone(),
            None    => return,
        };
        let meta = self.channel_meta.entry(ch.to_string()).or_default().clone();
        let si   = meta.unit.to_si_factor();
        // Apply SI conversion factor first, so integrated values are in metres.
        let si_values: Vec<f32> = values.iter().map(|v| v * si).collect();
        let disp = match meta.data_type {
            DataType::Displacement  => si_values,
            DataType::Velocity      => integrate_once(&self.time, &si_values),
            DataType::Acceleration  => {
                let vel = integrate_once(&self.time, &si_values);
                integrate_once(&self.time, &vel)
            }
        };
        // Remove DC offset: shift so displacement at t=0 is zero,
        // ensuring the node starts at its defined position.
        let offset = disp.first().copied().unwrap_or(0.0);
        let disp: Vec<f32> = disp.iter().map(|v| v - offset).collect();
        self.displacement.insert(ch.to_string(), disp);
    }

    /// Rebuild displacement for every channel.
    pub fn rebuild_all_displacement(&mut self) {
        let names: Vec<String> = self.channels.clone();
        for ch in &names {
            self.rebuild_displacement_for(ch);
        }
    }

    /// Interpolate the pre-integrated displacement of `channel` at time `t` (seconds).
    /// Returns 0.0 if the channel has no displacement data.
    pub fn sample_displacement(&self, channel: &str, t: f64) -> f32 {
        let disp = match self.displacement.get(channel) {
            Some(d) if !d.is_empty() => d,
            _                         => return 0.0,
        };
        let ti = &self.time;
        if ti.len() < 2 { return disp[0]; }

        let t = t as f32;
        // Binary search for the bracket.
        let n = ti.len();
        if t <= ti[0]       { return disp[0]; }
        if t >= ti[n - 1]   { return disp[n - 1]; }

        let idx = ti.partition_point(|&v| v <= t).saturating_sub(1).min(n - 2);
        let t0 = ti[idx];
        let t1 = ti[idx + 1];
        let frac = if (t1 - t0).abs() < 1e-12 { 0.0 } else { (t - t0) / (t1 - t0) };
        disp[idx] * (1.0 - frac) + disp[idx + 1] * frac
    }

    /// Compute the time-derivative of the pre-integrated displacement channel at time `t`
    /// using finite differences.  Returns velocity in SI units (m/s).
    pub fn sample_velocity_at(&self, channel: &str, t: f64) -> f32 {
        let disp = match self.displacement.get(channel) {
            Some(d) if d.len() >= 2 => d,
            _ => return 0.0,
        };
        let ti = &self.time;
        let n = ti.len().min(disp.len());
        if n < 2 { return 0.0; }
        let t = t as f32;
        // Find the nearest index, clamp to valid range.
        let i = ti.partition_point(|&v| v <= t).min(n - 1);
        // Central differences where possible, one-sided at edges.
        if i == 0 {
            let dt = ti[1] - ti[0];
            if dt.abs() < 1e-12 { return 0.0; }
            (disp[1] - disp[0]) / dt
        } else if i >= n - 1 {
            let dt = ti[n - 1] - ti[n - 2];
            if dt.abs() < 1e-12 { return 0.0; }
            (disp[n - 1] - disp[n - 2]) / dt
        } else {
            let dt = ti[i + 1] - ti[i - 1];
            if dt.abs() < 1e-12 { return 0.0; }
            (disp[i + 1] - disp[i - 1]) / dt
        }
    }

    /// Compute the second time-derivative of the pre-integrated displacement channel at time `t`.
    /// Returns acceleration in SI units (m/s²).
    pub fn sample_acceleration_at(&self, channel: &str, t: f64) -> f32 {
        let disp = match self.displacement.get(channel) {
            Some(d) if d.len() >= 3 => d,
            _ => return 0.0,
        };
        let ti = &self.time;
        let n = ti.len().min(disp.len());
        if n < 3 { return 0.0; }
        let t = t as f32;
        let i = ti.partition_point(|&v| v <= t).min(n - 1);
        // Second finite difference, clamped to interior.
        let i = i.max(1).min(n - 2);
        let dt_back = ti[i] - ti[i - 1];
        let dt_fwd  = ti[i + 1] - ti[i];
        let dt = 0.5 * (dt_back + dt_fwd);
        if dt.abs() < 1e-12 { return 0.0; }
        (disp[i + 1] - 2.0 * disp[i] + disp[i - 1]) / (dt * dt)
    }

    /// Duration of the dataset in seconds (0.0 if no time data).
    pub fn duration(&self) -> f64 {
        self.time.last().copied().unwrap_or(0.0) as f64
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Integration
// ─────────────────────────────────────────────────────────────────────────────

/// Trapezoidal cumulative integration: ∫ values dt.
fn integrate_once(time: &[f32], values: &[f32]) -> Vec<f32> {
    let n = time.len().min(values.len());
    let mut out = vec![0.0_f32; n];
    for i in 1..n {
        let dt = time[i] - time[i - 1];
        out[i] = out[i - 1] + 0.5 * (values[i - 1] + values[i]) * dt;
    }
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Loaders
// ─────────────────────────────────────────────────────────────────────────────

/// Loads a CSV file. First column = time axis; remaining columns = channels.
pub fn load_csv(path: &PathBuf) -> anyhow::Result<Dataset> {
    let mut rdr = csv::Reader::from_path(path)?;
    let headers = rdr.headers()?.clone();

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let channels: Vec<String> = headers.iter().skip(1).map(|s| s.to_string()).collect();
    let nchan = channels.len();

    let mut time_vec: Vec<f32>     = Vec::new();
    let mut raw: Vec<Vec<f32>>     = vec![Vec::new(); nchan];

    for result in rdr.records() {
        let rec = result?;
        let mut it = rec.iter();
        // First field = time
        let t: f32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(0.0);
        time_vec.push(t);
        for (i, field) in it.enumerate() {
            if i < nchan {
                raw[i].push(field.parse().unwrap_or(0.0));
            }
        }
    }

    let mut values    = HashMap::new();
    let mut channel_meta = HashMap::new();
    for (i, ch) in channels.iter().enumerate() {
        values.insert(ch.clone(), raw[i].clone());
        channel_meta.insert(ch.clone(), ChannelMeta::default());
    }

    let mut ds = Dataset {
        name, path: path.clone(), channels, time: time_vec,
        values, channel_meta, displacement: HashMap::new(),
    };
    ds.rebuild_all_displacement();
    Ok(ds)
}

/// Loads a Parquet file using Polars.
pub fn load_parquet(path: &PathBuf) -> anyhow::Result<Dataset> {
    use polars::prelude::*;

    let file = std::fs::File::open(path)?;
    let df   = ParquetReader::new(file).finish()?;

    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let col_names: Vec<String> = df.get_column_names().iter().map(|s| s.to_string()).collect();
    let channels: Vec<String>  = col_names.iter().skip(1).map(|s| s.clone()).collect();
    let nrows = df.height();

    // Extract time column (first).
    let extract_f32 = |series: &polars::prelude::Series| -> Vec<f32> {
        series.cast(&polars::prelude::DataType::Float32)
            .ok()
            .and_then(|s| s.f32().ok().map(|ca| ca.into_iter().map(|v| v.unwrap_or(0.0)).collect()))
            .unwrap_or_else(|| vec![0.0; nrows])
    };

    let time_col = df.column(&col_names[0])?.as_series().unwrap();
    let time_vec = extract_f32(&time_col);

    let mut values       = HashMap::new();
    let mut channel_meta = HashMap::new();
    for ch in &channels {
        let col = df.column(ch)?.as_series().unwrap();
        let v = extract_f32(&col);
        values.insert(ch.clone(), v);
        channel_meta.insert(ch.clone(), ChannelMeta::default());
    }

    let mut ds = Dataset {
        name, path: path.clone(), channels, time: time_vec,
        values, channel_meta, displacement: HashMap::new(),
    };
    ds.rebuild_all_displacement();
    Ok(ds)
}

// ─────────────────────────────────────────────────────────────────────────────
// Import panel UI
// ─────────────────────────────────────────────────────────────────────────────

/// Renders the left import panel. Returns true if the channel list changed.
pub fn show_import_panel(
    ui:            &mut egui::Ui,
    datasets:      &mut Vec<Dataset>,
    channel_names: &mut Vec<String>,
) -> bool {
    let mut changed = false;

    ui.label(egui::RichText::new("Data Import").size(16.0).strong());
    ui.separator();
    
    //format!("{} Import CSV", egui_phosphor::regular::DOWNLOAD_SIMPLE)
    if ui.button(format!("{}  Import CSV / Parquet…", egui_phosphor::regular::DOWNLOAD_SIMPLE)).clicked() {
        if let Some(paths) = rfd::FileDialog::new()
            .add_filter("Time series", &["csv", "parquet"])
            .pick_files()
        {
            for path in paths {
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();

                let result = match ext.as_str() {
                    "csv"     => load_csv(&path),
                    "parquet" => load_parquet(&path),
                    _         => Err(anyhow::anyhow!("Unsupported extension: {ext}")),
                };

                match result {
                    Ok(ds) => {
                        if !datasets.iter().any(|d| d.path == ds.path) {
                            datasets.push(ds);
                            changed = true;
                        }
                    }
                    Err(e) => eprintln!("Failed to load {:?}: {e}", path),
                }
            }
        }
    }

    ui.add_space(8.0);

    let mut remove_idx: Option<usize> = None;

    for (i, ds) in datasets.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            egui::CollapsingHeader::new(&ds.name)
                .default_open(false)
                .show(ui, |ui| {
                    let channels = ds.channels.clone();
                    for ch in &channels {
                        ui.push_id(ch, |ui| {
                            ui.horizontal(|ui| {
                                // Channel name (truncated if long)
                                ui.label(egui::RichText::new(ch).small());
                                ui.add_space(4.0);

                                let meta = ds.channel_meta.entry(ch.clone()).or_default();
                                let mut rebuild = false;

                                // DataType selector
                                egui::ComboBox::from_id_salt(("dt", i, ch))
                                    .selected_text(meta.data_type.label())
                                    .width(54.0)
                                    .show_ui(ui, |ui| {
                                        let mut sel = |variant: DataType, label| {
                                            if ui.selectable_value(&mut meta.data_type, variant.clone(), label).changed() {
                                                // Reset unit to the default for the new DataType.
                                                meta.unit = Unit::default_for(&variant);
                                                rebuild = true;
                                            }
                                        };
                                        sel(DataType::Displacement, "Disp");
                                        sel(DataType::Velocity,     "Vel");
                                        sel(DataType::Acceleration, "Accel");
                                    });

                                // Unit selector — filtered to options valid for the current DataType.
                                let options = Unit::options_for(&meta.data_type);
                                // Ensure current unit is valid for the current DataType.
                                if !options.contains(&meta.unit) {
                                    meta.unit = Unit::default_for(&meta.data_type);
                                    rebuild = true;
                                }
                                let cur_label = meta.unit.label();
                                egui::ComboBox::from_id_salt(("unit", i, ch))
                                    .selected_text(cur_label)
                                    .width(64.0)
                                    .show_ui(ui, |ui| {
                                        for opt in options {
                                            if ui.selectable_value(&mut meta.unit, opt.clone(), opt.label()).changed() {
                                                rebuild = true;
                                            }
                                        }
                                    });

                                if rebuild {
                                    ds.rebuild_displacement_for(ch);
                                }
                            });
                        });
                    }
                });

            ui.horizontal(|ui| {
                ui.add_space(16.0);
                if ui.small_button(format!("{}  Remove", egui_phosphor::regular::TRASH)).clicked() {
                    remove_idx = Some(i);
                }
            });
        });
        ui.add_space(2.0);
    }

    if let Some(idx) = remove_idx {
        datasets.remove(idx);
        changed = true;
    }

    if changed {
        rebuild_channels(datasets, channel_names);
    }

    changed
}

/// Rebuilds the flat channel name list from all datasets.
pub fn rebuild_channels(datasets: &[Dataset], channel_names: &mut Vec<String>) {
    channel_names.clear();
    for ds in datasets {
        for ch in &ds.channels {
            let qualified = format!("{}::{}", ds.name, ch);
            if !channel_names.contains(&qualified) {
                channel_names.push(qualified);
            }
        }
    }
}

/// Find the maximum displacement magnitude across all time steps for the
/// given channel (qualified name `"filename::channel"`), searching `datasets`.
/// Returns 0.0 if not found or data is empty.
pub fn channel_max_displacement(datasets: &[Dataset], qualified: &str) -> f32 {
    let Some((file, ch)) = qualified.split_once("::") else { return 0.0 };
    let Some(ds) = datasets.iter().find(|d| d.name == file) else { return 0.0 };
    ds.displacement.get(ch)
        .map(|v| v.iter().cloned().fold(0.0_f32, f32::max).abs())
        .unwrap_or(0.0)
}

/// Sample displacement for a qualified channel name at time `t`.
pub fn sample_by_channel_path(datasets: &[Dataset], qualified: &str, t: f64) -> f32 {
    let Some((file, ch)) = qualified.split_once("::") else { return 0.0 };
    let Some(ds) = datasets.iter().find(|d| d.name == file) else { return 0.0 };
    ds.sample_displacement(ch, t)
}

pub fn sample_velocity_by_channel_path(datasets: &[Dataset], qualified: &str, t: f64) -> f32 {
    let Some((file, ch)) = qualified.split_once("::") else { return 0.0 };
    let Some(ds) = datasets.iter().find(|d| d.name == file) else { return 0.0 };
    ds.sample_velocity_at(ch, t)
}

pub fn sample_acceleration_by_channel_path(datasets: &[Dataset], qualified: &str, t: f64) -> f32 {
    let Some((file, ch)) = qualified.split_once("::") else { return 0.0 };
    let Some(ds) = datasets.iter().find(|d| d.name == file) else { return 0.0 };
    ds.sample_acceleration_at(ch, t)
}

/// Longest duration in seconds, considering only datasets that have channels
/// actually assigned to nodes (via `dx`, `dy`, `dz` indices into `channel_names`).
pub fn max_duration(
    datasets: &[Dataset],
    channel_names: &[String],
    rows: &[crate::table::Row],
) -> f64 {
    // Collect the set of dataset names referenced by at least one node.
    let mut used_files: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for row in rows {
        for idx in [row.channel_dx, row.channel_dy, row.channel_dz] {
            if idx == 0 { continue; }
            if let Some(qname) = channel_names.get(idx - 1) {
                if let Some((file, _)) = qname.split_once("::") {
                    used_files.insert(file);
                }
            }
        }
    }

    datasets
        .iter()
        .filter(|d| used_files.contains(d.name.as_str()))
        .map(|d| d.duration())
        .fold(0.0_f64, f64::max)
}
