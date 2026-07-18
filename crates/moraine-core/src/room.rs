use std::path::Path;

/// Room id for a path. Uses absolute path when possible.
/// Hash matches the frontend `roomIdForPath` (Java-style over UTF-16 code units).
pub fn room_id_for_path(path: &Path) -> String {
    let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    room_id_for_str(&abs.to_string_lossy())
}

pub fn room_id_for_str(s: &str) -> String {
    let mut h: i32 = 0;
    for unit in s.encode_utf16() {
        h = h.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    format!("doc_{:x}", h as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_ascii() {
        assert_eq!(room_id_for_str("/tmp/note.md"), room_id_for_str("/tmp/note.md"));
        assert_ne!(room_id_for_str("/tmp/a.md"), room_id_for_str("/tmp/b.md"));
        assert!(room_id_for_str("/tmp/note.md").starts_with("doc_"));
    }
}
