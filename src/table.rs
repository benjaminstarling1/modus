use egui_extras::{Column, TableBuilder};
use serde::{Serialize, Deserialize};
use crate::csys_builder::CsysOp;

// ─────────────────────────────────────────────────────────────────────────────
// CSV import / export helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Import nodes from a CSV file chosen via a file dialog.
/// Expected columns: id, x, y, z  (extra columns are silently ignored).
/// The dx/dy/dz/rx/ry/rz channel columns are optional; when present they are
/// matched against `channel_names` and stored as indices (0 = "—" if no match).
pub fn import_nodes_csv(rows: &mut Vec<Row>, channel_names: &[String]) -> bool {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_title("Import Nodes CSV")
        .pick_file()
    else { return false; };

    let Ok(mut rdr) = csv::Reader::from_path(&path) else { return false; };
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return false,
    };
    let col = |name: &str| -> Option<usize> {
        headers.iter().position(|h| h.eq_ignore_ascii_case(name))
    };
    let lookup_channel = |s: &str| -> usize {
        if s.is_empty() { return 0; }
        channel_names.iter().position(|n| n == s).map(|i| i + 1).unwrap_or(0)
    };
    let ci_id = col("id");
    let ci_x  = col("x");  let ci_y = col("y"); let ci_z = col("z");
    let ci_dx = col("dx"); let ci_dy = col("dy"); let ci_dz = col("dz");
    let ci_rx = col("rx"); let ci_ry = col("ry"); let ci_rz = col("rz");

    let get = |record: &csv::StringRecord, idx: Option<usize>| -> String {
        idx.and_then(|i| record.get(i)).unwrap_or("").to_string()
    };

    let mut new_rows: Vec<Row> = Vec::new();
    for result in rdr.records() {
        let Ok(rec) = result else { continue; };
        new_rows.push(Row {
            id: get(&rec, ci_id),
            x:  get(&rec, ci_x),
            y:  get(&rec, ci_y),
            z:  get(&rec, ci_z),
            dx: lookup_channel(&get(&rec, ci_dx)),
            dy: lookup_channel(&get(&rec, ci_dy)),
            dz: lookup_channel(&get(&rec, ci_dz)),
            rx: lookup_channel(&get(&rec, ci_rx)),
            ry: lookup_channel(&get(&rec, ci_ry)),
            rz: lookup_channel(&get(&rec, ci_rz)),
            selected: false,
            color_override: None,
            stored_color: [1.0, 1.0, 1.0],
            local_csys: identity_mat3(),
            local_csys_base: identity_mat3(),
            local_csys_ops: Vec::new(),
            show_csys_axes: true,
        });
    }
    if new_rows.is_empty() { return false; }
    *rows = new_rows;
    true
}

/// Export nodes to a CSV file chosen via a save dialog.
pub fn export_nodes_csv(rows: &[Row], channel_names: &[String]) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name("nodes.csv")
        .set_title("Export Nodes CSV")
        .save_file()
    else { return; };

    let channel_name = |idx: usize| -> &str {
        if idx == 0 || channel_names.is_empty() { return ""; }
        channel_names.get(idx - 1).map(|s| s.as_str()).unwrap_or("")
    };

    let Ok(mut wtr) = csv::Writer::from_path(&path) else { return; };
    let _ = wtr.write_record(&["id","x","y","z","dx","dy","dz","rx","ry","rz"]);
    for row in rows {
        let _ = wtr.write_record(&[
            &row.id, &row.x, &row.y, &row.z,
            channel_name(row.dx), channel_name(row.dy), channel_name(row.dz),
            channel_name(row.rx), channel_name(row.ry), channel_name(row.rz),
        ]);
    }
    let _ = wtr.flush();
}

/// Import edges from a CSV file chosen via a file dialog.
/// Expected columns: from, to.
pub fn import_edges_csv(edges: &mut Vec<Edge>) -> bool {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_title("Import Edges CSV")
        .pick_file()
    else { return false; };

    let Ok(mut rdr) = csv::Reader::from_path(&path) else { return false; };
    let headers = match rdr.headers() {
        Ok(h) => h.clone(),
        Err(_) => return false,
    };
    let ci_id   = headers.iter().position(|h| h.eq_ignore_ascii_case("id"));
    let ci_from = headers.iter().position(|h| h.eq_ignore_ascii_case("from"));
    let ci_to   = headers.iter().position(|h| h.eq_ignore_ascii_case("to"));

    let mut new_edges: Vec<Edge> = Vec::new();
    for result in rdr.records() {
        let Ok(rec) = result else { continue; };
        new_edges.push(Edge {
            id:   ci_id  .and_then(|i| rec.get(i)).unwrap_or("").to_string(),
            from: ci_from.and_then(|i| rec.get(i)).unwrap_or("").to_string(),
            to:   ci_to  .and_then(|i| rec.get(i)).unwrap_or("").to_string(),
            color_override: None,
            thickness_override: None,
        });
    }
    if new_edges.is_empty() { return false; }
    *edges = new_edges;
    true
}

