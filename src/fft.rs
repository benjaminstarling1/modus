use std::collections::HashMap;
use num_complex::Complex;
use rustfft::FftPlanner;
use crate::data::Dataset;

// ─────────────────────────────────────────────────────────────────────────────
// FFT result
// ─────────────────────────────────────────────────────────────────────────────

pub struct FftResult {
    pub freqs:      Vec<f32>,
    pub amplitudes: Vec<f32>,
    pub phases:     Vec<f32>,
}

/// Compute one-sided amplitude + phase spectrum.
pub fn compute_fft(time: &[f32], values: &[f32]) -> Option<FftResult> {
    let n = time.len().min(values.len());
    if n < 4 { return None; }

    let dt = (time.last().unwrap_or(&1.0) - time.first().unwrap_or(&0.0)) / (n as f32 - 1.0);
    if dt <= 0.0 { return None; }
    let fs = 1.0 / dt;  // sample rate

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(n);

    let mut buffer: Vec<Complex<f32>> = values.iter()
        .take(n)
        .map(|&v| Complex::new(v, 0.0))
        .collect();
    fft.process(&mut buffer);

    // One-sided spectrum: bins 0..=n/2
    let half = n / 2 + 1;
    let mut freqs      = Vec::with_capacity(half);
    let mut amplitudes = Vec::with_capacity(half);
    let mut phases     = Vec::with_capacity(half);

    for k in 0..half {
        let freq = k as f32 * fs / n as f32;
        let amp  = buffer[k].norm() / n as f32;
        // Double amplitude for non-DC, non-Nyquist bins
        let amp = if k > 0 && k < n / 2 { amp * 2.0 } else { amp };
        let phase = buffer[k].arg();  // radians

        freqs.push(freq);
        amplitudes.push(amp);
        phases.push(phase);
    }

    Some(FftResult { freqs, amplitudes, phases })
}

// ─────────────────────────────────────────────────────────────────────────────
// Filtering
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterMode {
    None,
    SingleFreq,
    LowPass,
    HighPass,
    BandPass,
    BandStop,
}

/// Apply a frequency-domain filter to time-domain values. Returns filtered values
/// (same length, same time axis). Does NOT integrate — caller decides.
pub fn apply_freq_filter(
    time: &[f32],
    values: &[f32],
    mode: FilterMode,
    f_lo: f32,
    f_hi: f32,
) -> Vec<f32> {
    let n = time.len().min(values.len());
    if n < 4 || mode == FilterMode::None {
        return values.to_vec();
    }

    let dt = (time.last().unwrap_or(&1.0) - time.first().unwrap_or(&0.0)) / (n as f32 - 1.0);
    let fs = 1.0 / dt;

    let mut planner = FftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n);
    let inv = planner.plan_fft_inverse(n);

    let mut buf: Vec<Complex<f32>> = values.iter()
        .take(n)
        .map(|&v| Complex::new(v, 0.0))
        .collect();
    fwd.process(&mut buf);

    // Zero bins that don't pass the filter
    for k in 0..n {
        let freq = if k <= n / 2 {
            k as f32 * fs / n as f32
        } else {
            (n - k) as f32 * fs / n as f32  // mirror for negative freqs
        };

        let keep = match mode {
            FilterMode::None      => true,
            FilterMode::SingleFreq | FilterMode::BandPass => freq >= f_lo && freq <= f_hi,
            FilterMode::LowPass   => freq <= f_hi,
            FilterMode::HighPass   => freq >= f_lo,
            FilterMode::BandStop   => freq < f_lo || freq > f_hi,
        };
        if !keep {
            buf[k] = Complex::new(0.0, 0.0);
        }
    }

    inv.process(&mut buf);

    // Normalise (IFFT in rustfft doesn't normalise)
    buf.iter().map(|c| c.re / n as f32).collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Animation modes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimMode {
    /// Use filtered time-domain data (shows decay, transients)
    TimeBased,
    /// Synthesise steady-state sine wave from FFT amplitude+phase (no decay)
    FreqBased,
}

