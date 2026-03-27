import os

files = [
    'src/table.rs', 'src/csys_builder.rs', 'src/create_nodes.rs',
    'src/app.rs', 'src/persist.rs', 'src/fft.rs', 'src/export_video.rs'
]

replacements = {
    '"⬡  Nodes"': 'format!("{} Nodes", egui_phosphor::regular::HEXAGON)',
    '"╱  Edges"': 'format!("{} Edges", egui_phosphor::regular::LINE_SEGMENTS)',
    '"◆  Glyphs"': 'format!("{} Glyphs", egui_phosphor::regular::DIAMOND)',
    '"◧  Meshes"': 'format!("{} Meshes", egui_phosphor::regular::POLYGON)',
    '"⬆ Export CSV"': 'format!("{} Export CSV", egui_phosphor::regular::EXPORT)',
    '"⬇ Import CSV"': 'format!("{} Import CSV", egui_phosphor::regular::DOWNLOAD_SIMPLE)',
    '"＋ Add Node"': 'format!("{} Add Node", egui_phosphor::regular::PLUS)',
    '"＋ Add Edge"': 'format!("{} Add Edge", egui_phosphor::regular::PLUS)',
    '"＋ Add Glyph"': 'format!("{} Add Glyph", egui_phosphor::regular::PLUS)',
    '"＋ Add Mesh"': 'format!("{} Add Mesh", egui_phosphor::regular::PLUS)',
    '"＋  Add Operation"': 'format!("{}  Add Operation", egui_phosphor::regular::PLUS)',
    '"➕ Save"': 'format!("{} Save", egui_phosphor::regular::FLOPPY_DISK)',
    '"🗑"': 'egui_phosphor::regular::TRASH',
    '"⚠ {value} (missing)"': 'format!("{} {value} (missing)", egui_phosphor::regular::WARNING)',
    '"▶  Apply"': 'format!("{}  Apply", egui_phosphor::regular::CHECK)',
    'message.starts_with(\'✔\')': 'message.starts_with(egui_phosphor::regular::CHECK)',
    '"▶  Export"': 'format!("{}  Export", egui_phosphor::regular::EXPORT)',
    '"✔ Exported {} frames to {}"': 'format!("{} Exported {} frames to {}", egui_phosphor::regular::CHECK, total, dir)',
    '"✔ Exported MP4 to {}"': 'format!("{} Exported MP4 to {}", egui_phosphor::regular::CHECK, output.display())',
    '"✔  Apply"': 'format!("{}  Apply", egui_phosphor::regular::CHECK)',
    '"⟲  Reset"': 'format!("{}  Reset", egui_phosphor::regular::ARROWS_CLOCKWISE)',
    '"⚠ Select nodes in the table first."': 'format!("{} Select nodes in the table first.", egui_phosphor::regular::WARNING)',
    '"✔  Create"': 'format!("{}  Create", egui_phosphor::regular::CHECK)',
    '"⚠ Node A and Node B must be different."': 'format!("{} Node A and Node B must be different.", egui_phosphor::regular::WARNING)',
    '"⏮"': 'egui_phosphor::regular::SKIP_BACK',
    '"⏸"': 'egui_phosphor::regular::PAUSE',
    '"⏹"': 'egui_phosphor::regular::STOP',
    '"⏭"': 'egui_phosphor::regular::SKIP_FORWARD',
    '"⬡  Node"': 'format!("{} Node", egui_phosphor::regular::HEXAGON)',
    '"╱  Edge"': 'format!("{} Edge", egui_phosphor::regular::LINE_SEGMENT)',
    '"◆  Glyph"': 'format!("{} Glyph", egui_phosphor::regular::DIAMOND)',
}

for f in files:
    with open(f, "r", encoding="utf-8") as fh:
        content = fh.read()
    
    for k, v in replacements.items():
        content = content.replace(k, v)

    # Special cases
    if f == 'src/csys_builder.rs':
        content = content.replace('ui.small_button("Edit")', 'ui.small_button(egui_phosphor::regular::PENCIL)')
    if f == 'src/table.rs':
        content = content.replace('"▶"', 'egui_phosphor::regular::CARET_RIGHT')
        
    else:
        content = content.replace('"▶"', 'egui_phosphor::regular::PLAY')

    with open(f, "w", encoding="utf-8") as fh:
        fh.write(content)
