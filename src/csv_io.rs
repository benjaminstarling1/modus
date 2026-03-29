use crate::entities::{Row, Edge, LocalCoordSys, identity_mat3};

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
            x_str:  get(&rec, ci_x),
            y_str:  get(&rec, ci_y),
            z_str:  get(&rec, ci_z),
            channel_dx: lookup_channel(&get(&rec, ci_dx)),
            channel_dy: lookup_channel(&get(&rec, ci_dy)),
            channel_dz: lookup_channel(&get(&rec, ci_dz)),
            channel_rx: lookup_channel(&get(&rec, ci_rx)),
            channel_ry: lookup_channel(&get(&rec, ci_ry)),
            channel_rz: lookup_channel(&get(&rec, ci_rz)),
            selected: false,
            color_override: None,
            stored_color: [1.0, 1.0, 1.0],
            local_coord_sys: LocalCoordSys { matrix: identity_mat3(), base: identity_mat3(), ops: vec![] },
            show_coord_sys_axes: true,
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
            &row.id, &row.x_str, &row.y_str, &row.z_str,
            channel_name(row.channel_dx), channel_name(row.channel_dy), channel_name(row.channel_dz),
            channel_name(row.channel_rx), channel_name(row.channel_ry), channel_name(row.channel_rz),
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
