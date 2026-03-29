use eframe::egui_wgpu;
use glam::{Mat4, Vec3, Vec4};
use crate::entities::GlyphShape;
use crate::data::Unit;
use crate::renderer::{VpCallback, Renderer};

/// Pre-computed mesh surface data: vertices (pos + rgba) and triangle indices.
pub struct MeshRenderData {
    /// Each vertex: [x, y, z, r, g, b, a]
    pub verts:   Vec<[f32; 7]>,
    pub indices: Vec<u32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Public state
// ─────────────────────────────────────────────────────────────────────────────

pub struct Viewport3D {
    pub azimuth:         f32,
    pub elevation:       f32,
    pub distance:        f32,
    pub target:          Vec3,
    pub show_coord_sys:       bool,
    pub show_local_coord_sys: bool,   // global toggle for all local node coord sys
    pub show_xy:         bool,
    pub show_yz:         bool,
    pub show_zx:         bool,
    pub orthographic:    bool,
    // Label overlay toggles
    pub show_node_numbers:  bool,
    pub show_edge_numbers:  bool,
    pub show_glyph_numbers: bool,
    pub show_mesh_numbers:  bool,
    /// When true, the next LMB click in the viewport sets the orbit center to the nearest node.
    pub pick_orbit_center: bool,
}

impl Default for Viewport3D {
    fn default() -> Self {
        Self {
            azimuth:         45_f32.to_radians(),
            elevation:       30_f32.to_radians(),
            distance:        5.0,
            target:          Vec3::ZERO,
            show_coord_sys:       true,
            show_local_coord_sys: true,
            show_xy:         false,
            show_yz:         false,
            show_zx:         false,
            orthographic:    true,
            show_node_numbers:  false,
            show_edge_numbers:  false,
            show_glyph_numbers: false,
            show_mesh_numbers:  false,
            pick_orbit_center:  false,
        }
    }
}

impl Viewport3D {
    fn eye(&self) -> Vec3 {
        self.target
            + Vec3::new(
                self.distance * self.elevation.cos() * self.azimuth.sin(),
                self.distance * self.elevation.sin(),
                self.distance * self.elevation.cos() * self.azimuth.cos(),
            )
    }
    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }
    pub fn proj_matrix(&self, aspect: f32, scene_radius: f32) -> Mat4 {
        // Near plane is based on scene size so it doesn't shrink to zero as
        // the camera zooms in, which would cause depth-buffer precision loss.
        // Far plane extends well beyond both the scene and the camera distance.
        let near = (scene_radius * 0.0002).max(1e-4);
        let far  = (scene_radius * 10.0 + self.distance * 2.0).max(near * 2000.0);
        if self.orthographic {
            let half_h = self.distance * (45_f32.to_radians() / 2.0).tan();
            let half_w = half_h * aspect;
            Mat4::orthographic_rh(-half_w, half_w, -half_h, half_h, -far, far)
        } else {
            Mat4::perspective_rh(45_f32.to_radians(), aspect, near, far)
        }
    }
    pub fn init_renderer(wrs: &egui_wgpu::RenderState) {
        let r = Renderer::new(&wrs.device, wrs.target_format);
        wrs.renderer.write().callback_resources.insert(r);
    }
}
// ─────────────────────────────────────────────────────────────────────────────
// Public show function
// ─────────────────────────────────────────────────────────────────────────────

/// Data returned from `show_viewport` so the caller can do picking and selection.
pub struct ViewportResponse {
    /// Click position in select mode (non-drag LMB), or single-node pick in orbit mode.
    pub clicked_pos: Option<egui::Pos2>,
    /// Ctrl+click position in select mode (additive toggle).
    pub ctrl_clicked_pos: Option<egui::Pos2>,
    /// Completed rubber-band selection rect (drag released in select mode).
    pub rect_selection: Option<egui::Rect>,
    /// The view-projection matrix used for rendering.
    pub vp: Mat4,
    /// The viewport rect in screen coordinates.
    pub rect: egui::Rect,
    /// If the user chose a different distance unit via the context menu.
    pub unit_change: Option<Unit>,
    /// If the user toggled directional lighting via the context menu.
    pub lighting_toggled: Option<bool>,
}

