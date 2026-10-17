//! macOS: the getattrlistbulk walker, Finder hand-offs, getmntinfo volumes and the Full Disk
//! Access probe.

use super::VolumeInfo;
use scan_core::{Arena, Cancel, Progress, ScanConfig};
use std::ffi::{CStr, CString};
use std::io;
use std::path::Path;

pub fn prepare_process() {
    scan_macos::disable_dataless_materialization();
    scan_core::raise_fd_limit();
}

pub fn walk(cfg: &ScanConfig, progress: &Progress, cancel: &Cancel) -> io::Result<Arena> {
    scan_macos::walk(cfg, progress, cancel)
}

/// Double-click on a drag region zooms the window; route that zoom through the animator so the
/// page follows the frame instead of freezing (zoom.rs).
pub fn on_setup(app: &tauri::App) {
    use tauri::Manager;
    match app.get_webview_window("main").map(|w| w.ns_window()) {
        Some(Ok(ns_window)) => crate::zoom::install(ns_window),
        Some(Err(e)) => eprintln!("zoom: no NSWindow for main ({e}); zoom stays native"),
        None => eprintln!("zoom: no main window at setup; zoom stays native"),
    }
}

/// Finder, with the item selected.
pub fn reveal(path: &Path) -> io::Result<()> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ())
}

pub fn show_trash(home: &Path) -> io::Result<()> {
    std::process::Command::new("open").arg(home.join(".Trash")).spawn().map(|_| ())
}

pub fn volume_info(path: &str) -> Result<VolumeInfo, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    Ok(volume_from(&st))
}

/// User-visible local volumes: the boot disk at `/` plus whatever is mounted under
/// `/Volumes`. That prefix already leaves out `/dev` and the read-only system volumes
/// under `/System/Volumes`, including the boot disk's Data volume, which `/` stands for.
pub fn volumes() -> Result<Vec<VolumeInfo>, String> {
    let mut buf: *mut libc::statfs = std::ptr::null_mut();
    let n = unsafe { libc::getmntinfo(&mut buf, libc::MNT_NOWAIT) };
    if n <= 0 {
        return Err(io::Error::last_os_error().to_string());
    }
    // getmntinfo owns the buffer and reuses it on the next call; copy out before returning.
    let mounts = unsafe { std::slice::from_raw_parts(buf, n as usize) };
    let mut out: Vec<VolumeInfo> = mounts
        .iter()
        .filter(|st| {
            let flags = st.f_flags as libc::c_int;
            let mount = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_bytes();
            flags & libc::MNT_LOCAL != 0
                && flags & libc::MNT_DONTBROWSE == 0
                && (mount == b"/" || mount.starts_with(b"/Volumes/"))
        })
        .map(volume_from)
        .collect();
    out.sort_by(|a, b| (a.mount != "/").cmp(&(b.mount != "/")).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

fn volume_from(st: &libc::statfs) -> VolumeInfo {
    let mount = unsafe { CStr::from_ptr(st.f_mntonname.as_ptr()) }.to_string_lossy().into_owned();
    let bsize = u64::from(st.f_bsize);
    VolumeInfo { name: volume_name(&mount), total: st.f_blocks * bsize, free: st.f_bavail * bsize, mount }
}

/// `/Volumes` holds the user-visible names; the boot volume is a symlink to `/`.
/// The boot disk's Data volume (home folders live there) shares the boot disk's name.
fn volume_name(mount: &str) -> String {
    let target = Path::new(if mount == "/System/Volumes/Data" { "/" } else { mount });
    let listed = std::fs::read_dir("/Volumes").ok().and_then(|entries| {
        entries
            .flatten()
            .find(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| p == target))
            .map(|e| e.file_name().to_string_lossy().into_owned())
    });
    listed.unwrap_or_else(|| match target.file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => "Startup Disk".into(),
    })
}

/// Folders under `$HOME` that macOS only lets an app list once it has Full Disk Access.
const TCC_PROBES: [&str; 3] = ["Library/Mail", "Library/Safari", "Library/Messages"];
const PRIVACY_ALL_FILES_URL: &str = "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles";

/// Granted when any protected folder that exists can be listed; with none present there is nothing to block.
pub fn full_disk_access(home: &Path) -> bool {
    let mut denied = false;
    for dir in TCC_PROBES.map(|p| home.join(p)) {
        match std::fs::read_dir(&dir) {
            Ok(_) => return true,
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => denied = true,
            Err(_) => {}
        }
    }
    !denied
}

pub fn open_privacy_settings() -> io::Result<()> {
    std::process::Command::new("open").arg(PRIVACY_ALL_FILES_URL).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_boot_volume_first() {
        let list = volumes().unwrap();
        let boot = &list[0];
        assert_eq!(boot.mount, "/");
        assert!(!boot.name.is_empty());
        assert!(boot.total > 0);
        assert!(list.iter().all(|v| v.mount == "/" || v.mount.starts_with("/Volumes/")));
    }
}
