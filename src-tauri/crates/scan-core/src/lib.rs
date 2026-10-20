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
    /// Device and inode, so live updates can tell a new hard link to a scanned file from a new
    /// file. 0 when a walker did not record them.
    pub dev: Vec<u32>,
    pub ino: Vec<u64>,
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
            dev: Vec::with_capacity(nodes),
            ino: Vec::with_capacity(nodes),
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
        self.dev.push(0);
        self.ino.push(0);
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
            self.dev[id as usize] = sub.dev[j];
            self.ino[id as usize] = sub.ino[j];
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

/// Children of every node, sorted by size descending, each node's run contiguous in `list`.
/// Built once after a scan so views never scan the whole arena, then kept current with
/// `extend` as live updates append nodes: a node that gains children has its run moved to the
/// end of `list`, leaving the old run as dead space that a rebuild reclaims once it dominates.
pub struct ChildIndex {
    start: Vec<u32>,
    end: Vec<u32>,
    list: Vec<NodeId>,
    /// Slots in `list` no run points at any more.
    dead: usize,
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
        let mut end = start.clone();
        end.pop();
        let mut list = vec![0 as NodeId; n.saturating_sub(1)];
        for i in 1..n {
            let p = arena.parent[i] as usize;
            list[end[p] as usize] = i as NodeId;
            end[p] += 1;
        }
        start.pop();
        let mut index = Self { start, end, list, dead: 0 };
        for i in 0..n {
            index.resort(arena, i as NodeId);
        }
        index
    }

    /// How many nodes the index covers; nodes from here on are not indexed yet.
    pub fn len(&self) -> usize {
        self.start.len()
    }

    pub fn is_empty(&self) -> bool {
        self.start.is_empty()
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        let i = id as usize;
        &self.list[self.start[i] as usize..self.end[i] as usize]
    }

    /// Re-sort one node's children after sizes changed (e.g. after a trash).
    pub fn resort(&mut self, arena: &Arena, id: NodeId) {
        let i = id as usize;
        let (a, b) = (self.start[i] as usize, self.end[i] as usize);
        if b - a > 1 {
            self.list[a..b].sort_unstable_by_key(|&c| std::cmp::Reverse(arena.size[c as usize]));
        }
    }

    /// Index the nodes appended to `arena` since the index last covered it (`insert` and `graft`
    /// only ever append). Costs the new nodes plus the children of each existing node that
    /// gained one, instead of the whole arena.
    pub fn extend(&mut self, arena: &Arena) {
        let from = self.len();
        let n = arena.len();
        if from >= n {
            return;
        }
        if self.dead > self.list.len() / 2 {
            *self = Self::build(arena);
            return;
        }
        // New nodes get contiguous runs after the current list, laid out by a counting pass.
        let mut count = vec![0u32; n - from];
        let mut to_old: Vec<(NodeId, NodeId)> = Vec::new();
        for i in from..n {
            let p = arena.parent[i] as usize;
            if p >= from {
                count[p - from] += 1;
            } else {
                to_old.push((p as NodeId, i as NodeId));
            }
        }
        let mut cursor = self.list.len() as u32;
        self.start.reserve(n - from);
        self.end.reserve(n - from);
        for &c in &count {
            self.start.push(cursor);
            self.end.push(cursor);
            cursor += c;
        }
        self.list.resize(cursor as usize, 0);
        for i in from..n {
            let p = arena.parent[i] as usize;
            if p >= from {
                self.list[self.end[p] as usize] = i as NodeId;
                self.end[p] += 1;
            }
        }
        for p in from..n {
            self.resort(arena, p as NodeId);
        }
        // An existing node that gained children has its run moved to the end, with the new ones appended.
        to_old.sort_unstable();
        for group in to_old.chunk_by(|a, b| a.0 == b.0) {
            let p = group[0].0 as usize;
            let (a, b) = (self.start[p] as usize, self.end[p] as usize);
            if b != self.list.len() {
                self.start[p] = self.list.len() as u32;
                self.list.extend_from_within(a..b);
                self.dead += b - a;
            }
            self.list.extend(group.iter().map(|&(_, c)| c));
            self.end[p] = self.list.len() as u32;
            self.resort(arena, p as NodeId);
        }
    }
}

