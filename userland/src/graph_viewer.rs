use crate::{canon, fs, graph, Value};
use alloc::string::ToString;

pub fn sync_graph_viewer_from_bin() -> Result<(), fs::FsError> {
    // 1. Ensure a graph_viewer_surface widget exists.
    // We'll use a deterministic ID for the surface so we can find it easily.
    let graph_viewer_id = crate::simple_uuid(b"graph_viewer_surface");

    let mut fields = graph::map();
    fields.insert(canon::ROLE, Value::Text("graph_viewer_surface".to_string()));
    fields.insert(canon::LABEL, Value::Text("Graph Viewer".to_string()));
    fields.insert(canon::VISIBLE, Value::Bool(true));

    // Create the graph viewer surface widget
    graph::fiat(Some(graph_viewer_id), canon::WIDGET, fields);

    // 2. Read /bin.
    let bin_entries = fs::read_dir("/bin")?;

    // 3. For each FsNode with SHOW_IN_GRAPH_VIEWER=true:
    let mut count = 0;
    for entry in bin_entries {
        if !entry.show_in_graph_viewer {
            continue;
        }
        count += 1;

        // Create or update a graph_viewer_entry widget under graph_viewer_surface.
        // We'll use a deterministic ID based on the app name to avoid duplicates.
        let entry_id =
            crate::simple_uuid(alloc::format!("graph_viewer_entry_{}", entry.name).as_bytes());

        let mut entry_fields = graph::map();
        entry_fields.insert(canon::ROLE, Value::Text("graph_viewer_entry".to_string()));
        entry_fields.insert(canon::WIDGET_KIND, Value::Text("thing_tile".to_string()));
        entry_fields.insert(canon::LABEL, Value::Text(entry.name.clone()));
        entry_fields.insert(canon::VISIBLE, Value::Bool(true));
        entry_fields.insert(canon::FOCUSABLE, Value::Bool(true));

        graph::fiat(Some(entry_id), canon::WIDGET, entry_fields);

        // Link it to the graph viewer surface with HAS_ENTRY.
        graph::that(graph_viewer_id, "HAS_ENTRY", entry_id, 0);

        // Link to the FsNode with LAUNCHES.
        graph::that(entry_id, "LAUNCHES", entry.id, 0);
    }

    crate::println!("Graph Viewer: synced {} entries from /bin", count);

    Ok(())
}