/// Export edges to a CSV file chosen via a save dialog.
pub fn export_edges_csv(edges: &[Edge]) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("CSV", &["csv"])
        .set_file_name("edges.csv")
        .set_title("Export Edges CSV")
        .save_file()
    else { return; };

    let Ok(mut wtr) = csv::Writer::from_path(&path) else { return; };
    let _ = wtr.write_record(&["id", "from", "to"]);
    for edge in edges {
        let _ = wtr.write_record(&[&edge.id, &edge.from, &edge.to]);
    }
    let _ = wtr.flush();
}

// ─────────────────────────────────────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────────────────────────────────────

/// Active tab in the top pane.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TableTab {
    #[default]
    Nodes,
    Edges,
    Glyphs,
    Meshes,
}

/// A single node row in the node table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    pub x: String,
    pub y: String,
    pub z: String,
    /// Index into the channel_names slice (0 = "—").
    pub dx: usize,
    pub dy: usize,
    pub dz: usize,
    pub rx: usize,
    pub ry: usize,
    pub rz: usize,
    pub selected: bool,
    /// Per-node color override (RGB 0..1). None = use global node color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_override: Option<[f32; 3]>,
    /// Stored color value preserved when toggling override off.
    #[serde(default = "default_white")]
    pub stored_color: [f32; 3],
    /// Local coordinate system: columns are the local X, Y, Z axes in world space.
    /// Defaults to the 3×3 identity (= aligned with global CSYS).
    #[serde(default = "identity_mat3", skip_serializing_if = "is_identity_mat3")]
    pub local_csys: [[f32; 3]; 3],
    /// The starting base matrix for this CSYS.
    #[serde(default = "identity_mat3", skip_serializing_if = "is_identity_mat3")]
    pub local_csys_base: [[f32; 3]; 3],
    /// Operations applied to the base matrix.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_csys_ops: Vec<CsysOp>,
    /// Whether the local CSYS axes should be drawn in the viewport.
    /// Not exported/imported via CSV; included in file save.
    #[serde(default = "default_true")]
    pub show_csys_axes: bool,
}

impl Default for Row {
    fn default() -> Self {
        Self {
            id:           String::new(),
            x:            String::new(),
            y:            String::new(),
            z:            String::new(),
            dx: 0, dy: 0, dz: 0,
            rx: 0, ry: 0, rz: 0,
            selected:     false,
            color_override: None,
            stored_color: [1.0, 1.0, 1.0],
            local_csys:   identity_mat3(),
            local_csys_base: identity_mat3(),
            local_csys_ops: Vec::new(),
            show_csys_axes: true,
        }
    }
}

pub fn identity_mat3() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

fn default_true() -> bool { true }

fn is_identity_mat3(m: &[[f32; 3]; 3]) -> bool {
    let id = identity_mat3();
    (m[0][0] - id[0][0]).abs() < 1e-6 && (m[0][1] - id[0][1]).abs() < 1e-6 && (m[0][2] - id[0][2]).abs() < 1e-6 &&
    (m[1][0] - id[1][0]).abs() < 1e-6 && (m[1][1] - id[1][1]).abs() < 1e-6 && (m[1][2] - id[1][2]).abs() < 1e-6 &&
    (m[2][0] - id[2][0]).abs() < 1e-6 && (m[2][1] - id[2][1]).abs() < 1e-6 && (m[2][2] - id[2][2]).abs() < 1e-6
}

fn default_white() -> [f32; 3] { [1.0, 1.0, 1.0] }

/// Generate a unique edge ID of the form `E<N>` that does not already exist in `edges`.
/// N is max(existing E<N> suffixes) + 1, falling back to `edges.len() + 1`.
pub fn next_edge_id(edges: &[Edge]) -> String {
    let max_n = edges.iter()
        .filter_map(|e| e.id.strip_prefix('E').and_then(|s| s.parse::<usize>().ok()))
        .max()
        .unwrap_or(0);
    let n = max_n.max(edges.len()) + 1;
    format!("E{}", n)
}

