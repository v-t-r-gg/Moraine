//! Integration: room ids + share links stay consistent.

use std::fs;

use moraine_core::{
    room_id_for_path, room_id_for_str, share_links, DEFAULT_RELAY_HTTP, DEFAULT_UI,
};
use tempfile::tempdir;

#[test]
fn room_id_matches_fixture() {
    // Must stay in lockstep with frontend roomIdForPath("/tmp/note.md").
    assert_eq!(room_id_for_str("/tmp/note.md"), "doc_53b4008c");
}

#[test]
fn share_links_roundtrip_fields() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("doc.md");
    fs::write(&path, "# hi\n").unwrap();

    let links = share_links(&path, DEFAULT_UI, DEFAULT_RELAY_HTTP);
    assert_eq!(links.room, room_id_for_path(&path));
    assert!(links.url.starts_with(DEFAULT_UI));
    assert!(links.url.contains(&links.room));
    assert!(links.ws.contains("/ws/"));
    assert_eq!(links.server, DEFAULT_RELAY_HTTP);
}

#[test]
fn different_paths_different_rooms() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    fs::write(&a, "a").unwrap();
    fs::write(&b, "b").unwrap();
    assert_ne!(room_id_for_path(&a), room_id_for_path(&b));
}
