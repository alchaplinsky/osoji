use scan_core::{Arena, ChildIndex, Flags, NodeId, NONE};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// Expand a leading `~` to `home`. Returns None when the path needs a home that is unknown.
pub fn expand(path: &str, home: &Path) -> Option<PathBuf> {
    let rest = match path.strip_prefix('~') {
        None => return Some(PathBuf::from(path)),
        Some(rest) => rest.trim_start_matches('/'),
    };
    if !home.is_absolute() {
        return None;
    }
    Some(if rest.is_empty() { home.to_path_buf() } else { home.join(rest) })
}

/// Resolve an absolute path to a node of the scan, or NONE when it is outside the scan or missing.
/// The root node's name is the scan root's full path.
pub fn find_node(arena: &Arena, index: &ChildIndex, path: &Path) -> NodeId {
    if arena.is_empty() {
        return NONE;
    }
    let root = Path::new(std::ffi::OsStr::from_bytes(arena.name(0)));
    let Ok(rest) = path.strip_prefix(root) else {
        return NONE;
    };
    let mut id: NodeId = 0;
    for part in rest.components() {
        let name = part.as_os_str().as_bytes();
        // Skip trashed and removed children: a name that came back on disk is a different, live node.
        let live = |c: NodeId| !arena.flags[c as usize].intersects(Flags::TRASHED | Flags::REMOVED);
        match index.children(id).iter().find(|&&c| live(c) && arena.name(c) == name) {
            Some(&c) => id = c,
            None => return NONE,
        }
    }
    id
}
