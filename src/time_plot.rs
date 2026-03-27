use crate::data::{Dataset, DataType, Unit};

// ─────────────────────────────────────────────────────────────────────────────
// Plot domain (which physical quantity to display)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PlotDomain {
    Displacement,
    Velocity,
    Acceleration,
}

impl PlotDomain {
    pub fn label(&self) -> &'static str {
        match self {
            PlotDomain::Displacement  => "Displacement",
            PlotDomain::Velocity      => "Velocity",
            PlotDomain::Acceleration  => "Acceleration",
        }
    }

    /// Convert to the corresponding `DataType` so we can reuse `Unit::options_for()`.
    pub fn as_data_type(&self) -> DataType {
        match self {
            PlotDomain::Displacement  => DataType::Displacement,
            PlotDomain::Velocity      => DataType::Velocity,
            PlotDomain::Acceleration  => DataType::Acceleration,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

pub struct TimePlotState {
    pub selected_channels: Vec<usize>,   // indices into channel_names
    pub plot_domain:       PlotDomain,
    pub plot_unit:         Unit,
}

impl Default for TimePlotState {
    fn default() -> Self {
        Self {
            selected_channels: Vec::new(),
            plot_domain:       PlotDomain::Displacement,
            plot_unit:         Unit::Meter,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain transform helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Trapezoidal cumulative integration.
fn integrate_trap(time: &[f32], values: &[f32]) -> Vec<f32> {
    let n = time.len().min(values.len());
    let mut out = vec![0.0_f32; n];
    for i in 1..n {
        let dt = time[i] - time[i - 1];
        out[i] = out[i - 1] + 0.5 * (values[i - 1] + values[i]) * dt;
    }
    out
}

/// Finite-difference differentiation.
fn differentiate(time: &[f32], values: &[f32]) -> Vec<f32> {
    let n = time.len().min(values.len());
    if n < 2 {
        return vec![0.0; n];
    }
    let mut out = vec![0.0_f32; n];
    for i in 1..n {
        let dt = time[i] - time[i - 1];
        out[i] = if dt.abs() > 1e-12 {
            (values[i] - values[i - 1]) / dt
        } else {
            0.0
        };
    }
    out[0] = out.get(1).copied().unwrap_or(0.0);
    out
}

/// Transform raw channel data from its native `DataType` to the requested
/// `PlotDomain`, then scale to the requested `Unit`.
fn transform_channel(
    time: &[f32],
    raw: &[f32],
    native: &DataType,
    target: &PlotDomain,
    unit: &Unit,
) -> Vec<f32> {
    // Steps needed:  positive = integrate, negative = differentiate.
    let native_order = match native {
        DataType::Displacement  => 0_i32,
        DataType::Velocity      => 1,
        DataType::Acceleration  => 2,
    };
    let target_order = match target {
        PlotDomain::Displacement  => 0_i32,
        PlotDomain::Velocity      => 1,
        PlotDomain::Acceleration  => 2,
    };
    let diff = native_order - target_order; // >0 ⇒ integrate, <0 ⇒ differentiate

    let mut data = raw.to_vec();
    match diff {
        2  => { data = integrate_trap(time, &data); data = integrate_trap(time, &data); }
        1  => { data = integrate_trap(time, &data); }
        -1 => { data = differentiate(time, &data); }
        -2 => { data = differentiate(time, &data); data = differentiate(time, &data); }
        _  => {} // 0 — no transform
    }

    // Apply unit conversion.
    // The raw data is assumed to be in SI base units for its native domain
    // (m, m/s, m/s²).  To display in the requested unit we divide by its
    // SI factor (which converts *to* SI, so we invert).
    let si = unit.to_si_factor();
    if (si - 1.0).abs() > 1e-12 {
        for v in &mut data {
            *v /= si;
        }
    }

    data
}

// ─────────────────────────────────────────────────────────────────────────────
// Distinct line colours
// ─────────────────────────────────────────────────────────────────────────────

const LINE_COLORS: &[[u8; 3]] = &[
    [100, 200, 255],   // cyan
    [255, 140,  60],   // orange
    [120, 220, 120],   // green
    [255, 100, 150],   // pink
    [200, 160, 255],   // lavender
    [255, 220,  80],   // yellow
    [100, 255, 200],   // mint
    [255, 100, 100],   // red
];

fn line_color(index: usize) -> egui::Color32 {
    let c = LINE_COLORS[index % LINE_COLORS.len()];
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

// ─────────────────────────────────────────────────────────────────────────────
// UI
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_time_plot_window(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut TimePlotState,
    datasets: &[Dataset],
    channel_names: &[String],
    anim_time: f64,
) {
    egui::Window::new("Time Plot")
        .open(open)
        .default_size([700.0, 440.0])
        .resizable(true)
        .show(ctx, |ui| {
            // ── Empty-data guard ─────────────────────────────────────────
            if channel_names.is_empty() {
                ui.label(
                    egui::RichText::new("Import data to see plots")
                        .color(egui::Color32::from_rgb(120, 120, 140))
                        .italics(),
                );
                return;
            }

            // ── Controls row ─────────────────────────────────────────────
            ui.horizontal(|ui| {
                // Domain selector
                ui.label("Domain:");
                let domains = [
                    PlotDomain::Displacement,
                    PlotDomain::Velocity,
                    PlotDomain::Acceleration,
                ];
                egui::ComboBox::from_id_salt("tp_domain")
                    .selected_text(state.plot_domain.label())
                    .width(110.0)
                    .show_ui(ui, |ui| {
                        for d in &domains {
                            if ui.selectable_value(
                                &mut state.plot_domain,
                                d.clone(),
                                d.label(),
                            ).changed() {
                                // Reset unit to default for new domain
                                state.plot_unit =
                                    Unit::default_for(&state.plot_domain.as_data_type());
                            }
                        }
                    });

                ui.add_space(8.0);

                // Unit selector
                ui.label("Unit:");
                let unit_opts = Unit::options_for(&state.plot_domain.as_data_type());
                // Ensure current unit is valid
                if !unit_opts.contains(&state.plot_unit) {
                    state.plot_unit =
                        Unit::default_for(&state.plot_domain.as_data_type());
                }
                egui::ComboBox::from_id_salt("tp_unit")
                    .selected_text(state.plot_unit.label())
                    .width(72.0)
                    .show_ui(ui, |ui| {
                        for opt in unit_opts {
                            ui.selectable_value(
                                &mut state.plot_unit,
                                opt.clone(),
                                opt.label(),
                            );
                        }
                    });
            });

            ui.add_space(2.0);

            // ── Channel selector (grouped by file) ───────────────────────
            egui::CollapsingHeader::new("Channels")
                .default_open(true)
                .show(ui, |ui| {
                    // Quick select/deselect all
                    ui.horizontal(|ui| {
                        if ui.small_button("All").clicked() {
                            state.selected_channels =
                                (0..channel_names.len()).collect();
                        }
                        if ui.small_button("None").clicked() {
                            state.selected_channels.clear();
                        }
                    });

                    egui::ScrollArea::vertical()
                        .max_height(140.0)
                        .id_salt("tp_ch_scroll")
                        .show(ui, |ui| {
                            // Group channels by dataset file name
                            let mut current_file: Option<&str> = None;
                            for (i, qname) in channel_names.iter().enumerate() {
                                let (file, ch) = qname.split_once("::").unwrap_or(("", qname));

                                // Start a new file group when the file name changes
                                if current_file != Some(file) {
                                    current_file = Some(file);

                                    // Collect indices belonging to this file
                                    let file_indices: Vec<usize> = channel_names.iter()
                                        .enumerate()
                                        .filter(|(_, n)| n.starts_with(file) && n.get(file.len()..file.len()+2) == Some("::"))
                                        .map(|(idx, _)| idx)
                                        .collect();
                                    let all_on = file_indices.iter().all(|idx| state.selected_channels.contains(idx));

                                    ui.horizontal(|ui| {
                                        // File-level toggle
                                        let mut file_on = all_on;
                                        if ui.checkbox(&mut file_on, "").changed() {
                                            if file_on {
                                                for &idx in &file_indices {
                                                    if !state.selected_channels.contains(&idx) {
                                                        state.selected_channels.push(idx);
                                                    }
                                                }
                                            } else {
                                                state.selected_channels.retain(|x| !file_indices.contains(x));
                                            }
                                        }
                                        ui.label(
                                            egui::RichText::new(format!("📄 {}", file))
                                                .strong()
                                                .size(12.0),
                                        );
                                    });
                                }

                                // Individual channel checkbox (indented)
                                ui.horizontal(|ui| {
                                    ui.add_space(20.0);
                                    let mut on = state.selected_channels.contains(&i);
                                    if ui.checkbox(&mut on, ch).changed() {
                                        if on {
                                            if !state.selected_channels.contains(&i) {
                                                state.selected_channels.push(i);
                                            }
                                        } else {
                                            state.selected_channels.retain(|&x| x != i);
                                        }
                                    }
                                });
                            }
                        });
                });

            ui.separator();

            // ── Plot ─────────────────────────────────────────────────────
            let y_label = format!("{} ({})", state.plot_domain.label(), state.plot_unit.label());

            let plot = egui_plot::Plot::new("time_domain_plot")
                .height(ui.available_height().max(120.0))
                .x_axis_label("Time (s)")
                .y_axis_label(y_label)
                .legend(egui_plot::Legend::default())
                .allow_zoom(true)
                .allow_drag(true)
                .allow_scroll(true)
                .allow_boxed_zoom(true);

            plot.show(ui, |plot_ui| {
                // Plot each selected channel
                for (color_idx, &ch_idx) in state.selected_channels.iter().enumerate() {
                    let Some(qname) = channel_names.get(ch_idx) else {
                        continue;
                    };
                    let Some((file, ch)) = qname.split_once("::") else {
                        continue;
                    };
                    let Some(ds) = datasets.iter().find(|d| d.name == file) else {
                        continue;
                    };
                    let Some(raw) = ds.values.get(ch) else {
                        continue;
                    };
                    let native = ds
                        .channel_meta
                        .get(ch)
                        .map(|m| &m.data_type)
                        .cloned()
                        .unwrap_or_default();

                    let transformed = transform_channel(
                        &ds.time,
                        raw,
                        &native,
                        &state.plot_domain,
                        &state.plot_unit,
                    );

                    let points: Vec<[f64; 2]> = ds
                        .time
                        .iter()
                        .zip(transformed.iter())
                        .map(|(&t, &v)| [t as f64, v as f64])
                        .collect();

                    let color = line_color(color_idx);
                    let line = egui_plot::Line::new(egui_plot::PlotPoints::from(points))
                        .name(qname)
                        .color(color)
                        .width(1.5);
                    plot_ui.line(line);
                }

                // Vertical cursor at current animation time
                let cursor = egui_plot::VLine::new(anim_time)
                    .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 120))
                    .width(1.0)
                    .name("Playhead");
                plot_ui.vline(cursor);
            });
        });
}
