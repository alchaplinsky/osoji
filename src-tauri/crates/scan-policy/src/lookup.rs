use scan_core::{Arena, ChildIndex, Flags, NodeId, NONE};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The token a rule path may start with to mean the user's XDG data directory.
const XDG_DATA_HOME: &str = "$XDG_DATA_HOME";

/// Expand a leading `~` to `home` or a leading `$XDG_DATA_HOME` to `data_home`. Returns None
/// when the path needs a base that is unknown (not absolute).
pub fn expand(path: &str, home: &Path, data_home: &Path) -> Option<PathBuf> {
    let (base, rest) = if let Some(rest) = path.strip_prefix(XDG_DATA_HOME) {
        (data_home, rest)
    } else if let Some(rest) = path.strip_prefix('~') {
        (home, rest)
    } else {
        return Some(PathBuf::from(path));
    };
    if !base.is_absolute() {
        return None;
    }
    let rest = rest.trim_start_matches('/');
    Some(if rest.is_empty() { base.to_path_buf() } else { base.join(rest) })
}

/// The user's XDG data directory: `$XDG_DATA_HOME` when set to an absolute path, else the
/// `~/.local/share` default. The Trash lives under it.
pub fn xdg_data_home(home: &Path) -> PathBuf {
    data_home_from(home, std::env::var_os("XDG_DATA_HOME").as_deref())
}

/// `xdg_data_home` with the variable's value passed in, so it is testable without touching the
/// environment. The spec says a relative value must be ignored.
pub(crate) fn data_home_from(home: &Path, var: Option<&OsStr>) -> PathBuf {
    match var {
        Some(v) if !v.is_empty() && Path::new(v).is_absolute() => PathBuf::from(v),
        _ => home.join(".local/share"),
    }
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
