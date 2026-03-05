//! Periodic room snapshot persistence.
//!
//! Serializes each active room's canonical document to a JSON file on disk.
//! On server startup, snapshots are loaded back into the room manager so
//! in-progress collaborative sessions survive restarts.
//!
//! File layout: `{data_dir}/{room_id}.json`

use std::path::{Path, PathBuf};
use std::time::Duration;

use selean_collab::types::RoomId;
use selean_engine::persistence::{Document, load_document, save_document};

use super::ws_handler::CollabState;

/// Errors from snapshot operations.
#[derive(Debug, thiserror::Error)]
pub enum SnapshotError {
    /// I/O error reading or writing snapshot files.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Persistence (de)serialization error.
    #[error("persistence error: {0}")]
    Persistence(String),
}

/// Saves a single room's document to `{dir}/{room_id}.json`.
///
/// Creates the directory if it does not exist.
///
/// # Errors
///
/// Returns `SnapshotError::Io` on filesystem errors or
/// `SnapshotError::Persistence` if the document cannot be serialized.
pub fn save_room_snapshot(
    dir: &Path,
    room_id: RoomId,
    document: &Document,
) -> Result<(), SnapshotError> {
    std::fs::create_dir_all(dir)?;
    let json = save_document(document).map_err(|e| SnapshotError::Persistence(e.to_string()))?;
    let path = dir.join(format!("{room_id}.json"));
    let tmp_path = dir.join(format!("{room_id}.json.tmp"));
    std::fs::write(&tmp_path, json)?;
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Loads all room snapshots from `{dir}/*.json`.
///
/// Returns a vec of `(RoomId, Document)` pairs. Files with invalid names or
/// corrupt data are logged and skipped.
///
/// # Errors
///
/// Returns `SnapshotError::Io` if the directory cannot be read.
pub fn load_room_snapshots(dir: &Path) -> Result<Vec<(RoomId, Document)>, SnapshotError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        let Some(ext) = path.extension() else {
            continue;
        };
        if ext != "json" {
            continue;
        }

        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };

        let Ok(room_id) =
            serde_json::from_value::<RoomId>(serde_json::Value::String(stem.to_string()))
        else {
            tracing::warn!("skipping snapshot with invalid room ID: {}", path.display());
            continue;
        };

        match std::fs::read_to_string(&path) {
            Ok(json) => match load_document(&json) {
                Ok(doc) => {
                    results.push((room_id, doc));
                }
                Err(e) => {
                    tracing::warn!("skipping corrupt snapshot {}: {e}", path.display());
                }
            },
            Err(e) => {
                tracing::warn!("failed to read snapshot {}: {e}", path.display());
            }
        }
    }

    Ok(results)
}

/// Snapshots all active rooms to disk.
///
/// Acquires a read lock on the room manager to collect room arcs, then
/// locks each room individually to serialize its document.
///
/// Returns the number of rooms successfully saved.
///
/// # Errors
///
/// Returns `SnapshotError::Io` on filesystem errors.
///
/// # Panics
///
/// Panics if the room manager or a room mutex is poisoned.
#[allow(clippy::expect_used)]
pub fn snapshot_all_rooms(dir: &Path, collab: &CollabState) -> Result<usize, SnapshotError> {
    let rooms: Vec<_> = {
        let mgr = collab.room_manager.read().expect("room manager poisoned");
        mgr.rooms().collect()
    };

    let mut saved = 0;
    for (room_id, room_arc) in &rooms {
        let doc = {
            let room = room_arc.lock().expect("room poisoned");
            room.document().clone()
        };
        match save_room_snapshot(dir, *room_id, &doc) {
            Ok(()) => saved += 1,
            Err(e) => {
                tracing::error!("failed to snapshot room {room_id}: {e}");
            }
        }
    }

    Ok(saved)
}

