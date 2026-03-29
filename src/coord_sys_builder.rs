use crate::table::identity_mat3;

// ─────────────────────────────────────────────────────────────────────────────
// Operations
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CoordSysOp {
    RotateX(f32), // degrees
    RotateY(f32),
    RotateZ(f32),
}

impl CoordSysOp {
    fn label(&self) -> &'static str {
        match self {
            CoordSysOp::RotateX(_) => "Rotate X",
            CoordSysOp::RotateY(_) => "Rotate Y",
            CoordSysOp::RotateZ(_) => "Rotate Z",
        }
    }

    fn angle_mut(&mut self) -> &mut f32 {
        match self {
            CoordSysOp::RotateX(a) | CoordSysOp::RotateY(a) | CoordSysOp::RotateZ(a) => a,
        }
    }

    /// Returns the 3×3 rotation matrix for this single operation.
    fn matrix(&self) -> [[f32; 3]; 3] {
        match self {
            CoordSysOp::RotateX(deg) => {
                let (s, c) = deg.to_radians().sin_cos();
                [[1.0, 0.0, 0.0], [0.0, c, s], [0.0, -s, c]]
            }
            CoordSysOp::RotateY(deg) => {
                let (s, c) = deg.to_radians().sin_cos();
                [[c, 0.0, -s], [0.0, 1.0, 0.0], [s, 0.0, c]]
            }
            CoordSysOp::RotateZ(deg) => {
                let (s, c) = deg.to_radians().sin_cos();
                [[c, s, 0.0], [-s, c, 0.0], [0.0, 0.0, 1.0]]
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Builder state
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CoordSysBuilder {
    pub ops: Vec<CoordSysOp>,
    /// Base matrix (e.g. from an existing node) to apply ops to.
    pub base_matrix: [[f32; 3]; 3],
    /// Preview camera azimuth (radians).
    pub preview_az: f32,
    /// Preview camera elevation (radians).
    pub preview_el: f32,
    /// Name used for "Save to Manager".
    pub save_name: String,
}

impl Default for CoordSysBuilder {
    fn default() -> Self {
        Self {
            ops:         Vec::new(),
            base_matrix: identity_mat3(),
            preview_az:  30_f32.to_radians(),
            preview_el:  20_f32.to_radians(),
            save_name:   String::new(),
        }
    }
}

impl CoordSysBuilder {
    /// Load from an existing 3×3 matrix (clears ops, but keeps the matrix as the base).
    pub fn load_from_matrix(&mut self, m: [[f32; 3]; 3]) {
        self.base_matrix = m;
        self.ops.clear();
    }

    /// Load from an existing base matrix and a set of ops.
    pub fn load_with_ops(&mut self, base: [[f32; 3]; 3], ops: Vec<CoordSysOp>) {
        self.base_matrix = base;
        self.ops = ops;
    }

    /// Fold all ops into a single 3×3 rotation matrix (column-major: columns = local axes).
    pub fn result_matrix(&self) -> [[f32; 3]; 3] {
        let mut m = self.base_matrix;
        for op in &self.ops {
            m = mat3_mul(op.matrix(), m);
        }
        m
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Matrix helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Multiply two 3×3 row-major matrices: result = a * b.
fn mat3_mul(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut r = [[0.0_f32; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            for k in 0..3 {
                r[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    r
}

// ─────────────────────────────────────────────────────────────────────────────
// CSYS Builder egui window
// ─────────────────────────────────────────────────────────────────────────────

/// Render the CSYS Builder floating window.
///
/// * `open`          — whether the window is visible; set to false to close.
/// * `builder`       — mutable builder state.
/// * `target_label`  — display name of the target node ("(no node)" if None).
/// * `on_apply`          — set to Some((matrix, base_matrix, ops)) when Apply is clicked (applies to targeted node).
/// * `on_save_to_manager` — set to Some((name, matrix, base_matrix, ops)) when Save to Manager is clicked.
pub fn show_coord_sys_builder_window(
    ctx:                 &egui::Context,
    open:                &mut bool,
    builder:             &mut CoordSysBuilder,
    target_label:        Option<&str>,
    on_apply:            &mut Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)>,
    on_save_to_manager:  &mut Option<(String, [[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)>,
) {
    if !*open { return; }

    let title = match target_label {
        Some(lbl) => format!("CSYS Builder — {lbl}"),
        None      => "CSYS Builder".to_string(),
    };

    // Let's use local Option to avoid borrow checker issues
    let mut close_requested   = false;
    let mut apply_result: Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)>        = None;
    let mut save_mgr_result: Option<(String, [[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)> = None;

    egui::Window::new(title)
        .resizable(true)
        .default_width(500.0)
        .min_width(380.0)
        .max_width(750.0)
        .collapsible(false)
        .open(open)               // only title-bar X button touches *open
        .show(ctx, |ui| {
            // ── Two-panel layout: fixed left column + fluid preview ──────────────
            ui.horizontal_top(|ui| {
                // Left column: fixed 200 px – ops table, matrix, buttons
                ui.vertical(|ui| {
                    ui.set_width(200.0);  // exact width; right column gets the rest

                    ui.strong("Operations");
                    ui.separator();

                    let mut delete_idx: Option<usize> = None;

                    egui::ScrollArea::vertical()
                        .id_salt("csys_ops_scroll")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            let n = builder.ops.len();
                            for i in 0..n {
                                ui.push_id(i, |ui| {
                                    ui.horizontal(|ui| {
                                        // Op type selector
                                        let cur_label = builder.ops[i].label();
                                        egui::ComboBox::from_id_salt(("op_type", i))
                                            .selected_text(cur_label)
                                            .width(90.0)
                                            .show_ui(ui, |ui| {
                                                let angle = *builder.ops[i].angle_mut();
                                                if ui.selectable_label(
                                                    matches!(builder.ops[i], CoordSysOp::RotateX(_)),
                                                    "Rotate X",
                                                ).clicked() {
                                                    builder.ops[i] = CoordSysOp::RotateX(angle);
                                                }
                                                if ui.selectable_label(
                                                    matches!(builder.ops[i], CoordSysOp::RotateY(_)),
                                                    "Rotate Y",
                                                ).clicked() {
                                                    builder.ops[i] = CoordSysOp::RotateY(angle);
                                                }
                                                if ui.selectable_label(
                                                    matches!(builder.ops[i], CoordSysOp::RotateZ(_)),
                                                    "Rotate Z",
                                                ).clicked() {
                                                    builder.ops[i] = CoordSysOp::RotateZ(angle);
                                                }
                                            });

                                        // Angle input
                                        ui.add(
                                            egui::DragValue::new(builder.ops[i].angle_mut())
                                                .range(-360.0..=360.0)
                                                .speed(1.0)
                                                .suffix("°")
                                                .min_decimals(1)
                                                .max_decimals(2),
                                        );

                                        // Delete row
                                        if ui.button(egui_phosphor::regular::TRASH).clicked() {
                                            delete_idx = Some(i);
                                        }
                                    });
                                });
                            }
                        });

                    if let Some(idx) = delete_idx {
                        builder.ops.remove(idx);
                    }

                    ui.add_space(4.0);
                    if ui.button(format!("{}  Add Operation", egui_phosphor::regular::PLUS)).clicked() {
                        builder.ops.push(CoordSysOp::RotateY(0.0));
                    }

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(4.0);

                    // ── Resulting matrix (read-only) ────────────────────────
                    ui.strong("Resulting 3×3 Matrix");
                    ui.add_space(4.0);
                    let m = builder.result_matrix();
                    let row_labels = ["X:", "Y:", "Z:"];
                    egui::Grid::new("csys_mat_grid")
                        .num_columns(4)
                        .spacing([6.0, 2.0])
                        .show(ui, |ui| {
                            for (row_label, col) in row_labels.iter().zip(m.iter()) {
                                ui.label(egui::RichText::new(*row_label).color(
                                    match *row_label {
                                        "X:" => egui::Color32::from_rgb(230, 80, 80),
                                        "Y:" => egui::Color32::from_rgb(80, 210, 80),
                                        _     => egui::Color32::from_rgb(80, 130, 230),
                                    }
                                ));
                                for &v in col.iter() {
                                    ui.label(
                                        egui::RichText::new(format!("{:+.4}", v))
                                            .monospace()
                                            .size(11.0),
                                    );
                                }
                                ui.end_row();
                            }
                        });

                    ui.add_space(8.0);

                    // ── Action buttons ──────────────────────────────────────
                    ui.horizontal(|ui| {
                        if ui.button(format!("{}  Apply", egui_phosphor::regular::CHECK)).clicked() {
                            apply_result  = Some((builder.result_matrix(), builder.base_matrix, builder.ops.clone()));
                            close_requested = true;
                        }
                        if ui.button(format!("{}  Reset", egui_phosphor::regular::ARROWS_CLOCKWISE)).clicked() {
                            builder.ops.clear();
                            builder.base_matrix = identity_mat3();
                        }
                    });

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(2.0);
                    let weak_color = ui.visuals().weak_text_color();
                    ui.label(egui::RichText::new("Save to CSYS Manager").small().italics()
                        .color(weak_color));
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut builder.save_name)
                                .hint_text("Name…")
                                .desired_width(110.0),
                        );
                        let can_save = !builder.save_name.trim().is_empty();
                        if ui.add_enabled(can_save, egui::Button::new(format!("{} Save", egui_phosphor::regular::FLOPPY_DISK))).clicked() {
                            save_mgr_result = Some((
                                builder.save_name.trim().to_string(),
                                builder.result_matrix(),
                                builder.base_matrix,
                                builder.ops.clone()
                            ));
                        }
                    });
                });

                ui.separator();

                // ── Right: mini 2D projection viewport ──────────────────────
                ui.vertical(|ui| {
                    ui.set_min_width(150.0);
                    ui.strong("Preview");
                    ui.label(
                        egui::RichText::new("drag to orbit")
                            .color(egui::Color32::from_rgb(100, 100, 120))
                            .size(9.5),
                    );
                    ui.separator();

                    // Preview canvas: square, fills all remaining available width
                    // so the window can be freely resized and the preview grows with it.
                    let preview_side = ui.available_width().max(120.0);
                    let size = egui::vec2(preview_side, preview_side);
                    let (rect, drag_resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

                    // Orbit: drag delta matches main viewport convention
                    if drag_resp.dragged() {
                        let d = drag_resp.drag_delta();
                        builder.preview_az += d.x * 0.008;
                        builder.preview_el  = (builder.preview_el + d.y * 0.008)
                            .clamp(-89_f32.to_radians(), 89_f32.to_radians());
                    }

                    let painter = ui.painter_at(rect);

                    // Dark background
                    painter.rect_filled(
                        rect,
                        egui::CornerRadius::same(4),
                        egui::Color32::from_rgb(18, 18, 24),
                    );

                    let centre = rect.center();
                    let scale  = preview_side * 0.40;

                    // Projection using the user-controlled azimuth/elevation.
                    let az  = builder.preview_az;
                    let el  = builder.preview_el;
                    let project = |v: [f32; 3]| -> egui::Pos2 {
                        // Right-hand: X right, Y up, Z toward camera.
                        let rot_az = [
                            v[0] * az.cos() + v[2] * az.sin(),
                            v[1],
                            -v[0] * az.sin() + v[2] * az.cos(),
                        ];
                        let sx = rot_az[0];
                        let sy = rot_az[1] * el.cos() - rot_az[2] * el.sin();
                        egui::pos2(centre.x + sx * scale, centre.y - sy * scale)
                    };

                    let origin = [0.0_f32, 0.0, 0.0];
                    let o_sc = project(origin);

                    // Draw global axes (dim grey, shorter)
                    let global_axes: [([f32;3], egui::Color32); 3] = [
                        ([0.6, 0.0, 0.0], egui::Color32::from_rgb(100, 40, 40)),
                        ([0.0, 0.6, 0.0], egui::Color32::from_rgb(40, 100, 40)),
                        ([0.0, 0.0, 0.6], egui::Color32::from_rgb(40, 60, 110)),
                    ];
                    for (vec, color) in &global_axes {
                        let tip = project(*vec);
                        painter.line_segment([o_sc, tip], egui::Stroke::new(1.0, *color));
                    }
                    // Global axis labels
                    let glbl = [("X", [0.65, 0.0, 0.0]), ("Y", [0.0, 0.65, 0.0]), ("Z", [0.0, 0.0, 0.65])];
                    for (lbl, v) in &glbl {
                        let p = project(*v);
                        painter.text(
                            p,
                            egui::Align2::CENTER_CENTER,
                            lbl,
                            egui::FontId::proportional(9.0),
                            egui::Color32::from_rgb(90, 90, 110),
                        );
                    }

                    // Draw local axes (RGB, full length)
                    let m = builder.result_matrix();
                    let local_axes: [([f32;3], egui::Color32, &str); 3] = [
                        (m[0], egui::Color32::from_rgb(230, 80, 80),  "X"),
                        (m[1], egui::Color32::from_rgb(80, 210, 80),  "Y"),
                        (m[2], egui::Color32::from_rgb(80, 130, 230), "Z"),
                    ];
                    for (vec, color, lbl) in &local_axes {
                        let tip = project(*vec);
                        painter.line_segment([o_sc, tip], egui::Stroke::new(2.0, *color));
                        let label_pos = project([vec[0]*1.1, vec[1]*1.1, vec[2]*1.1]);
                        painter.text(
                            label_pos,
                            egui::Align2::CENTER_CENTER,
                            *lbl,
                            egui::FontId::proportional(10.0),
                            *color,
                        );
                    }

                    // Origin dot
                    painter.circle_filled(o_sc, 3.0, egui::Color32::WHITE);

                    // Legend
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Dim = global  Bright = local")
                            .color(egui::Color32::from_rgb(100, 100, 120))
                            .size(9.5),
                    );
                });
            });
        });

    // Apply close and result after the window borrow is released
    if close_requested {
        *open = false;
    }
    if let Some(mat) = apply_result {
        *on_apply = Some(mat);
    }
    if let Some(entry) = save_mgr_result {
        *on_save_to_manager = Some(entry);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CSYS Manager
// ─────────────────────────────────────────────────────────────────────────────

/// A named coordinate system stored in the manager.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoordSysEntry {
    pub name:   String,
    pub matrix: [[f32; 3]; 3],
    #[serde(default = "crate::table::identity_mat3")]
    pub base_matrix: [[f32; 3]; 3],
    #[serde(default)]
    pub ops:    Vec<CoordSysOp>,
    /// True for the built-in "Global" entry (cannot be deleted).
    pub locked: bool,
}

/// Application-level list of named coordinate systems.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CoordSysManager {
    pub entries: Vec<CoordSysEntry>,
}

impl Default for CoordSysManager {
    fn default() -> Self {
        Self {
            entries: vec![CoordSysEntry {
                name:   "Global".to_string(),
                matrix: identity_mat3(),
                base_matrix: identity_mat3(),
                ops:    Vec::new(),
                locked: true,
            }],
        }
    }
}

impl CoordSysManager {
    /// Add a named entry (or replace if name already exists).
    pub fn add_or_replace(&mut self, name: String, matrix: [[f32; 3]; 3], base_matrix: [[f32; 3]; 3], ops: Vec<CoordSysOp>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.name == name && !e.locked) {
            e.matrix = matrix;
            e.base_matrix = base_matrix;
            e.ops = ops;
        } else {
            self.entries.push(CoordSysEntry { name, matrix, base_matrix, ops, locked: false });
        }
    }
}

/// `on_apply`: set to `Some((matrix, base_matrix, ops))` if the user clicks "Apply to selected".
/// `selected_count`: number of currently selected nodes (for the button label).
/// `on_edit`: set to `Some((name, base_matrix, ops))` if the user clicks "Edit".
pub fn show_coord_sys_manager_panel(
    ui:             &mut egui::Ui,
    manager:        &mut CoordSysManager,
    selected_count: usize,
    on_apply:       &mut Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)>,
    on_edit:        &mut Option<(String, [[f32; 3]; 3], Vec<CoordSysOp>)>,
) {
    ui.label(egui::RichText::new("Coord Sys Manager").size(16.0).strong());
    ui.separator();

    if manager.entries.is_empty() {
        ui.label(egui::RichText::new("(no entries)").italics().color(egui::Color32::GRAY));
        return;
    }

    let apply_label = if selected_count == 0 {
        "Apply to sel. (0)".to_string()
    } else {
        format!("Apply to sel. ({})", selected_count)
    };

    let mut delete_idx: Option<usize> = None;
    let mut apply_mat:  Option<([[f32; 3]; 3], [[f32; 3]; 3], Vec<CoordSysOp>)> = None;
    let mut edit_entry: Option<(String, [[f32; 3]; 3], Vec<CoordSysOp>)> = None;

    egui::ScrollArea::vertical()
        .id_salt("csys_mgr_scroll")
        .show(ui, |ui| {
            for (i, entry) in manager.entries.iter().enumerate() {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        // Name
                        let dark = ui.visuals().dark_mode;
                        let name_color = if entry.locked {
                            if dark { egui::Color32::from_rgb(150, 150, 200) } else { egui::Color32::from_rgb(100, 100, 160) }
                        } else {
                            if dark { egui::Color32::from_rgb(220, 220, 255) } else { egui::Color32::from_rgb(20, 20, 80) }
                        };
                        ui.label(egui::RichText::new(&entry.name).color(name_color).strong());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Edit (non-locked only)
                            if !entry.locked {
                                if ui.small_button(egui_phosphor::regular::PENCIL).on_hover_text("Edit entry in builder").clicked() {
                                    edit_entry = Some((entry.name.clone(), entry.base_matrix, entry.ops.clone()));
                                }
                            }
                            // Delete (non-locked only)
                            if !entry.locked {
                                if ui.small_button(egui_phosphor::regular::TRASH).on_hover_text("Remove entry").clicked() {
                                    delete_idx = Some(i);
                                }
                            }
                            // Apply
                            let enabled = selected_count > 0;
                            if ui.add_enabled(enabled, egui::Button::new(
                                egui::RichText::new(&apply_label).size(10.5)
                            )).clicked() {
                                apply_mat = Some((entry.matrix, entry.base_matrix, entry.ops.clone()));
                            }
                        });
                    });

                    // Mini compact matrix display (3 cols × 3 rows)
                    let m = entry.matrix;
                    ui.horizontal(|ui| {
                        for col in &m {
                            ui.vertical(|ui| {
                                for &v in col {
                                    ui.label(
                                        egui::RichText::new(format!("{:+.2}", v))
                                            .monospace()
                                            .size(9.5)
                                            .color(egui::Color32::from_rgb(140, 160, 140)),
                                    );
                                }
                            });
                        }
                    });

                    ui.separator();
                });
            }
        });

    if let Some(idx) = delete_idx {
        manager.entries.remove(idx);
    }
    if let Some(mat) = apply_mat {
        *on_apply = Some(mat);
    }
    if let Some(ed) = edit_entry {
        *on_edit = Some(ed);
    }
}