/// An edge connecting two nodes by their IDs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Edge {
    /// User-defined edge identifier.
    #[serde(default)]
    pub id: String,
    /// The node ID of the "from" end.
    pub from: String,
    /// The node ID of the "to" end.
    pub to: String,
    /// Per-edge color override (RGB 0..1). None = use global edge color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_override: Option<[f32; 3]>,
    /// Per-edge thickness override. None = use global edge thickness.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thickness_override: Option<f32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Glyph types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GlyphShape {
    Sphere,
    Cube,
    Cylinder,
    Torus,
}

impl Default for GlyphShape {
    fn default() -> Self { Self::Sphere }
}

impl GlyphShape {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Sphere   => "Sphere",
            Self::Cube     => "Cube",
            Self::Cylinder => "Cylinder",
            Self::Torus    => "Torus",
        }
    }
    pub const ALL: &'static [GlyphShape] = &[
        GlyphShape::Sphere,
        GlyphShape::Cube,
        GlyphShape::Cylinder,
        GlyphShape::Torus,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Glyph {
    pub id: String,
    pub shape: GlyphShape,
    /// Node IDs this glyph is scoped to.
    pub node_ids: Vec<String>,
    /// RGB colour 0..1.
    pub color: [f32; 3],
    /// Uniform scale.
    pub size: f32,
    /// Per-axis stretch multipliers applied on top of `size`.
    #[serde(default = "default_stretch")]
    pub stretch: [f32; 3],
    /// Tube-to-major-radius ratio for Torus glyphs (0..1).
    #[serde(default = "default_tube_ratio")]
    pub tube_ratio: f32,
    /// Offset from the average node position.
    #[serde(default)]
    pub position_offset: [f32; 3],
    #[serde(skip)]
    pub selected: bool,
}

fn default_stretch() -> [f32; 3] { [1.0, 1.0, 1.0] }
fn default_tube_ratio() -> f32 { 0.3 }