pub fn show_viewport(
    ui:               &mut egui::Ui,
    state:            &mut Viewport3D,
    nodes:            &[[f32; 3]],
    edge_segments:    &[([f32; 3], [f32; 3])],
    edge_colors:      &[([f32; 4], [f32; 4])],
    node_colors:      &[[f32; 4]],
    node_scales:      &[f32],
    node_coord_sys:        &[[[f32; 3]; 3]],
    node_coord_sys_visible: &[bool],
    selected_nodes:   &[bool],
    node_size:        f32,
    local_coord_sys_scale: f32,
    select_mode:      bool,
    edge_thickness:   f32,
    edge_thicknesses: &[f32],
    viewport_bg:      [f32; 3],
    mmb_orbit:        bool,
    glyph_positions:  &[([f32; 3], GlyphShape, [f32; 4], f32, [f32; 3], f32)],
    glyph_selected:   &[bool],
    mesh_surfaces:    Vec<MeshRenderData>,
    wireframe_edges:  &[([f32; 3], [f32; 3])],
    unit_label:       &str,
    current_unit:     &Unit,
    light_brightness: f32,
    lighting_enabled: bool,
    node_labels:      &[String],
    edge_labels:      &[String],
    glyph_labels:     &[String],
    mesh_labels:      &[String],
    arrows:           Vec<([f32; 3], [f32; 3], [f32; 4], f32)>,
    scene_center:     [f32; 3],
    scene_radius:     f32,
) -> ViewportResponse {
    let rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(rect, egui::Sense::click_and_drag());

    // Paint viewport background
    ui.painter().rect_filled(
        rect,
        0.0,
        egui::Color32::from_rgb(
            (viewport_bg[0] * 255.0) as u8,
            (viewport_bg[1] * 255.0) as u8,
            (viewport_bg[2] * 255.0) as u8,
        ),
    );

    let mut unit_change: Option<Unit> = None;
    let mut lighting_toggled: Option<bool> = None;

    // Right-click context menu
    response.context_menu(|ui| {
        ui.label(egui::RichText::new("Viewport Settings").strong());
        ui.separator();
        ui.checkbox(&mut state.show_coord_sys, "Global CSYS");
        ui.checkbox(&mut state.show_local_coord_sys, "Local CSYS axes");
        ui.separator();
        ui.label("Reference Planes");
        ui.checkbox(&mut state.show_xy, "XY Plane");
        ui.checkbox(&mut state.show_yz, "YZ Plane");
        ui.checkbox(&mut state.show_zx, "ZX Plane");
        ui.separator();
        ui.label("Projection");
        ui.radio_value(&mut state.orthographic, false, "Perspective");
        ui.radio_value(&mut state.orthographic, true,  "Orthographic");
        ui.separator();
        ui.menu_button(format!("Model Unit ({})", current_unit.label()), |ui| {
            for u in Unit::DISTANCE_UNITS {
                let selected = current_unit == u;
                if ui.selectable_label(selected, u.label()).clicked() && !selected {
                    unit_change = Some(u.clone());
                    ui.close_menu();
                }
            }
        });
        ui.separator();
        ui.label("Camera");
        if ui.button("Focus on Model").clicked() {
            state.target   = Vec3::from(scene_center);
            state.distance = (scene_radius * 2.5).max(0.01);
            ui.close_menu();
        }
        if ui.button("Set Orbit Center (next click)").clicked() {
            state.pick_orbit_center = true;
            ui.close_menu();
        }
        if ui.button("Reset Orbit Center").clicked() {
            state.target = Vec3::from(scene_center);
            ui.close_menu();
        }
        ui.separator();
        let mut lit = lighting_enabled;
        if ui.checkbox(&mut lit, "Directional Lighting").changed() {
            lighting_toggled = Some(lit);
        }
        ui.separator();
        ui.menu_button("Labels", |ui| {
            ui.checkbox(&mut state.show_node_numbers,  format!("{} Nodes",  egui_phosphor::regular::HEXAGON));
            ui.checkbox(&mut state.show_edge_numbers,  format!("{} Edges",  egui_phosphor::regular::LINE_SEGMENT));
            ui.checkbox(&mut state.show_glyph_numbers, format!("{} Glyphs", egui_phosphor::regular::DIAMOND));
            ui.checkbox(&mut state.show_mesh_numbers,  format!("{} Meshes", egui_phosphor::regular::POLYGON));
        });
    });

    let mut clicked_pos:      Option<egui::Pos2>  = None;
    let mut ctrl_clicked_pos: Option<egui::Pos2>  = None;
    let mut rect_selection:   Option<egui::Rect>  = None;

    if select_mode {
        // ── SELECT MODE: rubber-band rect + Ctrl+click ────────────────────
        let ctrl = ui.input(|i| i.modifiers.ctrl);
        let drag_start_id = egui::Id::new("vp_rect_sel_start");

        // On the very first frame of a drag, record the anchor position
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                ui.ctx().data_mut(|d| d.insert_temp(drag_start_id, pos));
            }
        }

        // While dragging: draw the rect from the stored anchor to current pos
        if response.dragged_by(egui::PointerButton::Primary) {
            if let Some(current) = response.interact_pointer_pos() {
                let anchor: Option<egui::Pos2> =
                    ui.ctx().data(|d| d.get_temp(drag_start_id));
                if let Some(start) = anchor {
                    let painter = ui.painter_at(rect);
                    let drag_rect = egui::Rect::from_two_pos(start, current);
                    painter.rect(
                        drag_rect,
                        egui::CornerRadius::ZERO,
                        egui::Color32::from_rgba_unmultiplied(80, 180, 255, 30),
                        egui::Stroke::new(1.0, egui::Color32::from_rgb(80, 180, 255)),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }

        // On drag end: emit rect or treat as click depending on distance
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            let anchor: Option<egui::Pos2> =
                ui.ctx().data(|d| d.get_temp(drag_start_id));
            ui.ctx().data_mut(|d| d.remove::<egui::Pos2>(drag_start_id));

            if let (Some(start), Some(end)) = (anchor, response.interact_pointer_pos()) {
                if (end - start).length() > 5.0 {
                    rect_selection = Some(egui::Rect::from_two_pos(start, end));
                } else {
                    // Tiny movement = treat as click
                    if ctrl { ctrl_clicked_pos = Some(end); }
                    else     { clicked_pos = Some(end); }
                }
            }
        } else if response.clicked() {
            // True zero-movement tap
            if ctrl { ctrl_clicked_pos = response.interact_pointer_pos(); }
            else     { clicked_pos = response.interact_pointer_pos(); }
        }
    } else if !mmb_orbit {
        // ── ORBIT MODE with LMB ──────────────────────────────────────────
        // Track whether the drag covered meaningful distance — persisted in
        // egui temp data so it survives across frames.
        let orbit_dragged_id = egui::Id::new("vp_orbit_was_dragged");

        if response.drag_started_by(egui::PointerButton::Primary) {
            ui.ctx().data_mut(|d| d.insert_temp(orbit_dragged_id, false));
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            let d = response.drag_delta();
            if d.length() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(orbit_dragged_id, true));
                state.azimuth   -= d.x * 0.005;
                state.elevation  = (state.elevation + d.y * 0.005)
                                      .clamp(-89f32.to_radians(), 89f32.to_radians());
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            let was_dragged: bool =
                ui.ctx().data(|d| d.get_temp(orbit_dragged_id).unwrap_or(false));
            ui.ctx().data_mut(|d| d.remove::<bool>(orbit_dragged_id));
            if !was_dragged {
                clicked_pos = response.interact_pointer_pos();
            }
        } else if response.clicked() {
            clicked_pos = response.interact_pointer_pos();
        }
    } else {
        // ── MMB orbit mode: LMB click picks nodes ────────────────────────
        if response.clicked() {
            clicked_pos = response.interact_pointer_pos();
        }
    }

    // ── MMB orbit (always active, regardless of select/orbit mode) ────────
    if mmb_orbit {
        if response.dragged_by(egui::PointerButton::Middle) {
            let d = response.drag_delta();
            if d.length() > 0.5 {
                state.azimuth   -= d.x * 0.005;
                state.elevation  = (state.elevation + d.y * 0.005)
                                      .clamp(-89f32.to_radians(), 89f32.to_radians());
            }
        }
    }

    // Pan — scale by the larger of the current distance or a minimum based on
    // scene size, so pan remains usable even when zoomed in very close.
    let pan_btn = if mmb_orbit { egui::PointerButton::Secondary } else { egui::PointerButton::Middle };
    if response.dragged_by(pan_btn) {
        let d = response.drag_delta();
        let right = Vec3::new(state.azimuth.cos(), 0.0, -state.azimuth.sin());
        let pan_scale = state.distance.max(scene_radius * 0.005) * 0.001;
        state.target -= right   * d.x * pan_scale;
        state.target += Vec3::Y * d.y * pan_scale;
    }

    // Zoom (scroll)
    let scroll = ui.input(|i| i.raw_scroll_delta.y);
    if response.hovered() && scroll.abs() > 0.0 {
        let prev_dist = state.distance;
        let min_dist  = scene_radius * 1e-3;
        let max_dist  = scene_radius * 500.0;
        state.distance = (state.distance * (1.0 - scroll * 0.001)).clamp(min_dist.max(1e-6), max_dist.max(1.0));

        // In perspective: dolly the orbit center forward as the camera zooms in
        // so the pivot advances into the scene and zoom never "bottoms out".
        if !state.orthographic {
            let dist_delta = prev_dist - state.distance; // positive = zoomed in
            if dist_delta.abs() > 1e-8 {
                let look = (state.target - state.eye()).normalize();
                state.target += look * dist_delta * 0.6;
            }
        }
    }

    // Pre-compute mesh label anchor (centroid) from each surface's vertices,
    // before mesh_surfaces is moved into the wgpu callback.
    let mesh_centroids: Vec<Option<[f32; 3]>> = mesh_surfaces.iter().map(|ms| {
        if ms.verts.is_empty() { return None; }
        let n = ms.verts.len() as f32;
        Some([
            ms.verts.iter().map(|v| v[0]).sum::<f32>() / n,
            ms.verts.iter().map(|v| v[1]).sum::<f32>() / n,
            ms.verts.iter().map(|v| v[2]).sum::<f32>() / n,
        ])
    }).collect();

    // Issue wgpu callback
    let aspect = rect.width() / rect.height().max(1.0);
    let vp = state.proj_matrix(aspect, scene_radius) * state.view_matrix();
    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
        rect,
        VpCallback {
            vp,
            view:              state.view_matrix(),
            show_coord_sys:         state.show_coord_sys,
            show_local_coord_sys:   state.show_local_coord_sys,
            show_xy:           state.show_xy,
            show_yz:           state.show_yz,
            show_zx:           state.show_zx,
            nodes:             nodes.to_vec(),
            node_colors:       node_colors.to_vec(),
            node_scales:       node_scales.to_vec(),
            node_coord_sys:         node_coord_sys.to_vec(),
            node_coord_sys_visible: node_coord_sys_visible.to_vec(),
            selected_nodes:    selected_nodes.to_vec(),
            edge_segments:     edge_segments.to_vec(),
            edge_colors:       edge_colors.to_vec(),
            distance:          state.distance,
            node_size,
            local_coord_sys_scale,
            edge_thickness,
            edge_thicknesses:  edge_thicknesses.to_vec(),
            glyphs:            glyph_positions.to_vec(),
            glyph_selected:    glyph_selected.to_vec(),
            mesh_surfaces,
            wireframe_edges:   wireframe_edges.to_vec(),
            light_brightness,
            arrows,
        },
    ));

    // 2D ruler overlay
    draw_ruler(ui.painter(), rect, state.distance, aspect, ui.visuals().dark_mode, unit_label);

    // ── Triad Overlay ────────────────────────────────────────────────────────
    let triad_center = rect.left_bottom() + egui::vec2(60.0, -60.0);
    let triad_radius = 40.0;
    
    let view_mat = state.view_matrix();
    let mut triad_axes = [
        (Vec3::X, egui::Color32::from_rgb(230, 60, 60), "X"),
        (Vec3::Y, egui::Color32::from_rgb(60, 210, 60), "Y"),
        (Vec3::Z, egui::Color32::from_rgb(60, 110, 240), "Z"),
    ];

    // Sort axes by depth (Z in view space) so axes pointing towards user are drawn last
    triad_axes.sort_by(|a, b| {
        let za = view_mat.transform_vector3(a.0).z;
        let zb = view_mat.transform_vector3(b.0).z;
        za.partial_cmp(&zb).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut triad_hit = false;
    let actual_click = clicked_pos.or(ctrl_clicked_pos);
    let hover_pos = response.hover_pos();

    for &(axis, color, label) in &triad_axes {
        let eye_dir = view_mat.transform_vector3(axis);
        let screen_dir = egui::vec2(eye_dir.x, -eye_dir.y);
        
        let tip_pos = triad_center + screen_dir * triad_radius;
        let neg_tip_pos = triad_center - screen_dir * 0.6 * triad_radius;
        
        let mut fwd_stroke = 3.0;
        let mut bwd_radius = 4.0;
        let mut font_size = 13.0;
        
        if let Some(hpos) = hover_pos {
            if hpos.distance(tip_pos) < 15.0 {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                fwd_stroke = 5.0;
                font_size = 15.0;
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(format!("tt_{}", label)), |ui| {
                    ui.label(format!("+{} View", label));
                });
            } else if hpos.distance(neg_tip_pos) < 12.0 {
                ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
                bwd_radius = 6.0;
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new(format!("tt_neg_{}", label)), |ui| {
                    ui.label(format!("-{} View", label));
                });
            }
        }

        // Draw line
        ui.painter().line_segment(
            [triad_center, tip_pos],
            egui::Stroke::new(fwd_stroke, color),
        );
        
        // Label (only if axis isn't pointing straight into camera)
        if screen_dir.length() > 0.1 {
            let label_pos = triad_center + screen_dir * (triad_radius + 12.0);
            ui.painter().text(
                label_pos,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(font_size),
                color,
            );
        }
        
        // Negative axis dot
        ui.painter().circle_filled(neg_tip_pos, bwd_radius, color.linear_multiply(0.4));
        
        if let Some(cpos) = actual_click {
            if cpos.distance(tip_pos) < 15.0 {
                triad_hit = true;
                match label {
                    "X" => { state.azimuth = 90f32.to_radians(); state.elevation = 0.0; }
                    "Y" => { state.azimuth = 0.0; state.elevation = 89.9f32.to_radians(); }
                    "Z" => { state.azimuth = 0.0; state.elevation = 0.0; }
                    _ => {}
                }
            } else if cpos.distance(neg_tip_pos) < 12.0 {
                triad_hit = true;
                match label {
                    "X" => { state.azimuth = -90f32.to_radians(); state.elevation = 0.0; }
                    "Y" => { state.azimuth = 0.0; state.elevation = -89.9f32.to_radians(); }
                    "Z" => { state.azimuth = 180f32.to_radians(); state.elevation = 0.0; }
                    _ => {}
                }
            }
        }
    }
    
    // Draw central origin dot
    ui.painter().circle_filled(triad_center, 3.0, egui::Color32::WHITE);

    // Isometric dot
    let iso_world_dir = Vec3::new(1.0, 1.0, 1.0).normalize();
    let iso_eye_dir = view_mat.transform_vector3(iso_world_dir);
    let iso_screen_dir = egui::vec2(iso_eye_dir.x, -iso_eye_dir.y);
    
    // When exactly isometric, iso_screen_dir is ~zero and it sits in the middle.
    let iso_pos = triad_center + iso_screen_dir * (triad_radius * 0.82);
    let iso_color = egui::Color32::from_rgb(80, 180, 255);
    let mut iso_radius = 6.0;
    if let Some(hpos) = hover_pos {
        if hpos.distance(iso_pos) < 15.0 {
            ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::PointingHand);
            iso_radius = 8.0;
            egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), egui::Id::new("tt_iso"), |ui| {
                ui.label("Isometric View");
            });
        }
    }
    ui.painter().circle_filled(iso_pos, iso_radius, iso_color);

    if let Some(cpos) = actual_click {
        if cpos.distance(iso_pos) < 15.0 {
            triad_hit = true;
            state.azimuth = 45f32.to_radians();
            state.elevation = 35.264f32.to_radians();
        }
    }

    if triad_hit {
        clicked_pos = None;
        ctrl_clicked_pos = None;
    }

    // ── World→screen projection (shared by labels and orbit-center pick) ────
    let proj_half_w = rect.width()  * 0.5;
    let proj_half_h = rect.height() * 0.5;
    let project = |p: [f32; 3]| -> Option<egui::Pos2> {
        let clip = vp * Vec4::new(p[0], p[1], p[2], 1.0);
        if clip.w <= 0.0 { return None; }
        let ndx = clip.x / clip.w;
        let ndy = clip.y / clip.w;
        if ndx < -1.05 || ndx > 1.05 || ndy < -1.05 || ndy > 1.05 { return None; }
        Some(egui::pos2(
            rect.left() + proj_half_w * (1.0 + ndx),
            rect.top()  + proj_half_h * (1.0 - ndy),
        ))
    };

    // ── Number label overlay ─────────────────────────────────────────────────
    let any_labels = state.show_node_numbers
        || state.show_edge_numbers
        || state.show_glyph_numbers
        || state.show_mesh_numbers;
    if any_labels {
        let label_font = egui::FontId::monospace(11.0);
        let bg_color   = egui::Color32::from_black_alpha(150);
        let fg_color   = egui::Color32::from_rgb(230, 230, 230);
        let painter    = ui.painter_at(rect);
        let half_w     = rect.width()  * 0.5;
        let half_h     = rect.height() * 0.5;
        // Re-define project with the local half_w/half_h for this block.
        let project = |p: [f32; 3]| -> Option<egui::Pos2> {
            let clip = vp * Vec4::new(p[0], p[1], p[2], 1.0);
            if clip.w <= 0.0 { return None; }
            let ndx = clip.x / clip.w;
            let ndy = clip.y / clip.w;
            if ndx < -1.05 || ndx > 1.05 || ndy < -1.05 || ndy > 1.05 { return None; }
            Some(egui::pos2(
                rect.left() + half_w * (1.0 + ndx),
                rect.top()  + half_h * (1.0 - ndy),
            ))
        };

        // Draw a small label with a dark background pill at the given screen pos.
        let draw_label = |painter: &egui::Painter, screen: egui::Pos2, text: &str| {
            let p = screen + egui::vec2(6.0, -6.0);
            let galley = painter.layout_no_wrap(
                text.to_string(), label_font.clone(), fg_color,
            );
            let pad = egui::vec2(3.0, 2.0);
            let bg = egui::Rect::from_min_size(p - pad, galley.size() + pad * 2.0);
            painter.rect_filled(bg, 2.0, bg_color);
            painter.galley(p, galley, fg_color);
        };

        if state.show_node_numbers {
            for (pos, label) in nodes.iter().zip(node_labels.iter()) {
                if let Some(sp) = project(*pos) {
                    draw_label(&painter, sp, label);
                }
            }
        }

        if state.show_edge_numbers {
            for (&(a, b), label) in edge_segments.iter().zip(edge_labels.iter()) {
                let mid = [
                    (a[0] + b[0]) * 0.5,
                    (a[1] + b[1]) * 0.5,
                    (a[2] + b[2]) * 0.5,
                ];
                if let Some(sp) = project(mid) {
                    draw_label(&painter, sp, label);
                }
            }
        }

        if state.show_glyph_numbers {
            for (gd, label) in glyph_positions.iter().zip(glyph_labels.iter()) {
                if let Some(sp) = project(gd.0) {
                    draw_label(&painter, sp, label);
                }
            }
        }

        if state.show_mesh_numbers {
            for (centroid_opt, label) in mesh_centroids.iter().zip(mesh_labels.iter()) {
                if let Some(centroid) = centroid_opt {
                    if let Some(sp) = project(*centroid) {
                        draw_label(&painter, sp, label);
                    }
                }
            }
        }
    }

    // ── Orbit-center pick mode ───────────────────────────────────────────────
    // When active, any click snaps the orbit target to the nearest visible node.
    // If no node is within 40 px of the click, fall back to the scene centre.
    if state.pick_orbit_center {
        // Tint the viewport border to hint that something is active.
        ui.painter().rect_stroke(
            rect,
            0.0,
            egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 200, 60)),
            egui::StrokeKind::Inside,
        );

        let maybe_click = if let Some(click) = clicked_pos.take() {
            Some(click)
        } else if let Some(click) = ctrl_clicked_pos.take() {
            Some(click)
        } else {
            None
        };

        if let Some(click) = maybe_click {
            let best = nodes.iter().filter_map(|&n| {
                project(n).map(|sp| (Vec3::from(n), (sp - click).length()))
            }).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

            state.target = if let Some((pos, dist)) = best {
                if dist < 40.0 { pos } else { Vec3::from(scene_center) }
            } else {
                Vec3::from(scene_center)
            };
            state.pick_orbit_center = false;
        }
    }

    ViewportResponse { clicked_pos, ctrl_clicked_pos, rect_selection, vp, rect, unit_change, lighting_toggled }
}

