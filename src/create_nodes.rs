use crate::csys_builder::CsysManager;
use crate::persist::DistanceUnit;
use crate::table::{Row, identity_mat3, row_position};

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum CreateNodesMode {
    None,
    CopyWithOffset,
    Interpolate,
}

impl CreateNodesMode {
    fn label(&self) -> &'static str {
        match self {
            Self::None           => "Select a method…",
            Self::CopyWithOffset => "Copy with Offset",
            Self::Interpolate    => "Interpolate Between Nodes",
        }
    }
}

pub struct CreateNodesState {
    pub mode: CreateNodesMode,
    // Copy-with-offset
    pub offset: [f32; 3],
    pub offset_unit: DistanceUnit,
    pub csys_index: usize,
    // Interpolate
    pub node_a: String,
    pub node_b: String,
    pub count: usize,
}

impl Default for CreateNodesState {
    fn default() -> Self {
        Self {
            mode: CreateNodesMode::None,
            offset: [0.0; 3],
            offset_unit: DistanceUnit::default(),
            csys_index: 0,
            node_a: String::new(),
            node_b: String::new(),
            count: 1,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dialog window
// ─────────────────────────────────────────────────────────────────────────────

/// Show the Create Nodes floating window.
///
/// Returns `Some(vec_of_new_rows)` when the user clicks Create, else `None`.
pub fn show_create_nodes_window(
    ctx:        &egui::Context,
    open:       &mut bool,
    state:      &mut CreateNodesState,
    rows:       &[Row],
    manager:    &CsysManager,
    model_unit: &DistanceUnit,
) -> Option<Vec<Row>> {
    if !*open { return None; }

    let mut result: Option<Vec<Row>> = None;

    egui::Window::new("Create Nodes")
        .resizable(true)
        .default_width(360.0)
        .min_width(300.0)
        .collapsible(false)
        .open(open)
        .show(ctx, |ui| {
            // ── Mode selector ────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.label("Method:");
                egui::ComboBox::from_id_salt("create_nodes_mode")
                    .selected_text(state.mode.label())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.mode, CreateNodesMode::CopyWithOffset,
                            CreateNodesMode::CopyWithOffset.label(),
                        );
                        ui.selectable_value(
                            &mut state.mode, CreateNodesMode::Interpolate,
                            CreateNodesMode::Interpolate.label(),
                        );
                    });
            });

            ui.separator();

            match state.mode {
                CreateNodesMode::None => {
                    ui.add_space(20.0);
                    ui.vertical_centered(|ui| {
                        ui.label(
                            egui::RichText::new("Choose a creation method above.")
                                .italics()
                                .color(egui::Color32::from_rgb(140, 140, 160)),
                        );
                    });
                    ui.add_space(20.0);
                }

                CreateNodesMode::CopyWithOffset => {
                    result = copy_with_offset_ui(ui, state, rows, manager, model_unit);
                }

                CreateNodesMode::Interpolate => {
                    result = interpolate_ui(ui, state, rows);
                }
            }
        });

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Copy with Offset
// ─────────────────────────────────────────────────────────────────────────────