impl Default for Glyph {
    fn default() -> Self {
        Self {
            id: String::new(),
            shape: GlyphShape::Sphere,
            node_ids: Vec::new(),
            color: [0.6, 0.6, 0.8],
            size: 0.1,
            stretch: [1.0, 1.0, 1.0],
            tube_ratio: 0.3,
            position_offset: [0.0, 0.0, 0.0],
            selected: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mesh types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mesh {
    pub id: String,
    /// Node IDs this mesh surface spans.
    pub node_ids: Vec<String>,
    /// RGB colour 0..1.
    pub color: [f32; 3],
    /// Surface opacity 0..1.
    pub opacity: f32,
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            id: String::new(),
            node_ids: Vec::new(),
            color: [0.3, 0.6, 0.9],
            opacity: 0.7,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Top pane: tab bar + table dispatch
// ─────────────────────────────────────────────────────────────────────────────

/// Renders the tab bar and the active table.
/// Returns true if any data changed.
pub fn show_top_pane(
    ui: &mut egui::Ui,
    active_tab: &mut TableTab,
    rows: &mut Vec<Row>,
    edges: &mut Vec<Edge>,
    glyphs: &mut Vec<Glyph>,
    meshes: &mut Vec<Mesh>,
    channel_names: &[String],
    clipboard: &mut Option<Row>,
    unit_label: &str,
) -> (bool, bool, bool) {
    let mut changed = false;
    let mut activate_create_edges = false;
    let mut activate_create_nodes = false;

    // ── Outer frame gives the inset look ──────────────────────────────────────
    let v = ui.visuals();
    egui::Frame::new()
        .fill(v.faint_bg_color)
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(6))
        .stroke(v.widgets.noninteractive.bg_stroke)
        .show(ui, |ui| {
            // ── Tab bar ───────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                tab_button(ui, active_tab, TableTab::Nodes,  &format!("{} Nodes",  egui_phosphor::regular::HEXAGON),
                    "Nodes are points in 3D space.\nDefine each node with an ID and X / Y / Z coordinates.\nAssign displacement or rotation channels to animate them.");
                tab_button(ui, active_tab, TableTab::Edges,  &format!("{} Edges",  egui_phosphor::regular::LINE_SEGMENTS),
                    "Edges are straight line segments connecting two nodes.\nDefine each edge with an ID and a From / To node.\nEdge colour and thickness can be overridden per edge.");
                tab_button(ui, active_tab, TableTab::Glyphs, &format!("{} Glyphs", egui_phosphor::regular::DIAMOND),
                    "Glyphs are 3D shapes (sphere, cube, cylinder, torus) attached to one or more nodes.\nTheir position is the average of the assigned nodes plus an optional offset.\nSize, stretch, and colour are all configurable.");
                tab_button(ui, active_tab, TableTab::Meshes, &format!("{} Meshes", egui_phosphor::regular::POLYGON),
                    "Meshes are triangulated surface patches spanning a set of nodes.\nAssign three or more nodes and modus computes a Delaunay triangulation.\nColour and opacity are configurable.");

                // Right-aligned buttons: Add | Export | Import
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    match active_tab {
                        TableTab::Nodes => {
                            if ui.button(format!("{} Add Node", egui_phosphor::regular::PLUS)).clicked() {
                                rows.push(Row::default());
                                changed = true;
                            }
                            if ui.button(format!("{} Create Nodes", egui_phosphor::regular::MAGIC_WAND)).clicked() {
                                activate_create_nodes = true;
                            }
                            ui.add_space(4.0);
                            if ui.button(format!("{} Export CSV", egui_phosphor::regular::EXPORT)).clicked() {
                                export_nodes_csv(rows, channel_names);
                            }
                            if ui.button(format!("{} Import CSV", egui_phosphor::regular::DOWNLOAD_SIMPLE)).clicked() {
                                if import_nodes_csv(rows, channel_names) {
                                    prune_edges(rows, edges);
                                    changed = true;
                                }
                            }
                        }
                        TableTab::Edges => {
                            if ui.button(format!("{} Add Edge", egui_phosphor::regular::PLUS)).clicked() {
                                let id = next_edge_id(edges);
                                edges.push(Edge { id, ..Default::default() });
                                changed = true;
                            }
                            if ui.button(format!("{} Create Edges", egui_phosphor::regular::LINE_SEGMENTS)).clicked() {
                                activate_create_edges = true;
                            }
                            ui.add_space(4.0);
                            if ui.button(format!("{} Export CSV", egui_phosphor::regular::EXPORT)).clicked() {
                                export_edges_csv(edges);
                            }
                            if ui.button(format!("{} Import CSV", egui_phosphor::regular::DOWNLOAD_SIMPLE)).clicked() {
                                if import_edges_csv(edges) {
                                    changed = true;
                                }
                            }
                        }
                        TableTab::Glyphs => {
                            if ui.button(format!("{} Add Glyph", egui_phosphor::regular::PLUS)).clicked() {
                                glyphs.push(Glyph::default());
                                changed = true;
                            }
                        }
                        TableTab::Meshes => {
                            if ui.button(format!("{} Add Mesh", egui_phosphor::regular::PLUS)).clicked() {
                                meshes.push(Mesh::default());
                                changed = true;
                            }
                        }
                    }
                });
            });

            ui.separator();

            // ── Active table ──────────────────────────────────────────────────
            egui::ScrollArea::both()
                .id_salt("top_pane_scroll")
                .show(ui, |ui| {
                    match active_tab {
                        TableTab::Nodes => {
                            if show_node_table(ui, rows, channel_names, clipboard, unit_label) {
                                // When nodes change, prune stale edge endpoints.
                                prune_edges(rows, edges);
                                changed = true;
                            }
                        }
                        TableTab::Edges => {
                            if show_edge_table(ui, edges, rows) {
                                changed = true;
                            }
                        }
                        TableTab::Glyphs => {
                            if show_glyph_table(ui, glyphs, rows) {
                                changed = true;
                            }
                        }
                        TableTab::Meshes => {
                            if show_mesh_table(ui, meshes, rows) {
                                changed = true;
                            }
                        }
                    }
                });
        });

    (changed, activate_create_edges, activate_create_nodes)
}