// ─────────────────────────────────────────────────────────────────────────────
// Scale ruler (2D egui overlay)
// ─────────────────────────────────────────────────────────────────────────────

/// Draw a horizontal scale bar at the bottom-centre of the viewport.
///
/// The bar is a fixed ~175 screen-pixels wide; the label shows the
/// snapped real-world length it represents.
fn draw_ruler(
    painter: &egui::Painter,
    rect: egui::Rect,
    distance: f32,
    aspect: f32,
    dark_mode: bool,
    unit_label: &str,
) {
    // World units visible across the full viewport width.
    // This formula is identical for perspective and the matched orthographic mode.
    let fov_half = 45_f32.to_radians() / 2.0;
    let world_width = 2.0 * distance * fov_half.tan() * aspect;
    let world_per_px = world_width / rect.width();

    // Target ruler width in screen pixels, then snap world value to 1-2-5.
    let target_px    = 175.0_f32;
    let raw_world    = target_px * world_per_px;
    let world_nice   = nice_scale(raw_world);
    let ruler_px     = world_nice / world_per_px;

    // Layout: centred horizontally, 28 px from the bottom edge.
    let cx     = rect.center().x;
    let y_bar  = rect.bottom() - 28.0;
    let y_tick = y_bar - 7.0;                   // tick height above the bar

    let x0 = cx - ruler_px * 0.5;
    let x1 = cx + ruler_px * 0.5;

    // Colours — adapt for dark/light mode
    let (bar_color, bg_color, text_color) = if dark_mode {
        (
            egui::Color32::from_rgba_premultiplied(220, 220, 220, 200),
            egui::Color32::from_rgba_premultiplied(20,  20,  30,  160),
            egui::Color32::from_rgb(230, 230, 230),
        )
    } else {
        (
            egui::Color32::from_rgba_premultiplied(40, 40, 50, 220),
            egui::Color32::from_rgba_premultiplied(240, 240, 245, 180),
            egui::Color32::from_rgb(30, 30, 40),
        )
    };

    // Background pill so the ruler is readable over any scene.
    let bg_rect = egui::Rect::from_min_max(
        egui::pos2(x0 - 10.0, y_tick - 4.0),
        egui::pos2(x1 + 10.0, y_bar  + 6.0),
    );
    painter.rect_filled(bg_rect, egui::CornerRadius::same(4), bg_color);

    let stroke = egui::Stroke::new(1.5, bar_color);

    // Main horizontal bar
    painter.line_segment([egui::pos2(x0, y_bar), egui::pos2(x1, y_bar)], stroke);
    // Left tick
    painter.line_segment([egui::pos2(x0, y_bar), egui::pos2(x0, y_tick)], stroke);
    // Right tick
    painter.line_segment([egui::pos2(x1, y_bar), egui::pos2(x1, y_tick)], stroke);
    // Centre tick (half way)
    let xm = (x0 + x1) * 0.5;
    painter.line_segment(
        [egui::pos2(xm, y_bar), egui::pos2(xm, y_bar - 4.0)],
        egui::Stroke::new(1.0, bar_color),
    );

    // Label: choose unit automatically.
    let label = format!("{} {}", format_ruler_value(world_nice), unit_label);
    painter.text(
        egui::pos2(cx, y_tick - 6.0),
        egui::Align2::CENTER_BOTTOM,
        label,
        egui::FontId::proportional(11.0),
        text_color,
    );
}

/// Snap a raw world length to the nearest nice 1-2-5 value.
fn nice_scale(raw: f32) -> f32 {
    if raw <= 0.0 { return 1.0; }
    let exp   = raw.log10().floor();
    let base  = 10_f32.powi(exp as i32);
    let frac  = raw / base;
    let nice  = if frac < 1.5 { 1.0 }
                else if frac < 3.5 { 2.0 }
                else if frac < 7.5 { 5.0 }
                else { 10.0 };
    nice * base
}

/// Format a world-space ruler value with sensible precision.
fn format_ruler_value(v: f32) -> String {
    if v == 0.0 { return "0".to_string(); }
    let abs = v.abs();
    if abs >= 1_000_000.0 {
        format!("{:.3e}", v)
    } else if abs >= 100.0 {
        format!("{:.0}", v)
    } else if abs >= 10.0 {
        format!("{:.1}", v)
    } else if abs >= 1.0 {
        format!("{:.2}", v)
    } else if abs >= 0.01 {
        format!("{:.3}", v)
    } else {
        format!("{:.3e}", v)
    }
}
