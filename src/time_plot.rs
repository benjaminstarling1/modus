use crate::data::{Dataset, DataType, Unit};
use crate::fft::compute_fft;
use crate::table::Row;

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
// Tab selector
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlotTab {
    Time,
    Fft,
    Spectrogram,
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

pub struct TimePlotState {
    pub selected_channels: Vec<usize>,   // indices into channel_names
    pub plot_domain:       PlotDomain,
    pub plot_unit:         Unit,
    pub tab:               PlotTab,
    /// FFT: show log scale for amplitude axis
    pub fft_log_scale:     bool,
    /// Spectrogram: window size in samples
    pub spec_window:       usize,
    /// Spectrogram: hop size in samples
    pub spec_hop:          usize,
    /// Spectrogram: which selected-channel index to display (for spectrogram, one at a time)
    pub spec_channel_idx:  usize,
    /// Set to true by the Plot UI to tell app.rs to activate Select+Node mode.
    pub activate_select:   bool,
}

impl Default for TimePlotState {
    fn default() -> Self {
        Self {
            selected_channels: Vec::new(),
            plot_domain:       PlotDomain::Displacement,
            plot_unit:         Unit::Meter,
            tab:               PlotTab::Time,
            fft_log_scale:     true,
            spec_window:       64,
            spec_hop:          16,
            spec_channel_idx:  0,
            activate_select:   false,
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
    let diff = native_order - target_order;

    let mut data = raw.to_vec();
    match diff {
        2  => { data = integrate_trap(time, &data); data = integrate_trap(time, &data); }
        1  => { data = integrate_trap(time, &data); }
        -1 => { data = differentiate(time, &data); }
        -2 => { data = differentiate(time, &data); data = differentiate(time, &data); }
        _  => {}
    }

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
// Short-Time Fourier Transform for spectrogram
// ─────────────────────────────────────────────────────────────────────────────

struct StftResult {
    /// Time centers for each window (seconds).
    times: Vec<f32>,
    /// Frequency bins (Hz), length = window_size/2 + 1.
    freqs: Vec<f32>,
    /// 2D magnitude: magnitudes[time_idx][freq_idx].
    magnitudes: Vec<Vec<f32>>,
}

fn compute_stft(time: &[f32], values: &[f32], window_size: usize, hop: usize) -> Option<StftResult> {
    let n = time.len().min(values.len());
    if n < window_size || window_size < 4 { return None; }

    let dt = (time.last().unwrap_or(&1.0) - time.first().unwrap_or(&0.0)) / (n as f32 - 1.0);
    if dt <= 0.0 { return None; }
    let fs = 1.0 / dt;
    let half = window_size / 2 + 1;

    // Hann window
    let hann: Vec<f32> = (0..window_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (window_size as f32 - 1.0)).cos()))
        .collect();

    let mut planner = rustfft::FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(window_size);

    let mut times_out = Vec::new();
    let mut magnitudes_out = Vec::new();

    let mut start = 0usize;
    while start + window_size <= n {
        // Center time of this window
        let center_idx = start + window_size / 2;
        let t_center = if center_idx < n { time[center_idx] } else { *time.last().unwrap() };
        times_out.push(t_center);

        // Apply window and FFT
        let mut buffer: Vec<num_complex::Complex<f32>> = (0..window_size)
            .map(|i| num_complex::Complex::new(values[start + i] * hann[i], 0.0))
            .collect();
        fft.process(&mut buffer);

        // One-sided magnitude
        let mags: Vec<f32> = (0..half)
            .map(|k| {
                let amp = buffer[k].norm() / window_size as f32;
                if k > 0 && k < window_size / 2 { amp * 2.0 } else { amp }
            })
            .collect();
        magnitudes_out.push(mags);

        start += hop;
    }

    let freqs: Vec<f32> = (0..half).map(|k| k as f32 * fs / window_size as f32).collect();

    Some(StftResult { times: times_out, freqs, magnitudes: magnitudes_out })
}

// ─────────────────────────────────────────────────────────────────────────────
// Resolve channel data helper
// ─────────────────────────────────────────────────────────────────────────────

struct ChannelData<'a> {
    qname: &'a str,
    _ch:   &'a str,
    time:  &'a [f32],
    raw:   &'a [f32],
    native: DataType,
}

fn resolve_channel<'a>(
    ch_idx: usize,
    channel_names: &'a [String],
    datasets: &'a [Dataset],
) -> Option<ChannelData<'a>> {
    let qname = channel_names.get(ch_idx)?;
    let (file, ch) = qname.split_once("::")?;
    let ds = datasets.iter().find(|d| d.name == file)?;
    let raw = ds.values.get(ch)?;
    let native = ds.channel_meta.get(ch).map(|m| m.data_type.clone()).unwrap_or_default();
    Some(ChannelData { qname, _ch: ch, time: &ds.time, raw, native })
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
    rows: &[Row],
    anim_time: f64,
) {
    egui::Window::new("Plot")
        .open(open)
        .default_size([700.0, 480.0])
        .min_size([400.0, 300.0])
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

                // ── Tab bar ──────────────────────────────────────────────────
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.tab, PlotTab::Time,        "Time");
                    ui.selectable_value(&mut state.tab, PlotTab::Fft,         "FFT");
                    ui.selectable_value(&mut state.tab, PlotTab::Spectrogram, "Spectrogram");

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // "Select Nodes" — activates the viewport multi-select tool
                    if ui.button(format!("{}  Select Nodes", egui_phosphor::regular::SELECTION_PLUS))
                        .on_hover_text("Activate node multi-select in the viewport")
                        .clicked()
                    {
                        state.activate_select = true;
                    }

                    // "Apply Selection" — reads currently selected nodes
                    let n_sel = rows.iter().filter(|r| r.selected).count();
                    let apply_label = if n_sel > 0 {
                        format!("Apply ({} nodes)", n_sel)
                    } else {
                        "Apply".to_string()
                    };
                    if ui.add_enabled(n_sel > 0, egui::Button::new(apply_label))
                        .on_hover_text("Plot channels for the selected nodes")
                        .clicked()
                    {
                        state.selected_channels.clear();
                        for row in rows {
                            if !row.selected { continue; }
                            for idx in [row.dx, row.dy, row.dz, row.rx, row.ry, row.rz] {
                                if idx > 0 && idx <= channel_names.len()
                                    && !state.selected_channels.contains(&(idx - 1))
                                {
                                    state.selected_channels.push(idx - 1);
                                }
                            }
                        }
                    }
                });

                ui.separator();

                // ── Controls row ─────────────────────────────────────────────
                if state.tab == PlotTab::Time || state.tab == PlotTab::Fft {
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
                                        state.plot_unit =
                                            Unit::default_for(&state.plot_domain.as_data_type());
                                    }
                                }
                            });

                        ui.add_space(8.0);

                        // Unit selector
                        ui.label("Unit:");
                        let unit_opts = Unit::options_for(&state.plot_domain.as_data_type());
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

                        if state.tab == PlotTab::Fft {
                            ui.add_space(8.0);
                            ui.checkbox(&mut state.fft_log_scale, "Log scale");
                        }
                    });
                    ui.add_space(2.0);
                }

                // Spectrogram controls
                if state.tab == PlotTab::Spectrogram {
                    ui.horizontal(|ui| {
                        ui.label("Window:");
                        ui.add(egui::DragValue::new(&mut state.spec_window)
                            .range(16..=512).speed(1));
                        ui.add_space(8.0);
                        ui.label("Hop:");
                        ui.add(egui::DragValue::new(&mut state.spec_hop)
                            .range(1..=256).speed(1));
                        ui.add_space(8.0);

                        // Channel selector for spectrogram (one at a time)
                        if !state.selected_channels.is_empty() {
                            // Clamp index when selection changes
                            if state.spec_channel_idx >= state.selected_channels.len() {
                                state.spec_channel_idx = 0;
                            }
                            ui.label("Channel:");
                            let current_label = state.selected_channels.get(state.spec_channel_idx)
                                .and_then(|&idx| channel_names.get(idx))
                                .map(|s| s.as_str())
                                .unwrap_or("—");
                            egui::ComboBox::from_id_salt("spec_ch_sel")
                                .selected_text(current_label)
                                .width(200.0)
                                .height(300.0)
                                .show_ui(ui, |ui| {
                                    for (i, &ch_idx) in state.selected_channels.iter().enumerate() {
                                        if let Some(name) = channel_names.get(ch_idx) {
                                            ui.selectable_value(&mut state.spec_channel_idx, i, name.as_str());
                                        }
                                    }
                                });
                        }
                    });
                    ui.add_space(2.0);
                }

                // ── Channel selector (grouped by file) ───────────────────────
                egui::CollapsingHeader::new("Channels")
                    .default_open(false)
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
                                let mut current_file: Option<&str> = None;
                                for (i, qname) in channel_names.iter().enumerate() {
                                    let (file, ch) = qname.split_once("::").unwrap_or(("", qname));

                                    if current_file != Some(file) {
                                        current_file = Some(file);
                                        let file_indices: Vec<usize> = channel_names.iter()
                                            .enumerate()
                                            .filter(|(_, n)| n.starts_with(file) && n.get(file.len()..file.len()+2) == Some("::"))
                                            .map(|(idx, _)| idx)
                                            .collect();
                                        let all_on = file_indices.iter().all(|idx| state.selected_channels.contains(idx));

                                        ui.horizontal(|ui| {
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

                // ── Plot area ────────────────────────────────────────────────
                match state.tab {
                    PlotTab::Time => show_time_tab(ui, state, datasets, channel_names, anim_time),
                    PlotTab::Fft  => show_fft_tab(ui, state, datasets, channel_names),
                    PlotTab::Spectrogram => show_spectrogram_tab(ui, state, datasets, channel_names, anim_time),
                }
        });
}

// ─────────────────────────────────────────────────────────────────────────────
// Time domain tab
// ─────────────────────────────────────────────────────────────────────────────

fn show_time_tab(
    ui: &mut egui::Ui,
    state: &TimePlotState,
    datasets: &[Dataset],
    channel_names: &[String],
    anim_time: f64,
) {
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
        for (color_idx, &ch_idx) in state.selected_channels.iter().enumerate() {
            let Some(cd) = resolve_channel(ch_idx, channel_names, datasets) else { continue };
            let transformed = transform_channel(cd.time, cd.raw, &cd.native, &state.plot_domain, &state.plot_unit);

            let points: Vec<[f64; 2]> = cd.time.iter()
                .zip(transformed.iter())
                .map(|(&t, &v)| [t as f64, v as f64])
                .collect();

            let color = line_color(color_idx);
            let line = egui_plot::Line::new(egui_plot::PlotPoints::from(points))
                .name(cd.qname)
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
}

// ─────────────────────────────────────────────────────────────────────────────
// FFT tab
// ─────────────────────────────────────────────────────────────────────────────

fn show_fft_tab(
    ui: &mut egui::Ui,
    state: &TimePlotState,
    datasets: &[Dataset],
    channel_names: &[String],
) {
    let plot = egui_plot::Plot::new("fft_plot")
        .height(ui.available_height().max(120.0))
        .x_axis_label("Frequency (Hz)")
        .y_axis_label(format!("Amplitude ({})", state.plot_unit.label()))
        .legend(egui_plot::Legend::default())
        .allow_zoom(true)
        .allow_drag(true)
        .allow_scroll(true)
        .allow_boxed_zoom(true);

    plot.show(ui, |plot_ui| {
        for (color_idx, &ch_idx) in state.selected_channels.iter().enumerate() {
            let Some(cd) = resolve_channel(ch_idx, channel_names, datasets) else { continue };

            // Transform to the requested domain first
            let transformed = transform_channel(cd.time, cd.raw, &cd.native, &state.plot_domain, &state.plot_unit);

            // Compute FFT
            let Some(fft) = compute_fft(cd.time, &transformed) else { continue };

            let points: Vec<[f64; 2]> = fft.freqs.iter()
                .zip(fft.amplitudes.iter())
                .skip(1) // skip DC
                .map(|(&f, &a)| {
                    let y = if state.fft_log_scale {
                        (a.max(1e-20) as f64).log10()
                    } else {
                        a as f64
                    };
                    [f as f64, y]
                })
                .collect();

            let color = line_color(color_idx);
            let line = egui_plot::Line::new(egui_plot::PlotPoints::from(points))
                .name(cd.qname)
                .color(color)
                .width(1.5);
            plot_ui.line(line);
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Spectrogram tab  (painted heat-map)
// ─────────────────────────────────────────────────────────────────────────────

fn show_spectrogram_tab(
    ui: &mut egui::Ui,
    state: &TimePlotState,
    datasets: &[Dataset],
    channel_names: &[String],
    anim_time: f64,
) {
    // Pick the channel to display
    let ch_idx = state.selected_channels
        .get(state.spec_channel_idx)
        .copied()
        .or_else(|| state.selected_channels.first().copied());

    let Some(ci) = ch_idx else {
        ui.label(
            egui::RichText::new("Select a channel to view spectrogram")
                .color(egui::Color32::from_rgb(120, 120, 140))
                .italics(),
        );
        return;
    };

    let Some(cd) = resolve_channel(ci, channel_names, datasets) else {
        ui.label("Channel data not available");
        return;
    };

    let stft = compute_stft(cd.time, cd.raw, state.spec_window, state.spec_hop.max(1));
    let Some(stft) = stft else {
        ui.label("Not enough data for spectrogram with current window size");
        return;
    };

    // Info label
    ui.label(
        egui::RichText::new(format!("Spectrogram: {}", cd.qname))
            .size(11.0).strong(),
    );

    // Allocate the plot area
    let avail = ui.available_size();
    let plot_height = avail.y.max(120.0);
    let plot_width = avail.x.max(200.0);
    let (rect, _resp) = ui.allocate_exact_size(
        egui::vec2(plot_width, plot_height),
        egui::Sense::hover(),
    );
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, egui::Color32::from_rgb(10, 10, 18));

    let n_time = stft.times.len();
    let n_freq = stft.freqs.len();
    if n_time == 0 || n_freq == 0 { return; }

    // Find global max for normalization (skip DC bin)
    let global_max = stft.magnitudes.iter()
        .flat_map(|row| row.iter().skip(1))
        .cloned()
        .fold(0.0_f32, f32::max)
        .max(1e-20);

    // Pixel dimensions
    let px_w = rect.width() / n_time as f32;
    let px_h = rect.height() / (n_freq - 1).max(1) as f32;

    for (ti, mags) in stft.magnitudes.iter().enumerate() {
        let x0 = rect.left() + ti as f32 * px_w;
        for fi in 1..n_freq {
            let y_bottom = rect.bottom() - fi as f32 * px_h;
            let amp = mags[fi];
            // Log-scale normalization
            let log_val = (amp / global_max).max(1e-6).log10();
            let normalized = ((log_val + 6.0) / 6.0).clamp(0.0, 1.0); // -60 dB range

            let color = viridis_color(normalized);
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(x0, y_bottom - px_h),
                    egui::vec2(px_w + 0.5, px_h + 0.5),
                ),
                0.0,
                color,
            );
        }
    }

    // Draw playhead
    let t_min = stft.times.first().copied().unwrap_or(0.0);
    let t_max = stft.times.last().copied().unwrap_or(1.0);
    let t_range = (t_max - t_min).max(1e-9);
    let playhead_x = rect.left() + ((anim_time as f32 - t_min) / t_range) * rect.width();
    if playhead_x >= rect.left() && playhead_x <= rect.right() {
        painter.line_segment(
            [egui::pos2(playhead_x, rect.top()), egui::pos2(playhead_x, rect.bottom())],
            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 160)),
        );
    }

    // Axis labels
    let max_freq = stft.freqs.last().copied().unwrap_or(1.0);
    // Frequency axis (left side)
    for &frac in &[0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let freq = max_freq * frac;
        let y = rect.bottom() - frac * rect.height();
        painter.text(
            egui::pos2(rect.left() + 2.0, y),
            egui::Align2::LEFT_CENTER,
            format!("{:.0} Hz", freq),
            egui::FontId::proportional(9.0),
            egui::Color32::from_rgb(180, 180, 180),
        );
    }
    // Time axis (bottom)
    for &frac in &[0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let t = t_min + t_range * frac;
        let x = rect.left() + frac * rect.width();
        painter.text(
            egui::pos2(x, rect.bottom() - 2.0),
            egui::Align2::CENTER_BOTTOM,
            format!("{:.2}s", t),
            egui::FontId::proportional(9.0),
            egui::Color32::from_rgb(180, 180, 180),
        );
    }
}

/// Approximate viridis colormap from normalized value [0, 1].
fn viridis_color(t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    // Simplified 5-stop viridis: dark purple → blue → teal → green → yellow
    let (r, g, b) = if t < 0.25 {
        let s = t / 0.25;
        (
            68.0 * (1.0 - s) + 59.0 * s,
            1.0 * (1.0 - s) + 82.0 * s,
            84.0 * (1.0 - s) + 139.0 * s,
        )
    } else if t < 0.5 {
        let s = (t - 0.25) / 0.25;
        (
            59.0 * (1.0 - s) + 33.0 * s,
            82.0 * (1.0 - s) + 145.0 * s,
            139.0 * (1.0 - s) + 140.0 * s,
        )
    } else if t < 0.75 {
        let s = (t - 0.5) / 0.25;
        (
            33.0 * (1.0 - s) + 94.0 * s,
            145.0 * (1.0 - s) + 201.0 * s,
            140.0 * (1.0 - s) + 98.0 * s,
        )
    } else {
        let s = (t - 0.75) / 0.25;
        (
            94.0 * (1.0 - s) + 253.0 * s,
            201.0 * (1.0 - s) + 231.0 * s,
            98.0 * (1.0 - s) + 37.0 * s,
        )
    };
    egui::Color32::from_rgb(r as u8, g as u8, b as u8)
}