/// Styled toggle button for the tab bar.
fn tab_button(ui: &mut egui::Ui, active: &mut TableTab, variant: TableTab, label: &str, tooltip: &str) {
    let selected = *active == variant;
    let resp = ui.add(
        egui::Button::new(label)
            .selected(selected)
            .corner_radius(egui::CornerRadius::same(4)),
    );
    if resp.on_hover_text(tooltip).clicked() {
        *active = variant;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Node table
// ─────────────────────────────────────────────────────────────────────────────

fn show_node_table(
    ui: &mut egui::Ui,
    rows: &mut Vec<Row>,
    channel_names: &[String],
    clipboard: &mut Option<Row>,
    unit_label: &str,
) -> bool {
    let mut changed = false;
    let mut delete_idx: Option<usize> = None;

    let text_h = ui.text_style_height(&egui::TextStyle::Body);
    let row_h = text_h + 10.0;

    // Ctrl+C / Ctrl+V copy-paste
    let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.mac_cmd);
    if ctrl && ui.input(|i| i.key_pressed(egui::Key::C)) {
        if let Some(r) = rows.iter().find(|r| r.selected) {
            *clipboard = Some(r.clone());
        }
    }
    if ctrl && ui.input(|i| i.key_pressed(egui::Key::V)) {
        if let Some(src) = clipboard.clone() {
            let mut new_row = src.clone();
            new_row.selected = false;
            rows.push(new_row);
            changed = true;
        }
    }

    let dropdown_label = |idx: usize, names: &[String]| -> String {
        if idx == 0 || names.is_empty() {
            "—".to_string()
        } else {
            names.get(idx - 1).cloned().unwrap_or_else(|| "—".to_string())
        }
    };

    let avail = ui.available_width();
    let dd_w = ((avail - 32.0 - 80.0 - 70.0 * 3.0 - 36.0) / 6.0).max(80.0);

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(32.0).at_least(24.0))
        .column(Column::initial(80.0).at_least(60.0))
        .column(Column::initial(70.0).at_least(50.0))
        .column(Column::initial(70.0).at_least(50.0))
        .column(Column::initial(70.0).at_least(50.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(dd_w).at_least(70.0))
        .column(Column::initial(36.0).at_least(32.0))
        .min_scrolled_height(0.0)
        .header(row_h, |mut header| {
            for label in ["", "ID",
                &format!("X ({})", unit_label),
                &format!("Y ({})", unit_label),
                &format!("Z ({})", unit_label),
                "dX", "dY", "dZ", "rX", "rY", "rZ", ""] {
                header.col(|ui| { ui.strong(label); });
            }
        })
        .body(|mut body| {
            let n = rows.len();
            for i in 0..n {
                let selected = rows[i].selected;
                body.row(row_h, |mut row_ui| {
                    // Row selector
                    row_ui.col(|ui| {
                        let label = if selected { egui_phosphor::regular::CARET_RIGHT } else { &format!("{}", i + 1) };
                        if ui.selectable_label(selected, label).clicked() {
                            for r in rows.iter_mut() { r.selected = false; }
                            rows[i].selected = !selected;
                        }
                    });
                    // ID
                    row_ui.col(|ui| {
                        if ui.text_edit_singleline(&mut rows[i].id).changed() { changed = true; }
                    });
                    // X Y Z
                    row_ui.col(|ui| { if float_input(ui, &mut rows[i].x) { changed = true; } });
                    row_ui.col(|ui| { if float_input(ui, &mut rows[i].y) { changed = true; } });
                    row_ui.col(|ui| { if float_input(ui, &mut rows[i].z) { changed = true; } });
                    // dX dY dZ rX rY rZ
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 0, &mut rows[i].dx, channel_names, &dropdown_label) { changed = true; } });
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 1, &mut rows[i].dy, channel_names, &dropdown_label) { changed = true; } });
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 2, &mut rows[i].dz, channel_names, &dropdown_label) { changed = true; } });
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 3, &mut rows[i].rx, channel_names, &dropdown_label) { changed = true; } });
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 4, &mut rows[i].ry, channel_names, &dropdown_label) { changed = true; } });
                    row_ui.col(|ui| { if channel_dropdown(ui, i, 5, &mut rows[i].rz, channel_names, &dropdown_label) { changed = true; } });
                    // Delete
                    row_ui.col(|ui| {
                        if ui.button(egui_phosphor::regular::TRASH).clicked() { delete_idx = Some(i); }
                    });
                });
            }
        });

    if let Some(idx) = delete_idx {
        rows.remove(idx);
        changed = true;
    }

    changed
}

// ─────────────────────────────────────────────────────────────────────────────
// Edge table
// ─────────────────────────────────────────────────────────────────────────────

