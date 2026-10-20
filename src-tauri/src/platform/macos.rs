//! macOS: the getattrlistbulk walker and FSEvents watcher from scan-macos, getmntinfo volumes,
//! Finder for reveal and the Trash, and Full Disk Access through TCC.

use super::VolumeInfo;
use std::ffi::{CStr, CString};
use std::path::Path;

pub use scan_macos::{list_dir, walk, Watcher};

pub const PROTECTED_BY_OS: &str = "Protected by macOS and cannot be moved by Osoji.";

/// Once per process, before the first scan.
pub fn init() {
    scan_macos::disable_dataless_materialization();
    scan_macos::raise_fd_limit();
}

pub fn volume_info(path: &str) -> Result<VolumeInfo, String> {
    let c = CString::new(path).map_err(|e| e.to_string())?;
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
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
        return Err(std::io::Error::last_os_error().to_string());
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

/// Show `path` selected in Finder.
pub fn reveal(path: &Path) -> Result<(), String> {
    std::process::Command::new("open").arg("-R").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open the user's Trash folder in Finder.
pub fn show_trash(home: &Path) -> Result<(), String> {
    std::process::Command::new("open").arg(home.join(".Trash")).spawn().map(|_| ()).map_err(|e| e.to_string())
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
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => denied = true,
            Err(_) => {}
        }
    }
    !denied
}

pub fn open_privacy_settings() -> Result<(), String> {
    std::process::Command::new("open").arg(PRIVACY_ALL_FILES_URL).spawn().map(|_| ()).map_err(|e| e.to_string())
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