/// For freq-based mode: extract amplitude & phase at `target_freq` from the
/// raw channel data, then sample `A * sin(2π * f * t + φ)`.
pub fn sample_freq_based(
    datasets: &[Dataset],
    qualified_channel: &str,
    target_freq: f32,
    t: f64,
) -> f32 {
    let Some((file, ch)) = qualified_channel.split_once("::") else { return 0.0 };
    let Some(ds) = datasets.iter().find(|d| d.name == file) else { return 0.0 };
    let Some(raw) = ds.values.get(ch) else { return 0.0 };

    let fft = match compute_fft(&ds.time, raw) {
        Some(f) => f,
        None => return 0.0,
    };

    // Find nearest bin to target_freq
    let bin = fft.freqs.iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            ((**a) - target_freq).abs().partial_cmp(&((**b) - target_freq).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let amp   = fft.amplitudes[bin];
    let phase = fft.phases[bin];

    // Apply unit conversion (SI factor) the same way Dataset does
    let meta = ds.channel_meta.get(ch);
    let si = meta.map(|m| m.unit.to_si_factor()).unwrap_or(1.0);

    amp * si * (2.0 * std::f32::consts::PI * target_freq * t as f32 + phase).sin()
}

// ─────────────────────────────────────────────────────────────────────────────
// Pane state
// ─────────────────────────────────────────────────────────────────────────────

pub struct FftPaneState {
    pub selected_channel: usize,  // 0 = none, 1..N = index into channel_names
    pub show_phase:       bool,
    pub filter_mode:      FilterMode,
    pub anim_mode:        AnimMode,
    pub freq_lo:          f32,    // lower bound of selection (Hz)
    pub freq_hi:          f32,    // upper bound of selection (Hz)
    pub single_freq:      f32,    // single clicked freq (Hz), 0 = none
    pub active:           bool,   // true when filter is applied to animation
    pub cached_fft:       Option<FftResult>,
    /// Filtered displacement per qualified-channel-name (only for time-based).
    pub filtered_displacements: HashMap<String, Vec<f32>>,
}

impl Default for FftPaneState {
    fn default() -> Self {
        Self {
            selected_channel: 0,
            show_phase:       false,
            filter_mode:      FilterMode::None,
            anim_mode:        AnimMode::TimeBased,
            freq_lo:          0.0,
            freq_hi:          0.0,
            single_freq:      0.0,
            active:           false,
            cached_fft:       None,
            filtered_displacements: HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UI
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_fft_panel(
    ui: &mut egui::Ui,
    state: &mut FftPaneState,
    datasets: &[Dataset],
    channel_names: &[String],
) {
    ui.label(egui::RichText::new("FFT Analysis").size(16.0).strong());
    ui.separator();

    if channel_names.is_empty() {
        ui.label(
            egui::RichText::new("Import data to see FFT")
                .color(egui::Color32::from_rgb(120, 120, 140))
                .italics(),
        );
        return;
    }

    // ── Channel selector ─────────────────────────────────────────────────
    let ch_label = if state.selected_channel > 0 && state.selected_channel <= channel_names.len() {
        channel_names[state.selected_channel - 1].as_str()
    } else {
        "— Select channel —"
    };

    let mut changed = false;
    egui::ComboBox::from_id_salt("fft_ch_sel")
        .selected_text(ch_label)
        .width(ui.available_width() - 8.0)
        .show_ui(ui, |ui| {
            for (i, name) in channel_names.iter().enumerate() {
                if ui.selectable_value(&mut state.selected_channel, i + 1, name).changed() {
                    changed = true;
                }
            }
        });

    if changed {
        state.cached_fft = None;
        state.active = false;
        state.filtered_displacements.clear();
    }

    // ── Compute FFT if needed ────────────────────────────────────────────
    if state.cached_fft.is_none() && state.selected_channel > 0 {
        if let Some(qname) = channel_names.get(state.selected_channel - 1) {
            if let Some((file, ch)) = qname.split_once("::") {
                if let Some(ds) = datasets.iter().find(|d| d.name == file) {
                    if let Some(raw) = ds.values.get(ch) {
                        state.cached_fft = compute_fft(&ds.time, raw);
                    }
                }
            }
        }
    }

    let Some(ref fft) = state.cached_fft else {
        ui.label("No FFT data");
        return;
    };
    if fft.freqs.is_empty() { return; }

    // ── Amplitude plot ───────────────────────────────────────────────────
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Amplitude").size(11.0).strong());

    let plot_height = if state.show_phase { 80.0 } else { 120.0 };
    let (plot_rect, plot_resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), plot_height),
        egui::Sense::click_and_drag(),
    );

    let painter = ui.painter_at(plot_rect);
    painter.rect_filled(plot_rect, 2.0, egui::Color32::from_rgb(20, 20, 28));

    let max_freq = *fft.freqs.last().unwrap_or(&1.0);
    let max_amp  = fft.amplitudes.iter()
        .skip(1)  // skip DC
        .cloned()
        .fold(0.0_f32, f32::max)
        .max(1e-12);

    // Use log scale for amplitude
    let log_max = max_amp.log10();
    let log_min = (max_amp * 1e-4).log10();  // 4 decades range

    let freq_to_x = |f: f32| -> f32 {
        plot_rect.left() + (f / max_freq) * plot_rect.width()
    };
    let amp_to_y = |a: f32| -> f32 {
        let la = a.max(1e-20).log10();
        let t = ((la - log_min) / (log_max - log_min)).clamp(0.0, 1.0);
        plot_rect.bottom() - t * plot_rect.height()
    };

    // Draw amplitude line
    let points: Vec<egui::Pos2> = fft.freqs.iter().zip(fft.amplitudes.iter())
        .skip(1)
        .map(|(&f, &a)| egui::pos2(freq_to_x(f), amp_to_y(a)))
        .collect();

    for pair in points.windows(2) {
        painter.line_segment(
            [pair[0], pair[1]],
            egui::Stroke::new(1.2, egui::Color32::from_rgb(100, 200, 255)),
        );
    }

    // ── Frequency selection overlay ──────────────────────────────────────
    // single freq marker
    if state.single_freq > 0.0 {
        let x = freq_to_x(state.single_freq);
        painter.line_segment(
            [egui::pos2(x, plot_rect.top()), egui::pos2(x, plot_rect.bottom())],
            egui::Stroke::new(1.5, egui::Color32::from_rgba_unmultiplied(255, 200, 50, 200)),
        );
    }
    // band selection
    if state.freq_lo > 0.0 && state.freq_hi > state.freq_lo {
        let x0 = freq_to_x(state.freq_lo);
        let x1 = freq_to_x(state.freq_hi);
        painter.rect_filled(
            egui::Rect::from_x_y_ranges(x0..=x1, plot_rect.top()..=plot_rect.bottom()),
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 200, 50, 30),
        );
    }

    // ── Interaction: click = single freq, drag = band ────────────────────
    let x_to_freq = |x: f32| -> f32 {
        ((x - plot_rect.left()) / plot_rect.width() * max_freq).clamp(0.0, max_freq)
    };

    // Persistent drag anchor
    let drag_id = egui::Id::new("fft_plot_drag_start");
    if plot_resp.drag_started() {
        if let Some(pos) = plot_resp.interact_pointer_pos() {
            ui.ctx().data_mut(|d| d.insert_temp(drag_id, pos.x));
        }
    }
    if plot_resp.drag_stopped() {
        let anchor: Option<f32> = ui.ctx().data(|d| d.get_temp(drag_id));
        ui.ctx().data_mut(|d| d.remove::<f32>(drag_id));
        if let (Some(start_x), Some(end_pos)) = (anchor, plot_resp.interact_pointer_pos()) {
            let f0 = x_to_freq(start_x);
            let f1 = x_to_freq(end_pos.x);
            if (f1 - f0).abs() > max_freq * 0.005 {
                // Band selection
                state.freq_lo = f0.min(f1);
                state.freq_hi = f0.max(f1);
                state.single_freq = 0.0;
                if state.filter_mode == FilterMode::None || state.filter_mode == FilterMode::SingleFreq {
                    state.filter_mode = FilterMode::BandPass;
                }
            } else {
                // Click = single freq
                let f = x_to_freq(end_pos.x);
                // Snap to nearest bin
                let bin = fft.freqs.iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| ((**a) - f).abs().partial_cmp(&((**b) - f).abs()).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                state.single_freq = fft.freqs[bin];
                state.freq_lo = 0.0;
                state.freq_hi = 0.0;
                state.filter_mode = FilterMode::SingleFreq;
            }
            state.active = false;
            state.filtered_displacements.clear();
        }
    } else if plot_resp.clicked() {
        if let Some(pos) = plot_resp.interact_pointer_pos() {
            let f = x_to_freq(pos.x);
            let bin = fft.freqs.iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| ((**a) - f).abs().partial_cmp(&((**b) - f).abs()).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(i, _)| i)
                .unwrap_or(0);
            state.single_freq = fft.freqs[bin];
            state.freq_lo = 0.0;
            state.freq_hi = 0.0;
            state.filter_mode = FilterMode::SingleFreq;
            state.active = false;
            state.filtered_displacements.clear();
        }
    }

    // Show hover freq
    if let Some(hpos) = plot_resp.hover_pos() {
        let hf = x_to_freq(hpos.x);
        painter.line_segment(
            [egui::pos2(hpos.x, plot_rect.top()), egui::pos2(hpos.x, plot_rect.bottom())],
            egui::Stroke::new(0.5, egui::Color32::from_rgb(200, 200, 200)),
        );
        painter.text(
            egui::pos2(hpos.x + 4.0, plot_rect.top() + 4.0),
            egui::Align2::LEFT_TOP,
            format!("{:.1} Hz", hf),
            egui::FontId::proportional(10.0),
            egui::Color32::from_rgb(200, 200, 200),
        );
    }

    // ── Phase plot ───────────────────────────────────────────────────────
    ui.checkbox(&mut state.show_phase, "Show Phase");

    if state.show_phase {
        ui.label(egui::RichText::new("Phase").size(11.0).strong());
        let (phase_rect, _phase_resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), 60.0),
            egui::Sense::hover(),
        );
        let pp = ui.painter_at(phase_rect);
        pp.rect_filled(phase_rect, 2.0, egui::Color32::from_rgb(20, 20, 28));

        let phase_points: Vec<egui::Pos2> = fft.freqs.iter().zip(fft.phases.iter())
            .skip(1)
            .map(|(&f, &p)| {
                let x = freq_to_x(f);
                let t = (p / std::f32::consts::PI + 1.0) * 0.5;  // map [-π, π] to [0, 1]
                let y = phase_rect.bottom() - t * phase_rect.height();
                egui::pos2(x, y)
            })
            .collect();

        for pair in phase_points.windows(2) {
            pp.line_segment(
                [pair[0], pair[1]],
                egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 130, 255)),
            );
        }
    }

    // ── Filter mode controls ─────────────────────────────────────────────
    ui.add_space(4.0);
    ui.separator();

    // Show current selection info
    if state.single_freq > 0.0 {
        ui.label(
            egui::RichText::new(format!("Selected: {:.2} Hz", state.single_freq))
                .color(egui::Color32::from_rgb(255, 200, 50))
                .size(11.0),
        );
    } else if state.freq_lo > 0.0 && state.freq_hi > state.freq_lo {
        ui.label(
            egui::RichText::new(format!("Band: {:.1} – {:.1} Hz", state.freq_lo, state.freq_hi))
                .color(egui::Color32::from_rgb(255, 200, 50))
                .size(11.0),
        );
    }

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Filter:").size(11.0));
        let modes: &[(FilterMode, &str)] = &[
            (FilterMode::None, "Off"),
            (FilterMode::SingleFreq, "Single"),
            (FilterMode::LowPass, "LP"),
            (FilterMode::HighPass, "HP"),
            (FilterMode::BandPass, "BP"),
            (FilterMode::BandStop, "BS"),
        ];
        for &(mode, label) in modes {
            if ui.selectable_label(state.filter_mode == mode, label).clicked() {
                state.filter_mode = mode;
                state.active = false;
                state.filtered_displacements.clear();
            }
        }
    });

    // Animation mode (only visible for SingleFreq)
    if state.filter_mode == FilterMode::SingleFreq && state.single_freq > 0.0 {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Animate:").size(11.0));
            ui.selectable_value(&mut state.anim_mode, AnimMode::FreqBased, "Frequency");
            ui.selectable_value(&mut state.anim_mode, AnimMode::TimeBased, "Time");
        });
    }

    // Apply button
    let can_apply = state.filter_mode != FilterMode::None
        && (state.single_freq > 0.0 || (state.freq_lo > 0.0 && state.freq_hi > state.freq_lo));

    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(can_apply, egui::Button::new(format!("{}  Apply", egui_phosphor::regular::CHECK))).clicked() {
            state.active = true;
            // For time-based: pre-compute filtered displacements for all channels
            if state.anim_mode == AnimMode::TimeBased || state.filter_mode != FilterMode::SingleFreq {
                state.filtered_displacements.clear();
                let (flo, fhi) = resolve_freq_bounds(state);
                for qname in channel_names {
                    if let Some((file, ch)) = qname.split_once("::") {
                        if let Some(ds) = datasets.iter().find(|d| d.name == file) {
                            if let Some(raw) = ds.values.get(ch) {
                                let filtered_raw = apply_freq_filter(
                                    &ds.time, raw,
                                    state.filter_mode,
                                    flo, fhi,
                                );
                                // Apply SI conversion + integration from meta
                                let meta = ds.channel_meta.get(ch);
                                let si = meta.map(|m| m.unit.to_si_factor()).unwrap_or(1.0);
                                let si_vals: Vec<f32> = filtered_raw.iter().map(|v| v * si).collect();
                                let disp = match meta.map(|m| &m.data_type) {
                                    Some(crate::data::DataType::Velocity) =>
                                        integrate_trap(&ds.time, &si_vals),
                                    Some(crate::data::DataType::Acceleration) => {
                                        let vel = integrate_trap(&ds.time, &si_vals);
                                        integrate_trap(&ds.time, &vel)
                                    }
                                    _ => si_vals,
                                };
                                let offset = disp.first().copied().unwrap_or(0.0);
                                let disp: Vec<f32> = disp.iter().map(|v| v - offset).collect();
                                state.filtered_displacements.insert(qname.clone(), disp);
                            }
                        }
                    }
                }
            }
        }
        if state.active {
            ui.label(
                egui::RichText::new("● Active")
                    .color(egui::Color32::from_rgb(80, 220, 120))
                    .size(11.0),
            );
        }
        if state.active && ui.small_button("✕ Clear").clicked() {
            state.active = false;
            state.filtered_displacements.clear();
        }
    });
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_freq_bounds(state: &FftPaneState) -> (f32, f32) {
    match state.filter_mode {
        FilterMode::SingleFreq => {
            // Narrow band around the single frequency (±2% of freq or ±0.5 Hz min)
            let bw = (state.single_freq * 0.02).max(0.5);
            (state.single_freq - bw, state.single_freq + bw)
        }
        FilterMode::LowPass => {
            // Cutoff = the single freq if set, otherwise freq_hi from band selection
            let cutoff = if state.single_freq > 0.0 { state.single_freq } else { state.freq_hi };
            (0.0, cutoff)
        }
        FilterMode::HighPass => {
            // Cutoff = the single freq if set, otherwise freq_lo from band selection
            let cutoff = if state.single_freq > 0.0 { state.single_freq } else { state.freq_lo };
            (cutoff, f32::MAX)
        }
        _ => {
            // BandPass, BandStop, None — use the drag range directly
            (state.freq_lo, state.freq_hi)
        }
    }
}