fn show_edge_table(
    ui: &mut egui::Ui,
    edges: &mut Vec<Edge>,
    rows: &[Row],
) -> bool {
    let mut changed = false;
    let mut delete_idx: Option<usize> = None;

    // Build the list of node IDs for the dropdowns.
    // Nodes with empty IDs show as "(row N)".
    let node_labels: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.id.trim().is_empty() {
                format!("node {}", i + 1)
            } else {
                r.id.clone()
            }
        })
        .collect();

    let text_h = ui.text_style_height(&egui::TextStyle::Body);
    let row_h = text_h + 10.0;

    if rows.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Add nodes first to define edges between them.")
                    .color(egui::Color32::from_rgb(120, 120, 140))
                    .italics(),
            );
        });
        return false;
    }

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(32.0).at_least(28.0))        // #
        .column(Column::initial(80.0).at_least(60.0))        // ID
        .column(Column::initial(120.0).at_least(80.0))       // From
        .column(Column::initial(120.0).at_least(80.0))       // To
        .column(Column::initial(32.0).at_least(28.0))        // Delete
        .min_scrolled_height(0.0)
        .header(row_h, |mut header| {
            for label in ["#", "ID", "From", "To", ""] {
                header.col(|ui| { ui.strong(label); });
            }
        })
        .body(|mut body| {
            let n = edges.len();
            for i in 0..n {
                body.row(row_h, |mut row_ui| {
                    // Row number
                    row_ui.col(|ui| {
                        ui.label(format!("{}", i + 1));
                    });

                    // Edge ID
                    row_ui.col(|ui| {
                        if ui.add(
                            egui::TextEdit::singleline(&mut edges[i].id)
                                .desired_width(ui.available_width()),
                        ).changed() {
                            changed = true;
                        }
                    });

                    // From node dropdown
                    row_ui.col(|ui| {
                        if node_id_dropdown(
                            ui,
                            egui::Id::new(("edge_from", i)),
                            &mut edges[i].from,
                            &node_labels,
                            rows,
                        ) {
                            changed = true;
                        }
                    });

                    // To node dropdown
                    row_ui.col(|ui| {
                        if node_id_dropdown(
                            ui,
                            egui::Id::new(("edge_to", i)),
                            &mut edges[i].to,
                            &node_labels,
                            rows,
                        ) {
                            changed = true;
                        }
                    });

                    // Delete
                    row_ui.col(|ui| {
                        if ui.button(egui_phosphor::regular::TRASH).clicked() {
                            delete_idx = Some(i);
                        }
                    });
                });
            }
        });

    if let Some(idx) = delete_idx {
        edges.remove(idx);
        changed = true;
    }

    changed
}
pub fn show_glyph_table(
    ui: &mut egui::Ui,
    glyphs: &mut Vec<Glyph>,
    rows: &[Row],
) -> bool {
    let mut changed = false;
    let mut delete_idx: Option<usize> = None;

    // Build labels from rows
    let node_labels: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.id.is_empty() {
                format!("Node {}", i + 1)
            } else {
                r.id.clone()
            }
        })
        .collect();

    if glyphs.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Add glyphs to attach shapes to nodes.")
                    .color(egui::Color32::from_rgb(120, 120, 140))
                    .italics(),
            );
        });
        return false;
    }

    let text_h = ui.text_style_height(&egui::TextStyle::Body);
    let row_h = text_h + 10.0;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(30.0).at_least(28.0))   // #
        .column(Column::initial(80.0).at_least(50.0))    // ID
        .column(Column::initial(90.0).at_least(70.0))    // Shape
        .column(Column::initial(140.0).at_least(80.0))   // Nodes
        .column(Column::initial(60.0).at_least(40.0))    // Size
        .column(Column::initial(36.0).at_least(32.0))    // Color
        .column(Column::initial(36.0).at_least(32.0))    // Delete
        .min_scrolled_height(0.0)
        .header(row_h, |mut header| {
            for label in ["#", "ID", "Shape", "Nodes", "Size", "Color", ""] {
                header.col(|ui| { ui.strong(label); });
            }
        })
        .body(|mut body| {
            let n = glyphs.len();
            for i in 0..n {
                body.row(row_h, |mut row_ui| {
                    // Row number
                    row_ui.col(|ui| {
                        ui.label(format!("{}", i + 1));
                    });

                    // ID
                    row_ui.col(|ui| {
                        if ui.add(
                            egui::TextEdit::singleline(&mut glyphs[i].id)
                                .desired_width(ui.available_width()),
                        ).changed() {
                            changed = true;
                        }
                    });

                    // Shape dropdown
                    row_ui.col(|ui| {
                        let current = glyphs[i].shape.label();
                        egui::ComboBox::from_id_salt(egui::Id::new(("glyph_shape", i)))
                            .selected_text(current)
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for s in GlyphShape::ALL {
                                    if ui.selectable_value(&mut glyphs[i].shape, s.clone(), s.label()).changed() {
                                        changed = true;
                                    }
                                }
                            });
                    });

                    // Nodes (multi-select popup)
                    row_ui.col(|ui| {
                        let summary = if glyphs[i].node_ids.is_empty() {
                            "— select —".to_string()
                        } else {
                            glyphs[i].node_ids.join(", ")
                        };
                        egui::ComboBox::from_id_salt(egui::Id::new(("glyph_nodes", i)))
                            .selected_text(&summary)
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for (ri, row) in rows.iter().enumerate() {
                                    let label = &node_labels[ri];
                                    let id = &row.id;
                                    if id.is_empty() { continue; }
                                    let mut is_in = glyphs[i].node_ids.contains(id);
                                    if ui.checkbox(&mut is_in, label).changed() {
                                        if is_in {
                                            glyphs[i].node_ids.push(id.clone());
                                        } else {
                                            glyphs[i].node_ids.retain(|x| x != id);
                                        }
                                        changed = true;
                                    }
                                }
                            });
                    });

                    // Size
                    row_ui.col(|ui| {
                        if ui.add(
                            egui::DragValue::new(&mut glyphs[i].size)
                                .speed(0.005)
                                .range(0.001..=100.0),
                        ).changed() {
                            changed = true;
                        }
                    });

                    // Color
                    row_ui.col(|ui| {
                        if ui.color_edit_button_rgb(&mut glyphs[i].color).changed() {
                            changed = true;
                        }
                    });

                    // Delete
                    row_ui.col(|ui| {
                        if ui.button(egui_phosphor::regular::TRASH).clicked() {
                            delete_idx = Some(i);
                        }
                    });
                });
            }
        });

    if let Some(idx) = delete_idx {
        glyphs.remove(idx);
        changed = true;
    }

    changed
}

