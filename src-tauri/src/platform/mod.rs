//! What the app takes from the operating system: the walker, the directory listing, the file
//! watcher, volumes, and the few shell integrations. One implementation per platform with the
//! same surface, so lib.rs and live.rs never name a platform crate.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "macos")]
pub use macos::*;

use serde::Serialize;

#[derive(Serialize)]
pub struct VolumeInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub free: u64,
}
