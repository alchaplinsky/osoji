//! The OS-specific edges of the backend: which walker runs, process setup, volumes, file-manager
//! hand-offs and the permission probe. Each OS implements the same set of functions in its own
//! file; lib.rs only ever calls `platform::*`.

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct VolumeInfo {
    pub name: String,
    pub mount: String,
    pub total: u64,
    pub free: u64,
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("unshift builds on macOS and Linux only; see src-tauri/src/platform");