// ─────────────────────────────────────────────────────────────────────────────
// Mesh table
// ─────────────────────────────────────────────────────────────────────────────

fn show_mesh_table(
    ui: &mut egui::Ui,
    meshes: &mut Vec<Mesh>,
    rows: &[Row],
) -> bool {
    let mut changed = false;
    let mut delete_idx: Option<usize> = None;

    let node_labels: Vec<String> = rows
        .iter()
        .enumerate()
        .map(|(i, r)| {
            if r.id.is_empty() {
                format!("Node {}", i + 1)
            } else {
                r.id.clone()
            }
        })
        .collect();

    if meshes.is_empty() {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new("Add meshes to create triangulated surfaces between nodes.")
                    .color(egui::Color32::from_rgb(120, 120, 140))
                    .italics(),
            );
        });
        return false;
    }

    let text_h = ui.text_style_height(&egui::TextStyle::Body);
    let row_h = text_h + 10.0;

    TableBuilder::new(ui)
        .striped(true)
        .resizable(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(30.0).at_least(28.0))   // #
        .column(Column::initial(80.0).at_least(50.0))    // ID
        .column(Column::initial(160.0).at_least(100.0))  // Nodes
        .column(Column::initial(36.0).at_least(32.0))    // Color
        .column(Column::initial(60.0).at_least(40.0))    // Opacity
        .column(Column::initial(36.0).at_least(32.0))    // Delete
        .min_scrolled_height(0.0)
        .header(row_h, |mut header| {
            for label in ["#", "ID", "Nodes", "Color", "Opacity", ""] {
                header.col(|ui| { ui.strong(label); });
            }
        })
        .body(|mut body| {
            let n = meshes.len();
            for i in 0..n {
                body.row(row_h, |mut row_ui| {
                    // Row number
                    row_ui.col(|ui| {
                        ui.label(format!("{}", i + 1));
                    });

                    // ID
                    row_ui.col(|ui| {
                        if ui.add(
                            egui::TextEdit::singleline(&mut meshes[i].id)
                                .desired_width(ui.available_width()),
                        ).changed() {
                            changed = true;
                        }
                    });

                    // Nodes (multi-select popup)
                    row_ui.col(|ui| {
                        let summary = if meshes[i].node_ids.is_empty() {
                            "— select —".to_string()
                        } else {
                            meshes[i].node_ids.join(", ")
                        };
                        egui::ComboBox::from_id_salt(egui::Id::new(("mesh_nodes", i)))
                            .selected_text(&summary)
                            .width(ui.available_width())
                            .show_ui(ui, |ui| {
                                for (ri, row) in rows.iter().enumerate() {
                                    let label = &node_labels[ri];
                                    let id = &row.id;
                                    if id.is_empty() { continue; }
                                    let mut is_in = meshes[i].node_ids.contains(id);
                                    if ui.checkbox(&mut is_in, label).changed() {
                                        if is_in {
                                            meshes[i].node_ids.push(id.clone());
                                        } else {
                                            meshes[i].node_ids.retain(|x| x != id);
                                        }
                                        changed = true;
                                    }
                                }
                            });
                    });

                    // Color
                    row_ui.col(|ui| {
                        if ui.color_edit_button_rgb(&mut meshes[i].color).changed() {
                            changed = true;
                        }
                    });

                    // Opacity
                    row_ui.col(|ui| {
                        if ui.add(
                            egui::DragValue::new(&mut meshes[i].opacity)
                                .speed(0.01)
                                .range(0.0..=1.0),
                        ).changed() {
                            changed = true;
                        }
                    });

                    // Delete
                    row_ui.col(|ui| {
                        if ui.button(egui_phosphor::regular::TRASH).clicked() {
                            delete_idx = Some(i);
                        }
                    });
                });
            }
        });

    if let Some(idx) = delete_idx {
        meshes.remove(idx);
        changed = true;
    }

    changed
}

