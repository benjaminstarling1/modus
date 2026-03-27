use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum ExportFormat {
    PngSequence,
    Mp4,
}

impl ExportFormat {
    fn label(&self) -> &'static str {
        match self {
            Self::PngSequence => "PNG Sequence",
            Self::Mp4         => "MP4 (requires ffmpeg)",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExportPhase {
    /// Dialog shown, user configures settings.
    Idle,
    /// Stepping through frames — `frame` rendered, screenshot requested.
    Capturing { frame: usize, total: usize, waiting: bool },
    /// Running ffmpeg to encode MP4 (background).
    Encoding,
    /// Finished — show result message.
    Done { message: String },
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

pub struct ExportVideoState {
    pub format:     ExportFormat,
    pub output_dir: Option<PathBuf>,     // PNG: directory to save into
    pub output_file: Option<PathBuf>,    // MP4: target file
    pub phase:      ExportPhase,
    pub fps:        f32,

    // Saved animation state (restored after export)
    pub saved_time:    f64,
    pub saved_playing: bool,

    // Viewport rect for cropping (set by caller each frame)
    pub viewport_rect: Option<egui::Rect>,

    // Frame storage directory (temp for MP4, chosen for PNG)
    pub frame_dir: Option<PathBuf>,

    // ffmpeg detection
    pub ffmpeg_available: bool,
}

impl Default for ExportVideoState {
    fn default() -> Self {
        Self {
            format:      ExportFormat::PngSequence,
            output_dir:  None,
            output_file: None,
            phase:       ExportPhase::Idle,
            fps:         30.0,
            saved_time:    0.0,
            saved_playing: false,
            viewport_rect: None,
            frame_dir:   None,
            ffmpeg_available: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Actions returned to the caller
// ─────────────────────────────────────────────────────────────────────────────

pub enum ExportAction {
    None,
    /// Begin the capture loop — caller should save anim state and set time=0.
    StartCapture { total_frames: usize },
    /// Cancel the export.
    Cancel,
}

// ─────────────────────────────────────────────────────────────────────────────
// Dialog UI
// ─────────────────────────────────────────────────────────────────────────────

pub fn show_export_video_window(
    ctx:   &egui::Context,
    open:  &mut bool,
    state: &mut ExportVideoState,
    duration: f64,
) -> ExportAction {
    if !*open { return ExportAction::None; }

    let mut action = ExportAction::None;

    egui::Window::new("Export Animation")
        .resizable(true)
        .default_width(400.0)
        .min_width(340.0)
        .collapsible(false)
        .open(open)
        .show(ctx, |ui| {
            match &state.phase {
                ExportPhase::Idle => {
                    action = idle_ui(ui, state, duration);
                }
                ExportPhase::Capturing { frame, total, .. } => {
                    let f = *frame;
                    let t = *total;
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("Capturing frame {} / {}…", f + 1, t))
                            .size(14.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    let progress = if t > 0 { f as f32 / t as f32 } else { 0.0 };
                    ui.add(egui::ProgressBar::new(progress).animate(true));
                    ui.add_space(8.0);
                    if ui.button("Cancel").clicked() {
                        action = ExportAction::Cancel;
                    }
                }
                ExportPhase::Encoding => {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Encoding MP4 with ffmpeg…")
                            .size(14.0)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.add(egui::ProgressBar::new(1.0).animate(true));
                    ui.add_space(8.0);
                }
                ExportPhase::Done { message } => {
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(message)
                            .size(13.0)
                            .color(if message.starts_with(egui_phosphor::regular::CHECK) {
                                egui::Color32::from_rgb(80, 210, 120)
                            } else {
                                egui::Color32::from_rgb(220, 100, 80)
                            }),
                    );
                    ui.add_space(8.0);
                    if ui.button("OK").clicked() {
                        state.phase = ExportPhase::Idle;
                    }
                }
            }
        });

    action
}

fn idle_ui(
    ui:       &mut egui::Ui,
    state:    &mut ExportVideoState,
    duration: f64,
) -> ExportAction {
    // Detect ffmpeg availability each time
    state.ffmpeg_available = detect_ffmpeg();

    // Format selector
    ui.horizontal(|ui| {
        ui.label("Format:");
        egui::ComboBox::from_id_salt("export_format")
            .selected_text(state.format.label())
            .width(180.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(
                    &mut state.format, ExportFormat::PngSequence,
                    ExportFormat::PngSequence.label(),
                );
                if state.ffmpeg_available {
                    ui.selectable_value(
                        &mut state.format, ExportFormat::Mp4,
                        ExportFormat::Mp4.label(),
                    );
                } else {
                    ui.add_enabled(false, egui::Button::new("MP4 (ffmpeg not found)"));
                }
            });
    });

    // FPS
    ui.horizontal(|ui| {
        ui.label("FPS:");
        ui.add(
            egui::DragValue::new(&mut state.fps)
                .range(1.0..=120.0)
                .speed(1.0),
        );
    });

    let total_frames = if duration > 0.0 {
        (duration * state.fps as f64).ceil() as usize
    } else {
        0
    };

    ui.label(
        egui::RichText::new(format!(
            "Duration: {:.3}s  •  {} frames",
            duration, total_frames,
        ))
        .color(egui::Color32::from_rgb(140, 160, 190)),
    );

    ui.add_space(4.0);

    // Output path
    match state.format {
        ExportFormat::PngSequence => {
            ui.horizontal(|ui| {
                ui.label("Output folder:");
                let label = state.output_dir.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "— choose —".to_string());
                if ui.button(&label).clicked() {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Choose output folder")
                        .pick_folder()
                    {
                        state.output_dir = Some(dir);
                    }
                }
            });
        }
        ExportFormat::Mp4 => {
            ui.horizontal(|ui| {
                ui.label("Output file:");
                let label = state.output_file.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "— choose —".to_string());
                if ui.button(&label).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("MP4", &["mp4"])
                        .set_file_name("animation.mp4")
                        .set_title("Save MP4 As")
                        .save_file()
                    {
                        state.output_file = Some(path);
                    }
                }
            });
        }
    }

    ui.add_space(8.0);

    // Export button
    let can_export = total_frames > 0 && match state.format {
        ExportFormat::PngSequence => state.output_dir.is_some(),
        ExportFormat::Mp4         => state.output_file.is_some(),
    };

    if ui.add_enabled(can_export, egui::Button::new(format!("{}  Export", egui_phosphor::regular::EXPORT))).clicked() {
        // Set up frame directory
        match state.format {
            ExportFormat::PngSequence => {
                state.frame_dir = state.output_dir.clone();
            }
            ExportFormat::Mp4 => {
                // Use a temp directory for intermediate PNGs
                let tmp = std::env::temp_dir().join("ods_export_frames");
                let _ = std::fs::create_dir_all(&tmp);
                // Clean any previous frames
                if let Ok(entries) = std::fs::read_dir(&tmp) {
                    for entry in entries.flatten() {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
                state.frame_dir = Some(tmp);
            }
        }

        return ExportAction::StartCapture { total_frames };
    }

    ExportAction::None
}

// ─────────────────────────────────────────────────────────────────────────────
// Screenshot processing — called from App::update each frame during export
// ─────────────────────────────────────────────────────────────────────────────

/// Process a received screenshot during export capture.
/// Returns true if the export is now complete (all frames saved).
pub fn process_screenshot(
    state: &mut ExportVideoState,
    image: &egui::ColorImage,
) -> bool {
    let (frame, total) = match &state.phase {
        ExportPhase::Capturing { frame, total, waiting: true } => (*frame, *total),
        _ => return false,
    };

    // Crop to viewport rect if available, otherwise use full image
    let (crop_x, crop_y, crop_w, crop_h) = if let Some(rect) = state.viewport_rect {
        let x = (rect.min.x as usize).min(image.size[0]);
        let y = (rect.min.y as usize).min(image.size[1]);
        let w = (rect.width() as usize).min(image.size[0].saturating_sub(x));
        let h = (rect.height() as usize).min(image.size[1].saturating_sub(y));
        (x, y, w, h)
    } else {
        (0, 0, image.size[0], image.size[1])
    };

    if crop_w == 0 || crop_h == 0 {
        // Skip degenerate frames
        advance_frame(state, frame, total);
        return frame + 1 >= total;
    }

    // Extract cropped RGBA pixels
    let mut rgba = Vec::with_capacity(crop_w * crop_h * 4);
    for y in crop_y..(crop_y + crop_h) {
        for x in crop_x..(crop_x + crop_w) {
            let pixel = image.pixels[y * image.size[0] + x];
            rgba.extend_from_slice(&pixel.to_array());
        }
    }

    // Save as PNG
    if let Some(dir) = &state.frame_dir {
        let path = dir.join(format!("frame_{:05}.png", frame));
        if let Some(img) = image::RgbaImage::from_raw(crop_w as u32, crop_h as u32, rgba) {
            if let Err(e) = img.save(&path) {
                eprintln!("Failed to save frame {}: {e}", frame);
            }
        }
    }

    advance_frame(state, frame, total);
    frame + 1 >= total
}

fn advance_frame(state: &mut ExportVideoState, frame: usize, total: usize) {
    if frame + 1 >= total {
        // All frames captured
        if state.format == ExportFormat::Mp4 {
            state.phase = ExportPhase::Encoding;
        } else {
            let dir = state.frame_dir.as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            state.phase = ExportPhase::Done {
                message: format!("{} Exported {} frames to {}", egui_phosphor::regular::CHECK, total, dir),
            };
        }
    } else {
        state.phase = ExportPhase::Capturing {
            frame: frame + 1,
            total,
            waiting: false,
        };
    }
}

/// Run ffmpeg to encode captured PNGs into MP4.
/// Should be called when phase == Encoding.
pub fn run_ffmpeg_encode(state: &mut ExportVideoState) {
    let Some(frame_dir) = &state.frame_dir else {
        state.phase = ExportPhase::Done { message: "✘ No frame directory".into() };
        return;
    };
    let Some(output) = &state.output_file else {
        state.phase = ExportPhase::Done { message: "✘ No output file".into() };
        return;
    };

    let pattern = frame_dir.join("frame_%05d.png");
    let result = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-framerate", &format!("{}", state.fps),
            "-i", &pattern.to_string_lossy(),
            "-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            &output.to_string_lossy(),
        ])
        .output();

    match result {
        Ok(out) if out.status.success() => {
            // Clean up temp frames
            if let Ok(entries) = std::fs::read_dir(frame_dir) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
            let _ = std::fs::remove_dir(frame_dir);
            state.phase = ExportPhase::Done {
                message: format!("{} Exported MP4 to {}", egui_phosphor::regular::CHECK, output.display()),
            };
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            state.phase = ExportPhase::Done {
                message: format!("✘ ffmpeg failed: {}", stderr.lines().last().unwrap_or("unknown error")),
            };
        }
        Err(e) => {
            state.phase = ExportPhase::Done {
                message: format!("✘ Failed to run ffmpeg: {e}"),
            };
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn detect_ffmpeg() -> bool {
    match std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    {
        Ok(status) => status.success(),
        Err(_) => false,
    }
}
