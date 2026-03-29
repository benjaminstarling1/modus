use serde::{Serialize, Deserialize};
use crate::coord_sys_builder::CoordSysOp;

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

/// The local coordinate system for a node, stored as a flattened group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalCoordSys {
    #[serde(rename = "local_csys", default = "identity_mat3")]
    pub matrix: [[f32; 3]; 3],
    #[serde(rename = "local_csys_base", default = "identity_mat3")]
    pub base: [[f32; 3]; 3],
    #[serde(rename = "local_csys_ops", default)]
    pub ops: Vec<CoordSysOp>,
}

impl Default for LocalCoordSys {
    fn default() -> Self {
        Self {
            matrix: identity_mat3(),
            base:   identity_mat3(),
            ops:    Vec::new(),
        }
    }
}

/// A single node row in the node table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Row {
    pub id: String,
    #[serde(rename = "x")]
    pub x_str: String,
    #[serde(rename = "y")]
    pub y_str: String,
    #[serde(rename = "z")]
    pub z_str: String,
    /// Index into the channel_names slice (0 = "—").
    #[serde(rename = "dx")]
    pub channel_dx: usize,
    #[serde(rename = "dy")]
    pub channel_dy: usize,
    #[serde(rename = "dz")]
    pub channel_dz: usize,
    #[serde(rename = "rx")]
    pub channel_rx: usize,
    #[serde(rename = "ry")]
    pub channel_ry: usize,
    #[serde(rename = "rz")]
    pub channel_rz: usize,
    pub selected: bool,
    /// Per-node color override (RGB 0..1). None = use global node color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color_override: Option<[f32; 3]>,
    /// Stored color value preserved when toggling override off.
    #[serde(default = "default_white")]
    pub stored_color: [f32; 3],
    /// Local coordinate system (matrix, base, ops) — flattened into the Row JSON.
    #[serde(flatten)]
    pub local_coord_sys: LocalCoordSys,
    /// Whether the local coord sys axes should be drawn in the viewport.
    /// Not exported/imported via CSV; included in file save.
    #[serde(rename = "show_csys_axes", default = "default_true")]
    pub show_coord_sys_axes: bool,
}

impl Default for Row {
    fn default() -> Self {
        Self {
            id:           String::new(),
            x_str:        String::new(),
            y_str:        String::new(),
            z_str:        String::new(),
            channel_dx: 0, channel_dy: 0, channel_dz: 0,
            channel_rx: 0, channel_ry: 0, channel_rz: 0,
            selected:     false,
            color_override: None,
            stored_color: [1.0, 1.0, 1.0],
            local_coord_sys: LocalCoordSys { matrix: identity_mat3(), base: identity_mat3(), ops: vec![] },
            show_coord_sys_axes: true,
        }
    }
}

pub fn identity_mat3() -> [[f32; 3]; 3] {
    [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]
}

pub fn default_true() -> bool { true }


fn default_white() -> [f32; 3] { [1.0, 1.0, 1.0] }

/// Generate a unique edge ID of the form `E<N>` that does not already exist in `edges`.
/// N is max(existing E<N> suffixes) + 1, falling back to `edges.len() + 1`.
pub fn generate_edge_id(edges: &[Edge]) -> String {
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
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// When a node is deleted or its ID changes, prune edges whose endpoints can
/// no longer be resolved. (Only endpoints that were non-empty but are now
/// unresolvable are cleared — edges with "" endpoints stay as placeholders.)
pub fn remove_dangling_edges(rows: &[Row], edges: &mut Vec<Edge>) {
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

/// Parse X, Y, Z strings into f32 coordinates; returns None if any is invalid.
pub fn row_position(row: &Row) -> Option<[f32; 3]> {
    let x = row.x_str.parse::<f32>().ok()?;
    let y = row.y_str.parse::<f32>().ok()?;
    let z = row.z_str.parse::<f32>().ok()?;
    Some([x, y, z])
}
