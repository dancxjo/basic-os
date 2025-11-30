use crate::{canon, fs, graph, semantic_ui, Value};
use alloc::string::ToString;
use alloc::vec;
use uuid::Uuid;

pub fn sync_launcher_from_bin() -> Result<(), fs::FsError> {
    // 1. Ensure a launcher_surface widget exists.
    // We'll use a deterministic ID for the launcher surface so we can find it easily.
    let launcher_id = crate::simple_uuid(b"launcher_surface");

    let mut fields = graph::map();
    fields.insert(canon::ROLE, Value::Text("launcher_surface".to_string()));
    fields.insert(canon::LABEL, Value::Text("Launcher".to_string()));
    fields.insert(canon::VISIBLE, Value::Bool(true));

    // Create the launcher surface widget
    graph::fiat(Some(launcher_id), canon::WIDGET, fields);

    // 2. Read /bin.
    let bin_entries = fs::read_dir("/bin")?;

    // 3. For each FsNode with SHOW_IN_LAUNCHER=true:
    for entry in bin_entries {
        if !entry.show_in_launcher {
            continue;
        }

        // Create or update a launcher_entry widget under launcher_surface.
        // We'll use a deterministic ID based on the app name to avoid duplicates.
        let entry_id =
            crate::simple_uuid(alloc::format!("launcher_entry_{}", entry.name).as_bytes());

        let mut entry_fields = graph::map();
        entry_fields.insert(canon::ROLE, Value::Text("launcher_entry".to_string()));
        entry_fields.insert(canon::LABEL, Value::Text(entry.name.clone()));
        entry_fields.insert(canon::VISIBLE, Value::Bool(true));
        entry_fields.insert(canon::FOCUSABLE, Value::Bool(true));

        graph::fiat(Some(entry_id), canon::WIDGET, entry_fields);

        // Link it to the launcher surface with HAS_ENTRY.
        graph::that(launcher_id, "HAS_ENTRY", entry_id, 0);

        // Link to the FsNode with LAUNCHES.
        graph::that(entry_id, "LAUNCHES", entry.id, 0);
    }

    Ok(())
}
