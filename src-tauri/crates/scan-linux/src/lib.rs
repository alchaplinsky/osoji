//! Linux file watching for Osoji: the inotify counterpart of `scan-macos`'s FSEvents watcher.

pub mod watch;

pub use watch::Watcher;
