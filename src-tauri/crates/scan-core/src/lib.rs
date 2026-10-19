//! Shared scan data model: a column-oriented arena, hard-link dedup, progress
//! counters, and the jwalk reference walker (milestone M1).

pub mod jwalk_walker;
pub mod live;

use bitflags::bitflags;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

pub type NodeId = u32;
pub const NONE: NodeId = u32::MAX;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Flags: u16 {
        const DIR = 1;
        const SYMLINK = 2;
        const HARDLINK_DUP = 4;
        const MOUNT = 8;
        const EXCLUDED = 16;
        const ERROR = 32;
        const TRASHED = 64;
        const PROTECTED = 128;
        const RISKY = 256;
        const UNEXPECTED = 512;
        /// The user's Trash folder: real space, but never swept and never suggested.
        const TRASH_FOLDER = 1024;
        /// Disappeared from disk since the scan, seen by the watcher.
        const REMOVED = 2048;
    }
}

/// Column-oriented node storage. `parent[i] < i` for every non-root node,
/// which lets aggregation run as a single reverse pass.
#[derive(Default)]
pub struct Arena {
    pub size: Vec<u64>,
    pub parent: Vec<NodeId>,
    pub items: Vec<u32>,
    pub flags: Vec<Flags>,
    pub mtime: Vec<i64>,
    pub name_off: Vec<u32>,
    pub name_len: Vec<u16>,
    pub names: Vec<u8>,
}

impl Arena {
    pub fn with_capacity(nodes: usize, name_bytes: usize) -> Self {
        Self {
            size: Vec::with_capacity(nodes),
            parent: Vec::with_capacity(nodes),
            items: Vec::with_capacity(nodes),
            flags: Vec::with_capacity(nodes),
            mtime: Vec::with_capacity(nodes),
            name_off: Vec::with_capacity(nodes),
            name_len: Vec::with_capacity(nodes),
            names: Vec::with_capacity(name_bytes),
        }
    }

    pub fn len(&self) -> usize {
        self.size.len()
    }

    pub fn is_empty(&self) -> bool {
        self.size.is_empty()
    }

    pub fn push(&mut self, parent: NodeId, name: &[u8], size: u64, mtime: i64, flags: Flags) -> NodeId {
        let id = self.size.len() as NodeId;
        self.size.push(size);
        self.parent.push(parent);
        self.items.push(0);
        self.flags.push(flags);
        self.mtime.push(mtime);
        self.name_off.push(self.names.len() as u32);
        self.name_len.push(name.len().min(u16::MAX as usize) as u16);
        self.names.extend_from_slice(name);
        id
    }

    pub fn name(&self, id: NodeId) -> &[u8] {
        let off = self.name_off[id as usize] as usize;
        &self.names[off..off + self.name_len[id as usize] as usize]
    }

    /// Sum sizes and item counts into parents. Requires `parent[i] < i`.
    pub fn aggregate(&mut self) {
        for i in (1..self.len()).rev() {
            let p = self.parent[i] as usize;
            self.size[p] += self.size[i];
            self.items[p] += self.items[i] + 1;
        }
    }