fn copy_with_offset_ui(
    ui:         &mut egui::Ui,
    state:      &mut CreateNodesState,
    rows:       &[Row],
    manager:    &CsysManager,
    model_unit: &DistanceUnit,
) -> Option<Vec<Row>> {
    let selected: Vec<&Row> = rows.iter().filter(|r| r.selected).collect();
    let sel_count = selected.len();

    // CSYS selector
    ui.horizontal(|ui| {
        ui.label("Coordinate System:");
        let current_name = manager.entries
            .get(state.csys_index)
            .map(|e| e.name.as_str())
            .unwrap_or("Global");
        egui::ComboBox::from_id_salt("create_nodes_csys")
            .selected_text(current_name)
            .width(140.0)
            .show_ui(ui, |ui| {
                for (i, entry) in manager.entries.iter().enumerate() {
                    ui.selectable_value(&mut state.csys_index, i, &entry.name);
                }
            });
    });

    ui.add_space(4.0);

    // Offset inputs with unit selector
    ui.horizontal(|ui| {
        ui.label("Offset:");
        ui.add_space(4.0);
        ui.label("dX:");
        ui.add(egui::DragValue::new(&mut state.offset[0]).speed(0.1));
        ui.label("dY:");
        ui.add(egui::DragValue::new(&mut state.offset[1]).speed(0.1));
        ui.label("dZ:");
        ui.add(egui::DragValue::new(&mut state.offset[2]).speed(0.1));
        ui.add_space(6.0);
        egui::ComboBox::from_id_salt("create_nodes_offset_unit")
            .selected_text(state.offset_unit.label())
            .width(52.0)
            .show_ui(ui, |ui| {
                for u in DistanceUnit::ALL {
                    ui.selectable_value(&mut state.offset_unit, u.clone(), u.label());
                }
            });
    });
    if state.offset_unit != *model_unit {
        ui.label(
            egui::RichText::new(format!(
                "Offset entered in {} — will be converted to {} on create.",
                state.offset_unit.label(), model_unit.label()
            ))
            .italics()
            .color(egui::Color32::from_rgb(140, 180, 220))
            .size(11.0),
        );
    }

    ui.add_space(8.0);

    // Summary
    let info_color = egui::Color32::from_rgb(140, 180, 220);
    if sel_count == 0 {
        ui.label(
            egui::RichText::new(format!("{} Select nodes in the table first.", egui_phosphor::regular::WARNING))
                .color(egui::Color32::from_rgb(220, 160, 80)),
        );
    } else {
        ui.label(
            egui::RichText::new(format!("Will copy {} selected node(s) with offset.", sel_count))
                .color(info_color),
        );
    }

    ui.add_space(8.0);

    // Create button
    let can_create = sel_count > 0
        && (state.offset[0].abs() > 1e-9
            || state.offset[1].abs() > 1e-9
            || state.offset[2].abs() > 1e-9);

    if ui.add_enabled(can_create, egui::Button::new(format!("{}  Create", egui_phosphor::regular::CHECK))).clicked() {
        // Convert offset from the user's chosen unit to the model unit.
        let unit_factor = state.offset_unit.convert_factor(model_unit) as f32;
        let converted_offset = [
            state.offset[0] * unit_factor,
            state.offset[1] * unit_factor,
            state.offset[2] * unit_factor,
        ];

        // Transform offset from selected CSYS to global
        let csys_mat = manager.entries
            .get(state.csys_index)
            .map(|e| e.matrix)
            .unwrap_or_else(identity_mat3);
        let global_offset = mat3_transform(csys_mat, converted_offset);

        let mut new_rows: Vec<Row> = Vec::new();
        for src in &selected {
            if let Some([x, y, z]) = row_position(src) {
                let mut nr = Row {
                    id: format!("{}_copy", src.id),
                    x: format!("{}", x + global_offset[0]),
                    y: format!("{}", y + global_offset[1]),
                    z: format!("{}", z + global_offset[2]),
                    dx: src.dx,
                    dy: src.dy,
                    dz: src.dz,
                    rx: src.rx,
                    ry: src.ry,
                    rz: src.rz,
                    selected: false,
                    color_override: src.color_override,
                    stored_color: src.stored_color,
                    local_csys: src.local_csys,
                    local_csys_base: src.local_csys_base,
                    local_csys_ops: src.local_csys_ops.clone(),
                    show_csys_axes: src.show_csys_axes,
                };
                // Ensure unique ID if original is empty
                if src.id.is_empty() {
                    nr.id = String::new();
                }
                new_rows.push(nr);
            }
        }
        if !new_rows.is_empty() {
            return Some(new_rows);
        }
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Interpolate Between Nodes
// ─────────────────────────────────────────────────────────────────────────────

fn interpolate_ui(
    ui:    &mut egui::Ui,
    state: &mut CreateNodesState,
    rows:  &[Row],
) -> Option<Vec<Row>> {
    // Build node labels
    let node_labels: Vec<(String, String)> = rows.iter().enumerate().map(|(i, r)| {
        let label = if r.id.is_empty() {
            format!("Node {}", i + 1)
        } else {
            r.id.clone()
        };
        (r.id.clone(), label)
    }).filter(|(id, _)| !id.is_empty()).collect();

    // Node A
    ui.horizontal(|ui| {
        ui.label("Node A:");
        let current_a = if state.node_a.is_empty() { "— select —" } else { &state.node_a };
        egui::ComboBox::from_id_salt("interp_node_a")
            .selected_text(current_a)
            .width(140.0)
            .show_ui(ui, |ui| {
                for (id, label) in &node_labels {
                    ui.selectable_value(&mut state.node_a, id.clone(), label);
                }
            });
    });

    // Node B
    ui.horizontal(|ui| {
        ui.label("Node B:");
        let current_b = if state.node_b.is_empty() { "— select —" } else { &state.node_b };
        egui::ComboBox::from_id_salt("interp_node_b")
            .selected_text(current_b)
            .width(140.0)
            .show_ui(ui, |ui| {
                for (id, label) in &node_labels {
                    ui.selectable_value(&mut state.node_b, id.clone(), label);
                }
            });
    });

    ui.add_space(4.0);

    // Count
    ui.horizontal(|ui| {
        ui.label("Number of nodes:");
        ui.add(
            egui::DragValue::new(&mut state.count)
                .range(1..=100)
                .speed(0.2),
        );
    });

    ui.add_space(8.0);

    // Validation
    let pos_a = rows.iter().find(|r| r.id == state.node_a).and_then(|r| row_position(r));
    let pos_b = rows.iter().find(|r| r.id == state.node_b).and_then(|r| row_position(r));
    let same_node = !state.node_a.is_empty() && state.node_a == state.node_b;

    if state.node_a.is_empty() || state.node_b.is_empty() {
        ui.label(
            egui::RichText::new("Select both nodes above.")
                .italics()
                .color(egui::Color32::from_rgb(140, 140, 160)),
        );
    } else if same_node {
        ui.label(
            egui::RichText::new(format!("{} Node A and Node B must be different.", egui_phosphor::regular::WARNING))
                .color(egui::Color32::from_rgb(220, 160, 80)),
        );
    } else {
        let info = format!(
            "Will create {} node(s) between {} and {}.",
            state.count, state.node_a, state.node_b,
        );
        ui.label(
            egui::RichText::new(info)
                .color(egui::Color32::from_rgb(140, 180, 220)),
        );
    }

    ui.add_space(8.0);

    let can_create = pos_a.is_some() && pos_b.is_some() && !same_node && state.count > 0;

    if ui.add_enabled(can_create, egui::Button::new(format!("{}  Create", egui_phosphor::regular::CHECK))).clicked() {
        let [ax, ay, az] = pos_a.unwrap();
        let [bx, by, bz] = pos_b.unwrap();
        let n = state.count;
        let mut new_rows: Vec<Row> = Vec::new();
        for i in 1..=n {
            let t = i as f32 / (n + 1) as f32;
            new_rows.push(Row {
                id: format!("{}_{}_i{}", state.node_a, state.node_b, i),
                x: format!("{}", ax + (bx - ax) * t),
                y: format!("{}", ay + (by - ay) * t),
                z: format!("{}", az + (bz - az) * t),
                ..Row::default()
            });
        }
        if !new_rows.is_empty() {
            return Some(new_rows);
        }
    }

    None
}

// ─────────────────────────────────────────────────────────────────────────────
// Math helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Transform a vector by a 3×3 matrix (row-major: rows = axes).
/// result = M * v  (each row of M is dotted with v).
fn mat3_transform(m: [[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}
