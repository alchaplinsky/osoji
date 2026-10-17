//! Linux: the jwalk reference walker (a native getdents64 walker is spec milestone M7), xdg-open
//! hand-offs and volumes read from /proc/self/mounts. Nothing like Full Disk Access gates file
//! access here, so the probe always reports granted.

use super::VolumeInfo;
use scan_core::{Arena, Cancel, Progress, ScanConfig};
use std::ffi::CString;
use std::io;
use std::path::{Path, PathBuf};

pub fn prepare_process() {
    scan_core::raise_fd_limit();
}

pub fn walk(cfg: &ScanConfig, progress: &Progress, cancel: &Cancel) -> io::Result<Arena> {
    scan_core::jwalk_walker::walk(cfg, progress, cancel)
}

/// Nothing to hook into the window on Linux.
pub fn on_setup(_app: &tauri::App) {}

/// Opens the containing folder; `xdg-open` has no way to select an item.
pub fn reveal(path: &Path) -> io::Result<()> {
    let folder = if path.is_dir() { path } else { path.parent().unwrap_or(path) };
    std::process::Command::new("xdg-open").arg(folder).spawn().map(|_| ())
}

/// `trash:///` is understood by the GIO and KIO file managers; the `trash` crate puts items there.
pub fn show_trash(_home: &Path) -> io::Result<()> {
    std::process::Command::new("xdg-open").arg("trash:///").spawn().map(|_| ())
}

pub fn full_disk_access(_home: &Path) -> bool {
    true
}

pub fn open_privacy_settings() -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "no privacy setting gates file access on Linux"))
}

pub fn volume_info(path: &str) -> Result<VolumeInfo, String> {
    let target = std::fs::canonicalize(path).map_err(|e| e.to_string())?;
    let mount = mounts()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| target.starts_with(m))
        .max_by_key(|m| m.as_os_str().len())
        .unwrap_or_else(|| PathBuf::from("/"));
    volume_at(&mount).map_err(|e| e.to_string())
}

/// Block-device mounts, root first; see `mounts`.
pub fn volumes() -> Result<Vec<VolumeInfo>, String> {
    let mut out: Vec<VolumeInfo> =
        mounts().map_err(|e| e.to_string())?.iter().filter_map(|m| volume_at(m).ok()).collect();
    out.sort_by(|a, b| (a.mount != "/").cmp(&(b.mount != "/")).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Mount points backed by a block device, from /proc/self/mounts. Snap images (squashfs on a
/// loop device) and the boot partitions are left out. `/` is always present, even where it is
/// an overlay or tmpfs, because it is the app's default scan root.
fn mounts() -> io::Result<Vec<PathBuf>> {
    let text = std::fs::read_to_string("/proc/self/mounts")?;
    let mut out: Vec<PathBuf> = Vec::new();
    for line in text.lines() {
        let mut fields = line.split(' ');
        let (Some(source), Some(mount), Some(fstype)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        if !source.starts_with("/dev/") || fstype == "squashfs" || mount.starts_with("/boot") {
            continue;
        }
        let mount = PathBuf::from(unescape(mount));
        // A later line over-mounts an earlier one at the same point.
        out.retain(|m| *m != mount);
        out.push(mount);
    }
    if !out.iter().any(|m| m == Path::new("/")) {
        out.insert(0, PathBuf::from("/"));
    }
    Ok(out)
}

/// /proc/mounts writes space, tab, newline and backslash as octal `\ooo`.
fn unescape(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 3 < b.len() {
            if let Some(v) = octal(&b[i + 1..i + 4]) {
                out.push(v);
                i += 4;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn octal(digits: &[u8]) -> Option<u8> {
    let v =
        digits.iter().try_fold(0u32, |acc, &c| (b'0'..=b'7').contains(&c).then(|| acc * 8 + u32::from(c - b'0')))?;
    u8::try_from(v).ok()
}

// statvfs fields are already u64 on 64-bit Linux; the casts keep 32-bit targets building.
#[allow(clippy::unnecessary_cast)]
fn volume_at(mount: &Path) -> io::Result<VolumeInfo> {
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(mount.as_os_str().as_bytes()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let frsize = st.f_frsize as u64;
    Ok(VolumeInfo {
        name: volume_name(mount),
        mount: mount.to_string_lossy().into_owned(),
        total: st.f_blocks as u64 * frsize,
        free: st.f_bavail as u64 * frsize,
    })
}

/// Removable media mount under their label (`/media/<user>/<label>`, `/run/media/...`); the root has no name of its own.
fn volume_name(mount: &Path) -> String {
    match mount.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => "Root".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unescapes_proc_mounts_octal() {
        assert_eq!(unescape(r"/media/me/My\040Disk"), "/media/me/My Disk");
        assert_eq!(unescape(r"a\134b"), r"a\b");
        assert_eq!(unescape(r"trailing\04"), r"trailing\04");
        assert_eq!(unescape(r"not\999octal"), r"not\999octal");
    }

    #[test]
    fn lists_the_root_volume_first() {
        let list = volumes().unwrap();
        let root = &list[0];
        assert_eq!(root.mount, "/");
        assert_eq!(root.name, "Root");
        assert!(root.total > 0);
    }

    #[test]
    fn resolves_a_path_to_its_volume() {
        let info = volume_info(env!("CARGO_MANIFEST_DIR")).unwrap();
        assert!(info.total >= info.free);
        assert!(Path::new(env!("CARGO_MANIFEST_DIR")).starts_with(&info.mount));
    }
}
