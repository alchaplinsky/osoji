//! Linux: the jwalk walker and the portable listing from scan-core, no file watcher yet, the
//! root filesystem as the only volume, and xdg-open for the shell. Enough to build and scan;
//! the rest comes with the Linux work that follows.

use super::VolumeInfo;
use scan_core::live::Batch;
use std::ffi::CString;
use std::io;
use std::path::{Path, PathBuf};

pub use scan_core::jwalk_walker::walk;
pub use scan_core::live::list_dir;

pub const PROTECTED_BY_OS: &str = "Owned by the system and cannot be moved by Osoji.";

/// Once per process, before the first scan.
pub fn init() {
    scan_core::raise_fd_limit();
}

/// Stands in for a watcher until Linux has one: `start` always fails, so every scan reports
/// `live: false` and the UI shows a view that does not follow changes.
#[allow(dead_code)]
pub struct Watcher {
    root: PathBuf,
}

impl Watcher {
    pub fn start(_root: &Path, _on_batch: impl Fn(Batch) + Send + 'static) -> io::Result<Watcher> {
        Err(io::Error::new(io::ErrorKind::Unsupported, "file watching is not implemented on Linux yet"))
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// `statvfs` counts are `c_ulong`/`fsblkcnt_t`, 64-bit on the targets Osoji builds for; the cast
/// keeps a 32-bit build honest.
#[allow(clippy::unnecessary_cast)]
fn bytes(blocks: libc::fsblkcnt_t, frsize: libc::c_ulong) -> u64 {
    blocks as u64 * frsize as u64
}

/// The filesystem holding `path`, reported against `path` itself: there is no mount table
/// lookup yet, so `mount` is the path given.
pub fn volume_info(path: &str) -> Result<VolumeInfo, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(VolumeInfo {
        name: "System".into(),
        mount: path.to_string(),
        total: bytes(st.f_blocks, st.f_frsize),
        free: bytes(st.f_bavail, st.f_frsize),
    })
}

/// The root filesystem alone; other mounts come with the Linux volume work.
pub fn volumes() -> Result<Vec<VolumeInfo>, String> {
    Ok(vec![volume_info("/")?])
}

/// Open the folder holding `path` in the user's file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    let dir = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open the user's Trash folder in the file manager: `$XDG_DATA_HOME/Trash`, the same place the
/// policy's trash rule points at.
pub fn show_trash(home: &Path) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(scan_policy::xdg_data_home(home).join("Trash/files"))
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// No TCC on Linux: whatever the user can read, the app can read.
pub fn full_disk_access(_home: &Path) -> bool {
    true
}

pub fn open_privacy_settings() -> Result<(), String> {
    Ok(())
}