    pub fn children(&self, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.len() as NodeId).filter(move |&i| self.parent[i as usize] == id)
    }

    pub fn path(&self, mut id: NodeId) -> PathBuf {
        use std::os::unix::ffi::OsStrExt;
        let mut parts = Vec::new();
        loop {
            parts.push(self.name(id));
            let p = self.parent[id as usize];
            if p == NONE {
                break;
            }
            id = p;
        }
        let mut path = PathBuf::new();
        for part in parts.iter().rev() {
            path.push(std::ffi::OsStr::from_bytes(part));
        }
        path
    }

    pub fn count_flag(&self, flag: Flags) -> usize {
        self.flags.iter().filter(|f| f.contains(flag)).count()
    }

    /// Mark a node as moved to the Trash: zero its size, subtract it along the
    /// parent chain, and flag it. Returns the bytes removed from the tree.
    pub fn mark_trashed(&mut self, id: NodeId) -> u64 {
        self.detach(id, Flags::TRASHED)
    }

    /// Like `mark_trashed`, for a node that vanished from disk on its own.
    pub fn mark_removed(&mut self, id: NodeId) -> u64 {
        self.detach(id, Flags::REMOVED)
    }

    /// O(depth): descendants keep their columns, only the node and its ancestors
    /// stop counting them.
    fn detach(&mut self, id: NodeId, flag: Flags) -> u64 {
        let i = id as usize;
        if self.flags[i].intersects(Flags::TRASHED | Flags::REMOVED) {
            return 0;
        }
        let size = self.size[i];
        self.flags[i] |= flag;
        self.size[i] = 0;
        self.bump(self.parent[i], -(size as i64), -(i64::from(self.items[i]) + 1));
        size
    }

    /// Set a file node's size and mtime after aggregation, carrying the change up
    /// the parent chain. Returns the size delta.
    pub fn set_size(&mut self, id: NodeId, size: u64, mtime: i64) -> i64 {
        let i = id as usize;
        let delta = size as i64 - self.size[i] as i64;
        self.mtime[i] = mtime;
        self.bump(id, delta, 0);
        delta
    }

    /// Add a node after aggregation. `parent[i] < i` still holds because the new id
    /// is the largest in the arena.
    pub fn insert(&mut self, parent: NodeId, name: &[u8], size: u64, mtime: i64, flags: Flags) -> NodeId {
        let id = self.push(parent, name, size, mtime, flags);
        self.bump(parent, size as i64, 1);
        id
    }

    /// Append an aggregated arena below `under`, its root standing for `under`
    /// itself. Walkers zero the root's own size, so `sub.size[0]` is exactly what the
    /// grafted nodes add. Appending keeps `parent[i] < i`.
    pub fn graft(&mut self, sub: &Arena, under: NodeId) {
        if sub.is_empty() {
            return;
        }
        let base = self.len() as NodeId - 1;
        for j in 1..sub.len() {
            let p = sub.parent[j];
            let parent = if p == 0 { under } else { base + p };
            let id = self.push(parent, sub.name(j as NodeId), sub.size[j], sub.mtime[j], sub.flags[j]);
            self.items[id as usize] = sub.items[j];
        }
        self.bump(under, sub.size[0] as i64, i64::from(sub.items[0]));
    }

    /// Saturating so a bookkeeping slip shows up as a wrong total, not a wrapped one.
    fn bump(&mut self, mut id: NodeId, size: i64, items: i64) {
        while id != NONE {
            let i = id as usize;
            self.size[i] = self.size[i].saturating_add_signed(size);
            self.items[i] = self.items[i].saturating_add_signed(items as i32);
            id = self.parent[i];
        }
    }
}

/// Children of every node, sorted by size descending, in CSR form.
/// Built once after a scan so views never scan the whole arena. It cannot take new
/// children: after `insert` or `graft`, build it again (once per batch of updates).
pub struct ChildIndex {
    start: Vec<u32>,
    list: Vec<NodeId>,
}

impl ChildIndex {
    pub fn build(arena: &Arena) -> Self {
        let n = arena.len();
        let mut start = vec![0u32; n + 1];
        for i in 1..n {
            start[arena.parent[i] as usize + 1] += 1;
        }
        for i in 0..n {
            start[i + 1] += start[i];
        }
        let mut fill = start.clone();
        let mut list = vec![0 as NodeId; n.saturating_sub(1)];
        for i in 1..n {
            let p = arena.parent[i] as usize;
            list[fill[p] as usize] = i as NodeId;
            fill[p] += 1;
        }
        for i in 0..n {
            let (a, b) = (start[i] as usize, start[i + 1] as usize);
            if b - a > 1 {
                list[a..b].sort_unstable_by_key(|&c| std::cmp::Reverse(arena.size[c as usize]));
            }
        }
        Self { start, list }
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let i = id as usize;
        &self.list[self.start[i] as usize..self.start[i + 1] as usize]
    }

    /// Re-sort one node's children after sizes changed (e.g. after a trash).
    pub fn resort(&mut self, arena: &Arena, id: NodeId) {
        let i = id as usize;
        let (a, b) = (self.start[i] as usize, self.start[i + 1] as usize);
        self.list[a..b].sort_unstable_by_key(|&c| std::cmp::Reverse(arena.size[c as usize]));
    }
}

