//! Shared scan data model: a column-oriented arena, hard-link dedup, progress
//! counters, and the jwalk reference walker (milestone M1).

pub mod jwalk_walker;

use bitflags::bitflags;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

pub type NodeId = u32;
pub const NONE: NodeId = u32::MAX;

bitflags! {
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub struct Flags: u8 {
        const DIR = 1;
        const SYMLINK = 2;
        const HARDLINK_DUP = 4;
        const MOUNT = 8;
        const EXCLUDED = 16;
        const ERROR = 32;
        const TRASHED = 64;
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
        let i = id as usize;
        if self.flags[i].contains(Flags::TRASHED) {
            return 0;
        }
        let size = self.size[i];
        let items = self.items[i] + 1;
        self.flags[i] |= Flags::TRASHED;
        self.size[i] = 0;
        let mut p = self.parent[i];
        while p != NONE {
            self.size[p as usize] -= size;
            self.items[p as usize] -= items;
            p = self.parent[p as usize];
        }
        size
    }
}

/// Children of every node, sorted by size descending, in CSR form.
/// Built once after a scan so views never scan the whole arena.
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

#[derive(Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
}

impl Progress {
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