/// Loads snapshots from disk and inserts them into the room manager.
///
/// Returns the number of rooms loaded.
///
/// # Errors
///
/// Returns `SnapshotError::Io` if the snapshot directory cannot be read.
///
/// # Panics
///
/// Panics if the room manager mutex is poisoned.
#[allow(clippy::expect_used)]
pub fn load_snapshots_into(dir: &Path, collab: &CollabState) -> Result<usize, SnapshotError> {
    let snapshots = load_room_snapshots(dir)?;
    let count = snapshots.len();

    if count > 0 {
        let mut mgr = collab.room_manager.write().expect("room manager poisoned");
        for (room_id, document) in snapshots {
            mgr.get_or_create_room_with_document(room_id, document);
        }
    }

    Ok(count)
}

/// Spawns a background task that periodically snapshots all rooms.
///
/// The task runs every `interval` and saves each room's document to `dir`.
/// It logs errors but never panics.
pub fn start_snapshot_task(
    interval: Duration,
    dir: PathBuf,
    collab: CollabState,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        // The first tick fires immediately; skip it so we don't snapshot
        // right at startup before any work has been done.
        ticker.tick().await;

        loop {
            ticker.tick().await;
            match snapshot_all_rooms(&dir, &collab) {
                Ok(0) => {} // No rooms, nothing to log.
                Ok(n) => tracing::debug!("snapshotted {n} room(s)"),
                Err(e) => tracing::error!("snapshot task error: {e}"),
            }
        }
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use selean_common::types::NodeId;
    use selean_engine::persistence::Document;
    use selean_engine::scene::{BoundingBox, SceneNode, SceneNodeKind};

    fn make_document_with_node(name: &str) -> Document {
        let mut doc = Document::new();
        let node = SceneNode::new(
            NodeId::new(),
            name.to_string(),
            SceneNodeKind::Frame {
                corner_radius: [0.0; 4],
            },
            BoundingBox::new(0.0, 0.0, 100.0, 50.0),
        );
        doc.active_page_mut().scene.add_root(node);
        doc
    }

    // --- save_room_snapshot ---

    #[test]
    fn save_creates_directory_and_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("rooms");
        let room_id = RoomId::new();
        let doc = make_document_with_node("Test");

        save_room_snapshot(&dir, room_id, &doc).unwrap();

        let path = dir.join(format!("{room_id}.json"));
        assert!(path.exists());

        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("\"version\""));
    }

    #[test]
    fn save_overwrites_existing_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let room_id = RoomId::new();

        let doc1 = make_document_with_node("First");
        save_room_snapshot(tmp.path(), room_id, &doc1).unwrap();

        let doc2 = make_document_with_node("Second");
        save_room_snapshot(tmp.path(), room_id, &doc2).unwrap();

        let contents = std::fs::read_to_string(tmp.path().join(format!("{room_id}.json"))).unwrap();
        assert!(contents.contains("Second"));
    }

    // --- load_room_snapshots ---

    #[test]
    fn load_returns_empty_for_missing_dir() {
        let result = load_room_snapshots(Path::new("/nonexistent/path/rooms")).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_returns_empty_for_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_room_snapshots(tmp.path()).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn load_roundtrips_saved_snapshot() {
        let tmp = tempfile::tempdir().unwrap();
        let room_id = RoomId::new();
        let doc = make_document_with_node("Roundtrip");

        save_room_snapshot(tmp.path(), room_id, &doc).unwrap();
        let loaded = load_room_snapshots(tmp.path()).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].0, room_id);
        assert_eq!(loaded[0].1.active_page().scene.len(), 1);
    }

    #[test]
    fn load_multiple_rooms() {
        let tmp = tempfile::tempdir().unwrap();
        let id1 = RoomId::new();
        let id2 = RoomId::new();

        save_room_snapshot(tmp.path(), id1, &make_document_with_node("A")).unwrap();
        save_room_snapshot(tmp.path(), id2, &make_document_with_node("B")).unwrap();

        let loaded = load_room_snapshots(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 2);

        let ids: Vec<_> = loaded.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[test]
    fn load_skips_non_json_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("readme.txt"), "hello").unwrap();
        std::fs::write(tmp.path().join("data.bin"), [0u8; 10]).unwrap();

        let room_id = RoomId::new();
        save_room_snapshot(tmp.path(), room_id, &make_document_with_node("Valid")).unwrap();

        let loaded = load_room_snapshots(tmp.path()).unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn load_skips_invalid_room_id_filenames() {
        let tmp = tempfile::tempdir().unwrap();

        // Write a valid document with a non-UUID filename.
        let doc = make_document_with_node("BadName");
        let json = save_document(&doc).unwrap();
        std::fs::write(tmp.path().join("not-a-uuid.json"), &json).unwrap();

        let loaded = load_room_snapshots(tmp.path()).unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn load_skips_corrupt_json() {
        let tmp = tempfile::tempdir().unwrap();
        let room_id = RoomId::new();
        std::fs::write(tmp.path().join(format!("{room_id}.json")), "not valid json").unwrap();

        let loaded = load_room_snapshots(tmp.path()).unwrap();
        assert!(loaded.is_empty());
    }

    // --- snapshot_all_rooms ---

    #[test]
    fn snapshot_all_rooms_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let collab = CollabState::new();

        let saved = snapshot_all_rooms(tmp.path(), &collab).unwrap();
        assert_eq!(saved, 0);
    }

    #[test]
    fn snapshot_all_rooms_saves_active_rooms() {
        let tmp = tempfile::tempdir().unwrap();
        let collab = CollabState::new();

        let room_id = RoomId::new();
        {
            let mut mgr = collab.room_manager.write().unwrap();
            mgr.get_or_create_room_with_document(room_id, make_document_with_node("Active"));
        }

        let saved = snapshot_all_rooms(tmp.path(), &collab).unwrap();
        assert_eq!(saved, 1);

        let path = tmp.path().join(format!("{room_id}.json"));
        assert!(path.exists());
    }

    // --- load_snapshots_into ---

    #[test]
    fn load_snapshots_into_populates_rooms() {
        let tmp = tempfile::tempdir().unwrap();
        let room_id = RoomId::new();

        save_room_snapshot(tmp.path(), room_id, &make_document_with_node("Persisted")).unwrap();

        let collab = CollabState::new();
        let loaded = load_snapshots_into(tmp.path(), &collab).unwrap();

        assert_eq!(loaded, 1);

        let mgr = collab.room_manager.read().unwrap();
        assert_eq!(mgr.room_count(), 1);
        let room_arc = mgr.get_room(&room_id).unwrap();
        let room = room_arc.lock().unwrap();
        assert_eq!(room.document().active_page().scene.len(), 1);
    }

    #[test]
    fn load_snapshots_into_noop_for_empty_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let collab = CollabState::new();

        let loaded = load_snapshots_into(tmp.path(), &collab).unwrap();
        assert_eq!(loaded, 0);

        let mgr = collab.room_manager.read().unwrap();
        assert_eq!(mgr.room_count(), 0);
    }

    // --- Full round-trip: save all -> load all ---

    #[test]
    fn full_roundtrip_snapshot_and_restore() {
        let tmp = tempfile::tempdir().unwrap();
        let id1 = RoomId::new();
        let id2 = RoomId::new();

        // Create rooms with documents.
        let collab1 = CollabState::new();
        {
            let mut mgr = collab1.room_manager.write().unwrap();
            mgr.get_or_create_room_with_document(id1, make_document_with_node("Room1"));
            mgr.get_or_create_room_with_document(id2, make_document_with_node("Room2"));
        }

        // Snapshot to disk.
        let saved = snapshot_all_rooms(tmp.path(), &collab1).unwrap();
        assert_eq!(saved, 2);

        // Load into a fresh CollabState (simulating server restart).
        let collab2 = CollabState::new();
        let loaded = load_snapshots_into(tmp.path(), &collab2).unwrap();
        assert_eq!(loaded, 2);

        let mgr = collab2.room_manager.read().unwrap();
        assert_eq!(mgr.room_count(), 2);

        // Verify both rooms have their documents.
        let r1 = mgr.get_room(&id1).unwrap();
        assert_eq!(r1.lock().unwrap().document().active_page().scene.len(), 1);

        let r2 = mgr.get_room(&id2).unwrap();
        assert_eq!(r2.lock().unwrap().document().active_page().scene.len(), 1);
    }
}
