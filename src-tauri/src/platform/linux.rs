//! Linux: the jwalk walker and the portable listing from scan-core, no file watcher yet, the
//! volumes from /proc/self/mounts, and xdg-open for the shell. Enough to build and scan;
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

/// A line of `/proc/self/mounts` that stands for a user-visible local volume.
#[derive(Debug, PartialEq)]
struct Mount {
    source: String,
    point: String,
}

/// Image mounts (snaps, optical media) are never a disk the user manages.
const HIDDEN_FSTYPES: [&str; 2] = ["squashfs", "iso9660"];
/// The firmware partitions and the kernel, service and container trees.
const HIDDEN_POINTS: [&str; 2] = ["/boot", "/boot/efi"];
const HIDDEN_TREES: [&str; 6] = ["/snap", "/var/lib/docker", "/proc", "/sys", "/dev", "/run"];

/// The volumes among the lines of `/proc/self/mounts` (`source point fstype options ...`):
/// block devices outside the hidden trees, plus `/` whatever backs it (overlay and zfs roots
/// have no `/dev/` source), so the list is never empty. A point mounted twice keeps the later
/// line, which is the filesystem visible there.
fn parse_mounts(text: &str) -> Vec<Mount> {
    let mut out: Vec<Mount> = Vec::new();
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let (Some(source), Some(point), Some(fstype)) = (fields.next(), fields.next(), fields.next()) else {
            continue;
        };
        let mount = Mount { source: unescape(source), point: unescape(point) };
        if !is_volume(&mount, fstype) {
            continue;
        }
        match out.iter_mut().find(|m| m.point == mount.point) {
            Some(existing) => *existing = mount,
            None => out.push(mount),
        }
    }
    out
}

fn is_volume(m: &Mount, fstype: &str) -> bool {
    m.point == "/"
        || (m.source.starts_with("/dev/")
            && !HIDDEN_FSTYPES.contains(&fstype)
            && !HIDDEN_POINTS.contains(&m.point.as_str())
            && !HIDDEN_TREES.iter().any(|tree| Path::new(&m.point).starts_with(tree)))
}

/// Decode the `\ooo` octal escapes of `/proc/self/mounts` (`\040` for a space) and the `\xNN`
/// hex escapes udev gives names under `/dev/disk/by-label`. Each format escapes the backslash
/// itself, so one decoder serves both without misreading a literal.
fn unescape(field: &str) -> String {
    let bytes = field.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = match bytes[i] {
            b'\\' if bytes.get(i + 1) == Some(&b'x') => parse_byte(&bytes[i + 2..], 16, 2),
            b'\\' => parse_byte(&bytes[i + 1..], 8, 3),
            _ => None,
        };
        out.push(escaped.unwrap_or(bytes[i]));
        i += if escaped.is_some() { 4 } else { 1 };
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_byte(bytes: &[u8], radix: u32, digits: usize) -> Option<u8> {
    let s = std::str::from_utf8(bytes.get(..digits)?).ok()?;
    if !s.chars().all(|c| c.is_digit(radix)) {
        return None;
    }
    u8::from_str_radix(s, radix).ok()
}

/// The mount holding `path`: the kept mount with the longest point that is a component-wise
/// prefix of it, `/` when none is (a relative path).
fn mount_for<'a>(mounts: &'a [Mount], path: &str) -> Option<&'a Mount> {
    mounts
        .iter()
        .filter(|m| Path::new(path).starts_with(&m.point))
        .max_by_key(|m| m.point.len())
        .or_else(|| mounts.iter().find(|m| m.point == "/"))
}

fn mounts() -> Result<Vec<Mount>, String> {
    std::fs::read_to_string("/proc/self/mounts").map(|text| parse_mounts(&text)).map_err(|e| e.to_string())
}

/// `statvfs` counts are `c_ulong`/`fsblkcnt_t`, 64-bit on the targets Osoji builds for; the cast
/// keeps a 32-bit build honest.
#[allow(clippy::unnecessary_cast)]
fn bytes(blocks: libc::fsblkcnt_t, frsize: libc::c_ulong) -> u64 {
    blocks as u64 * frsize as u64
}

fn volume(m: &Mount) -> Result<VolumeInfo, String> {
    let c = CString::new(m.point.as_str()).map_err(|e| e.to_string())?;
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    Ok(VolumeInfo {
        name: volume_name(m),
        mount: m.point.clone(),
        total: bytes(st.f_blocks, st.f_frsize),
        free: bytes(st.f_bavail, st.f_frsize),
    })
}