/// Shared cancel flag for a running scan.
pub type Cancel = AtomicBool;

/// First occurrence of a (dev, ino) with nlink > 1 counts; later ones are dups.
#[derive(Default)]
pub struct HardlinkSet(Mutex<HashSet<(u64, u64)>>);

impl HardlinkSet {
    /// Returns true if this inode was already seen.
    pub fn is_dup(&self, dev: u64, ino: u64) -> bool {
        !self.0.lock().unwrap().insert((dev, ino))
    }
}

/// How many of the root's direct children get their own progress counter. Slots are fixed so
/// attribution on the hot path is a single relaxed atomic add; later children share `rest`.
pub const MAX_TOPS: usize = 48;

pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    top_bytes: [AtomicU64; MAX_TOPS],
    top_names: Mutex<Vec<Vec<u8>>>,
    rest: AtomicU64,
}

impl Default for Progress {
    fn default() -> Self {
        Self {
            files: AtomicU64::new(0),
            dirs: AtomicU64::new(0),
            bytes: AtomicU64::new(0),
            errors: AtomicU64::new(0),
            top_bytes: std::array::from_fn(|_| AtomicU64::new(0)),
            top_names: Mutex::new(Vec::new()),
            rest: AtomicU64::new(0),
        }
    }
}

impl Progress {
    /// Slot for bytes that belong to no registered top-level child.
    pub const REST: u32 = u32::MAX;

    /// Give a direct child of the root its own counter. Returns `REST` once every slot is taken.
    pub fn register_top(&self, name: &[u8]) -> u32 {
        let mut names = self.top_names.lock().unwrap();
        if names.len() >= MAX_TOPS {
            return Self::REST;
        }
        names.push(name.to_vec());
        (names.len() - 1) as u32
    }

    pub fn add_top(&self, top: u32, bytes: u64) {
        if bytes == 0 {
            return;
        }
        match self.top_bytes.get(top as usize) {
            Some(slot) => slot.fetch_add(bytes, Ordering::Relaxed),
            None => self.rest.fetch_add(bytes, Ordering::Relaxed),
        };
    }

    /// Bytes found so far under each registered top-level child, in registration order.
    pub fn tops(&self) -> Vec<(String, u64)> {
        let names = self.top_names.lock().unwrap();
        names
            .iter()
            .zip(&self.top_bytes)
            .map(|(name, bytes)| (String::from_utf8_lossy(name).into_owned(), bytes.load(Ordering::Relaxed)))
            .collect()
    }

    pub fn rest(&self) -> u64 {
        self.rest.load(Ordering::Relaxed)
    }

    pub fn add(&self, files: u64, dirs: u64, bytes: u64, errors: u64) {
        if files != 0 {
            self.files.fetch_add(files, Ordering::Relaxed);
        }
        if dirs != 0 {
            self.dirs.fetch_add(dirs, Ordering::Relaxed);
        }
        if bytes != 0 {
            self.bytes.fetch_add(bytes, Ordering::Relaxed);
        }
        if errors != 0 {
            self.errors.fetch_add(errors, Ordering::Relaxed);
        }
    }
}

#[derive(Clone, Debug)]
pub struct ScanConfig {
    pub root: PathBuf,
    pub threads: usize,
    pub cross_mounts: bool,
    /// Run workers at user-initiated QoS so macOS schedules them on P-cores.
    pub qos: bool,
}

#[cfg(test)]
mod progress_tests {
    use super::*;

    #[test]
    fn tracks_bytes_per_registered_top_and_overflows_to_rest() {
        let p = Progress::default();
        let a = p.register_top(b"a");
        let b = p.register_top(b"b");
        p.add_top(a, 10);
        p.add_top(b, 5);
        p.add_top(a, 1);
        assert_eq!(p.tops(), vec![("a".to_string(), 11), ("b".to_string(), 5)]);
        assert_eq!(p.rest(), 0);

        for i in 0..MAX_TOPS {
            p.register_top(format!("extra{i}").as_bytes());
        }
        let overflow = p.register_top(b"late");
        assert_eq!(overflow, Progress::REST);
        p.add_top(overflow, 7);
        assert_eq!(p.rest(), 7);
        assert_eq!(p.tops().len(), MAX_TOPS);
    }
}