/// Trapezoidal cumulative integration.
fn integrate_trap(time: &[f32], values: &[f32]) -> Vec<f32> {
    let n = time.len().min(values.len());
    let mut out = vec![0.0f32; n];
    for i in 1..n {
        let dt = time[i] - time[i - 1];
        out[i] = out[i - 1] + 0.5 * (values[i - 1] + values[i]) * dt;
    }
    out
}

/// Sample filtered displacement for time-based mode (interpolation at time t).
pub fn sample_filtered(
    filtered: &HashMap<String, Vec<f32>>,
    time_axis: &[f32],
    qualified: &str,
    t: f64,
) -> f32 {
    let disp = match filtered.get(qualified) {
        Some(d) if !d.is_empty() => d,
        _ => return 0.0,
    };
    let ti = time_axis;
    if ti.len() < 2 { return disp[0]; }
    let t = t as f32;
    let n = ti.len();
    if t <= ti[0]     { return disp[0]; }
    if t >= ti[n - 1] { return disp[n - 1]; }
    let idx = ti.partition_point(|&v| v <= t).saturating_sub(1).min(n - 2);
    let t0 = ti[idx];
    let t1 = ti[idx + 1];
    let frac = if (t1 - t0).abs() < 1e-12 { 0.0 } else { (t - t0) / (t1 - t0) };
    disp[idx] * (1.0 - frac) + disp[idx + 1] * frac
}