/// ComboBox that selects a node by displaying its label; stores the node's `id` string.
pub fn node_id_dropdown(
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut String,
    node_labels: &[String],
    rows: &[Row],
) -> bool {
    // Determine display text: match stored ID against rows.
    let current_label = rows
        .iter()
        .zip(node_labels.iter())
        .find(|(r, _)| &r.id == value)
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| {
            if value.is_empty() {
                "— select —".to_string()
            } else {
                format!("{} {value} (missing)", egui_phosphor::regular::WARNING)
            }
        });

    // Colour the label red if the node no longer exists.
    let is_missing = !value.is_empty() && !rows.iter().any(|r| &r.id == value);
    let text = if is_missing {
        egui::RichText::new(&current_label).color(egui::Color32::from_rgb(220, 80, 80))
    } else {
        egui::RichText::new(&current_label)
    };

    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(text)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if ui.selectable_value(value, String::new(), "— select —").changed() {
                changed = true;
            }
            for (row, label) in rows.iter().zip(node_labels.iter()) {
                if ui.selectable_value(value, row.id.clone(), label.as_str()).changed() {
                    changed = true;
                }
            }
        });
    changed
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// When a node is deleted or its ID changes, prune edges whose endpoints can
/// no longer be resolved. (Only endpoints that were non-empty but are now
/// unresolvable are cleared — edges with "" endpoints stay as placeholders.)
pub fn prune_edges(rows: &[Row], edges: &mut Vec<Edge>) {
    let ids: std::collections::HashSet<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    for edge in edges.iter_mut() {
        if !edge.from.is_empty() && !ids.contains(edge.from.as_str()) {
            edge.from.clear();
        }
        if !edge.to.is_empty() && !ids.contains(edge.to.as_str()) {
            edge.to.clear();
        }
    }
}

/// Single-line text input that turns red if the content is not a valid float.
fn float_input(ui: &mut egui::Ui, value: &mut String) -> bool {
    let valid = value.is_empty() || value.parse::<f64>().is_ok();
    let r = ui.add(
        egui::TextEdit::singleline(value)
            .text_color_opt(if valid { None } else { Some(egui::Color32::RED) }),
    );
    r.changed()
}

/// Combo-box dropdown populated from `channel_names`.
fn channel_dropdown(
    ui: &mut egui::Ui,
    row: usize,
    col: usize,
    value: &mut usize,
    channel_names: &[String],
    label_fn: &dyn Fn(usize, &[String]) -> String,
) -> bool {
    let id = egui::Id::new(("dd", row, col));
    let current_label = label_fn(*value, channel_names);
    let mut changed = false;
    egui::ComboBox::from_id_salt(id)
        .selected_text(&current_label)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if ui.selectable_value(value, 0, "—").changed() { changed = true; }
            for (i, name) in channel_names.iter().enumerate() {
                if ui.selectable_value(value, i + 1, name).changed() { changed = true; }
            }
        });
    changed
}

/// Parse X, Y, Z strings into f32 coordinates; returns None if any is invalid.
pub fn row_position(row: &Row) -> Option<[f32; 3]> {
    let x = row.x.parse::<f32>().ok()?;
    let y = row.y.parse::<f32>().ok()?;
    let z = row.z.parse::<f32>().ok()?;
    Some([x, y, z])
}
