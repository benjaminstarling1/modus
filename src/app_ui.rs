use crate::app::VisualizationMode;
use crate::data::Unit;
use crate::palette::Palette;
use crate::persist::{SavedView, UserPrefs};
use crate::viewport::Viewport3D;

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
                            VisualizationMode::None           => "None",
                            VisualizationMode::ContourColor   => "Contour",
                            VisualizationMode::SizeScale      => "Size",
                            VisualizationMode::ContourAndSize => "Both",
                        })
                        .width(100.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut prefs.default_vis_mode, VisualizationMode::None,           "None");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisualizationMode::ContourColor,   "Contour");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisualizationMode::SizeScale,      "Size");
                            ui.selectable_value(&mut prefs.default_vis_mode, VisualizationMode::ContourAndSize, "Both");
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
                    ui.add(egui::DragValue::new(&mut prefs.local_coord_sys_scale_pct)
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
                            for u in Unit::DISTANCE_UNITS {
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

                    // Default node colour
                    ui.label("Default Node Color:");
                    ui.color_edit_button_rgb(&mut prefs.default_node_color);
                    ui.end_row();

                    // Default edge colour
                    ui.label("Default Edge Color:");
                    ui.color_edit_button_rgb(&mut prefs.default_edge_color);
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
                        show_coord_sys: viewport.show_coord_sys,
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
                                viewport.show_coord_sys = view.show_coord_sys;
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
                                view.show_coord_sys = viewport.show_coord_sys;
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