/// The filesystem label when udev published one, else the last component of the mount point.
/// `/` is "System", as the UI's boot volume.
fn volume_name(m: &Mount) -> String {
    if m.point == "/" {
        return "System".into();
    }
    label_for(&m.source).unwrap_or_else(|| match Path::new(&m.point).file_name() {
        Some(n) => n.to_string_lossy().into_owned(),
        None => m.point.clone(),
    })
}

/// The entry of `/dev/disk/by-label` whose symlink resolves to the device `source` does.
fn label_for(source: &str) -> Option<String> {
    let device = std::fs::canonicalize(source).ok()?;
    std::fs::read_dir("/dev/disk/by-label")
        .ok()?
        .flatten()
        .find(|e| std::fs::canonicalize(e.path()).is_ok_and(|p| p == device))
        .map(|e| unescape(&e.file_name().to_string_lossy()))
}

/// The volume holding `path`, reported against its mount point.
pub fn volume_info(path: &str) -> Result<VolumeInfo, String> {
    let mounts = mounts()?;
    let mount = mount_for(&mounts, path).ok_or_else(|| format!("no mount holds {path}"))?;
    volume(mount)
}

/// User-visible local volumes, `/` first and the rest by name. Mounts `statvfs` rejects
/// (stale network shares, points this user cannot enter) are left out rather than failing the list.
pub fn volumes() -> Result<Vec<VolumeInfo>, String> {
    let mut out: Vec<VolumeInfo> = mounts()?.iter().filter_map(|m| volume(m).ok()).collect();
    out.sort_by(|a, b| (a.mount != "/").cmp(&(b.mount != "/")).then_with(|| a.name.cmp(&b.name)));
    Ok(out)
}

/// Open the folder holding `path` in the user's file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    let dir = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open").arg(dir).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Open the user's Trash folder in the file manager.
pub fn show_trash(home: &Path) -> Result<(), String> {
    std::process::Command::new("xdg-open")
        .arg(home.join(".local/share/Trash/files"))
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

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTS: &str = "\
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
/dev/nvme0n1p1 /boot/efi vfat rw,relatime,fmask=0077 0 0
/dev/nvme0n1p3 /home ext4 rw,relatime 0 0
proc /proc proc rw,nosuid,nodev,noexec,relatime 0 0
sysfs /sys sysfs rw,nosuid,nodev,noexec,relatime 0 0
tmpfs /run tmpfs rw,nosuid,nodev,noexec,relatime,size=1612516k,mode=755 0 0
/dev/loop0 /snap/core22/1380 squashfs ro,nodev,relatime 0 0
/dev/sdb1 /media/alex/My\\040Disk ext4 rw,nosuid,nodev,relatime 0 0
overlay /var/lib/docker/overlay2/3f1c/merged overlay rw,relatime,lowerdir=/var/lib/docker/overlay2/l/A 0 0
";

    fn mount(source: &str, point: &str) -> Mount {
        Mount { source: source.into(), point: point.into() }
    }

    #[test]
    fn keeps_block_devices_outside_the_hidden_trees() {
        let kept = parse_mounts(MOUNTS);
        assert_eq!(
            kept,
            vec![
                mount("/dev/nvme0n1p2", "/"),
                mount("/dev/nvme0n1p3", "/home"),
                mount("/dev/sdb1", "/media/alex/My Disk"),
            ]
        );
    }

    #[test]
    fn keeps_a_root_without_a_device_source() {
        let kept = parse_mounts("overlay / overlay rw,relatime 0 0\nproc /proc proc rw 0 0\n");
        assert_eq!(kept, vec![mount("overlay", "/")]);
    }

    #[test]
    fn picks_the_longest_mount_holding_the_path() {
        let kept = parse_mounts(MOUNTS);
        assert_eq!(mount_for(&kept, "/home/alex/x").unwrap().point, "/home");
        assert_eq!(mount_for(&kept, "/homework").unwrap().point, "/");
        assert_eq!(mount_for(&kept, "/media/alex/My Disk").unwrap().point, "/media/alex/My Disk");
        assert_eq!(mount_for(&kept, "relative").unwrap().point, "/");
    }

    #[test]
    fn decodes_udev_label_escapes() {
        assert_eq!(unescape("Time\\x20Machine"), "Time Machine");
        assert_eq!(unescape("a\\x5cb"), "a\\b");
        assert_eq!(unescape("plain"), "plain");
    }

    #[test]
    fn lists_the_root_volume_first() {
        let list = volumes().unwrap();
        let root = &list[0];
        assert_eq!(root.mount, "/");
        assert!(!root.name.is_empty());
        assert!(root.total > 0);
    }
}