/// Shared cancel flag for a running scan.
pub type Cancel = AtomicBool;

/// First occurrence of a (dev, ino) with nlink > 1 counts; later ones are dups. Walkers ask
/// `is_dup` as they read entries and `record` the node once its id is known, so the scan keeps
/// a `live::Hardlinks` registry for updating link sets later.
#[derive(Default)]
pub struct HardlinkSet {
    seen: Mutex<HashSet<(u64, u64)>>,
    links: Mutex<live::Hardlinks>,
}

impl HardlinkSet {
    /// Returns true if this inode was already seen.
    pub fn is_dup(&self, dev: u64, ino: u64) -> bool {
        !self.seen.lock().unwrap().insert((dev, ino))
    }

    /// Remember which node holds this inode. `counted` is the negation of what `is_dup` answered.
    pub fn record(&self, dev: u64, ino: u64, id: NodeId, counted: bool) {
        self.links.lock().unwrap().add(dev, ino, id, counted);
    }

    pub fn finish(self) -> live::Hardlinks {
        self.links.into_inner().unwrap()
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

/// Raise the soft fd limit so deep queues of open directories do not hit EMFILE.
pub fn raise_fd_limit() {
    unsafe {
        let mut rl = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            let want = 10240.min(rl.rlim_max);
            if rl.rlim_cur < want {
                rl.rlim_cur = want;
                libc::setrlimit(libc::RLIMIT_NOFILE, &rl);
            }
        }
    }
}

#[cfg(test)]
mod child_index_tests {
    use super::*;

    fn arena_of(parents: &[(NodeId, u64)]) -> Arena {
        let mut arena = Arena::default();
        arena.push(NONE, b"r", 0, 1, Flags::DIR);
        for (k, &(p, size)) in parents.iter().enumerate() {
            arena.push(p, format!("n{k}").as_bytes(), size, 1, Flags::DIR);
        }
        arena
    }

    /// Every node's children, as a fresh build would list them.
    fn assert_same_as_build(index: &ChildIndex, arena: &Arena) {
        let fresh = ChildIndex::build(arena);
        assert_eq!(index.len(), arena.len());
        for id in 0..arena.len() as NodeId {
            assert_eq!(index.children(id), fresh.children(id), "children of {id}");
        }
    }

    #[test]
    fn extend_indexes_appended_nodes_under_old_and_new_parents() {
        // r: 1 (30), 2 (20); 1: 3 (5)
        let mut arena = arena_of(&[(0, 30), (0, 20), (1, 5)]);
        let mut index = ChildIndex::build(&arena);
        assert_eq!(index.children(0), &[1, 2]);

        // A child of the root between 1 and 2 in size, a child of 2 (first for it), and a new dir
        // under 1 with children of its own, which lifts 1 to 41.
        arena.insert(0, b"mid", 35, 1, Flags::empty());
        arena.insert(2, b"c", 1, 1, Flags::empty());
        let d = arena.insert(1, b"d", 0, 1, Flags::DIR);
        arena.insert(d, b"e", 2, 1, Flags::empty());
        arena.insert(d, b"f", 9, 1, Flags::empty());
        index.extend(&arena);

        assert_eq!(index.children(0), &[1, 4, 2]);
        assert_eq!(index.children(2), &[5]);
        assert_eq!(index.children(d), &[8, 7]);
        assert_same_as_build(&index, &arena);

        // Idempotent, and further rounds keep every run intact.
        index.extend(&arena);
        arena.insert(0, b"g", 1, 1, Flags::empty());
        arena.insert(d, b"h", 100, 1, Flags::empty());
        index.extend(&arena);
        assert_same_as_build(&index, &arena);
    }

    #[test]
    fn extend_rebuilds_once_dead_space_dominates() {
        let mut arena = arena_of(&[(0, 1)]);
        let mut index = ChildIndex::build(&arena);
        // Distinct sizes: the sort is unstable, so ties could order differently from a build.
        for k in 2..66u64 {
            arena.insert(0, format!("f{k}").as_bytes(), k, 1, Flags::empty());
            index.extend(&arena);
            assert_same_as_build(&index, &arena);
        }
        assert!(index.dead <= index.list.len(), "dead space is bounded by the list");
    }
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
