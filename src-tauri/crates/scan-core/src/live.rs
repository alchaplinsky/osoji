//! Keeping a finished scan current without walking it again: one directory at a time,
//! from a fresh listing of that directory.
//!
//! `reconcile` appends nodes the `ChildIndex` has never seen, so the index is stale right
//! after it: reconcile a whole batch of directories, then build the index again. New
//! directories come back in `new_dirs` holding only their own blocks; the caller walks each one and `graft`s it.
//!
//! Hard links are not deduplicated here: a link that appears counts in full. Nodes the scan
//! already flagged `HARDLINK_DUP` keep counting as 0.

use crate::{Arena, ChildIndex, Flags, NodeId, NONE};
use std::collections::{HashMap, HashSet};

/// One entry of a directory listing, as a watcher re-reads it. Sizes are allocated bytes
/// (`blocks * 512`), matching what the walkers record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub name: Vec<u8>,
    pub size: u64,
    pub mtime: i64,
    /// `DIR`, `SYMLINK` and `ERROR` as the walkers would set them; policy flags are never set here.
    pub flags: Flags,
    pub dev: u64,
    pub ino: u64,
    /// Link count; above 1 the entry shares its inode, and `Hardlinks` decides which node counts.
    pub nlink: u32,
}

/// Which scanned nodes share an inode (`nlink > 1`), and which of them carries the bytes.
/// Filled by the walkers through `HardlinkSet`, then kept current by `reconcile`.
#[derive(Default, Debug)]
pub struct Hardlinks {
    /// Every scanned node of a multiply-linked inode; the first carries the bytes, the rest are
    /// `HARDLINK_DUP` at size 0.
    members: HashMap<(u64, u64), Vec<NodeId>>,
    inode_of: HashMap<NodeId, (u64, u64)>,
}

impl Hardlinks {
    /// Walkers call this for every entry with `nlink > 1`, in any order across threads: the
    /// counted node goes first whenever it is recorded.
    pub fn add(&mut self, dev: u64, ino: u64, id: NodeId, counted: bool) {
        let m = self.members.entry((dev, ino)).or_default();
        if counted {
            m.insert(0, id);
        } else {
            m.push(id);
        }
        self.inode_of.insert(id, (dev, ino));
    }

    pub fn inode(&self, id: NodeId) -> Option<(u64, u64)> {
        self.inode_of.get(&id).copied()
    }

    /// The node that carries this inode's bytes, if the scan holds any link to it.
    pub fn counted(&self, dev: u64, ino: u64) -> Option<NodeId> {
        self.members.get(&(dev, ino)).and_then(|m| m.first().copied())
    }

    pub fn members(&self, dev: u64, ino: u64) -> &[NodeId] {
        self.members.get(&(dev, ino)).map_or(&[], Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.inode_of.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inode_of.is_empty()
    }
}

/// What `reconcile` did to one directory's children.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Reconciled {
    pub removed: Vec<NodeId>,
    pub changed: Vec<NodeId>,
    pub added: Vec<NodeId>,
    /// Inserted holding only their own blocks; the caller walks each and grafts the result.
    pub new_dirs: Vec<NodeId>,
}

impl Reconciled {
    pub fn is_empty(&self) -> bool {
        self.removed.is_empty() && self.changed.is_empty() && self.added.is_empty() && self.new_dirs.is_empty()
    }
}

const GONE: Flags = Flags::TRASHED.union(Flags::REMOVED);

/// A node under a trashed or removed ancestor no longer counts toward the root, so
/// updating it would take its bytes off the ancestors a second time.
fn is_gone(arena: &Arena, mut id: NodeId) -> bool {
    while id != NONE {
        if arena.flags[id as usize].intersects(GONE) {
            return true;
        }
        id = arena.parent[id as usize];
    }
    false
}

/// Bring `dir`'s direct children in line with `listing`, matching by name. Children
/// already trashed or removed count as absent, so a name that comes back is a new node.
/// A child whose kind changed (file to directory or back) is removed and added again.
/// Directories in both are left alone: their totals follow their own children, which
/// the watcher reports as a separate directory.
///
/// Does nothing when `dir` or an ancestor is gone: a deleted tree also reports its subdirectories.
pub fn reconcile(
    arena: &mut Arena,
    index: &ChildIndex,
    _links: &mut Hardlinks,
    dir: NodeId,
    listing: &[DirEntry],
) -> Reconciled {
    let mut out = Reconciled::default();
    if is_gone(arena, dir) {
        return out;
    }
    let listed: HashMap<&[u8], &DirEntry> = listing.iter().map(|e| (e.name.as_slice(), e)).collect();
    let mut kept: HashSet<&[u8]> = HashSet::with_capacity(listing.len());
    for &c in index.children(dir) {
        let i = c as usize;
        let flags = arena.flags[i];
        if flags.intersects(GONE) {
            continue;
        }
        let is_dir = flags.contains(Flags::DIR);
        let entry = listed.get(arena.name(c)).filter(|e| e.flags.contains(Flags::DIR) == is_dir);
        let Some(e) = entry else {
            arena.mark_removed(c);
            out.removed.push(c);
            continue;
        };
        kept.insert(e.name.as_slice());
        if is_dir {
            continue;
        }
        let size = if flags.contains(Flags::HARDLINK_DUP) { 0 } else { e.size };
        if size != arena.size[i] || e.mtime != arena.mtime[i] {
            arena.set_size(c, size, e.mtime);
            out.changed.push(c);
        }
    }
    for e in listing.iter().filter(|e| !kept.contains(e.name.as_slice())) {
        if e.flags.contains(Flags::DIR) {
            out.new_dirs.push(arena.insert(dir, &e.name, e.size, e.mtime, e.flags));
        } else {
            out.added.push(arena.insert(dir, &e.name, e.size, e.mtime, e.flags));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    /// A freshly scanned arena rooted at `root`. Paths ending in `/` are directories and
    /// must follow their parent.
    fn tree(root: &str, nodes: &[(&str, u64)]) -> Arena {
        let mut arena = Arena::default();
        let mut ids = HashMap::from([(String::new(), arena.push(NONE, root.as_bytes(), 0, 1, Flags::DIR))]);
        for &(path, size) in nodes {
            let (path, flags) = match path.strip_suffix('/') {
                Some(p) => (p, Flags::DIR),
                None => (path, Flags::empty()),
            };
            let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
            let id = arena.push(ids[parent], name.as_bytes(), size, 1, flags);
            ids.insert(path.to_string(), id);
        }
        arena.aggregate();
        arena
    }

    const BASE: &[(&str, u64)] = &[("a/", 64), ("a/x", 100), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", 300), ("b", 50)];

    /// Live nodes by path, so arenas with different id layouts compare equal.
    fn dump(arena: &Arena) -> Vec<(PathBuf, u64, u32)> {
        let mut out: Vec<_> = (0..arena.len() as NodeId)
            .filter(|&i| !is_gone(arena, i))
            .map(|i| (arena.path(i), arena.size[i as usize], arena.items[i as usize]))
            .collect();
        out.sort();
        out
    }

    fn assert_matches(arena: &Arena, expected: &[(&str, u64)]) {
        assert_eq!(dump(arena), dump(&tree("r", expected)));
    }

    fn node(arena: &Arena, path: &str) -> NodeId {
        let want = Path::new("r").join(path);
        (0..arena.len() as NodeId).rev().find(|&i| !is_gone(arena, i) && arena.path(i) == want).unwrap()
    }

    fn listing(arena: &Arena, index: &ChildIndex, dir: NodeId) -> Vec<DirEntry> {
        let kind = Flags::DIR | Flags::SYMLINK | Flags::ERROR;
        index
            .children(dir)
            .iter()
            .map(|&c| c as usize)
            .filter(|&c| !arena.flags[c].intersects(GONE))
            .map(|c| DirEntry {
                name: arena.name(c as NodeId).to_vec(),
                size: arena.size[c],
                mtime: arena.mtime[c],
                flags: arena.flags[c] & kind,
                dev: 0,
                ino: 0,
                nlink: 1,
            })
            .collect()
    }

    fn file(name: &str, size: u64) -> DirEntry {
        DirEntry { name: name.into(), size, mtime: 1, flags: Flags::empty(), dev: 0, ino: 0, nlink: 1 }
    }

    fn dir(name: &str) -> DirEntry {
        DirEntry { name: name.into(), size: 64, mtime: 1, flags: Flags::DIR, dev: 0, ino: 0, nlink: 1 }
    }

    fn without(mut list: Vec<DirEntry>, name: &str) -> Vec<DirEntry> {
        list.retain(|e| e.name != name.as_bytes());
        list
    }

    fn setup() -> (Arena, ChildIndex) {
        let arena = tree("r", BASE);
        let index = ChildIndex::build(&arena);
        (arena, index)
    }

    #[test]
    fn removes_a_file() {
        let (mut arena, index) = setup();
        let (a, x) = (node(&arena, "a"), node(&arena, "a/x"));
        let list = without(listing(&arena, &index, a), "x");
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert_eq!(r, Reconciled { removed: vec![x], ..Default::default() });
        assert!(arena.flags[x as usize].contains(Flags::REMOVED));
        assert_matches(&arena, &[("a/", 64), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", 300), ("b", 50)]);
    }

    #[test]
    fn removes_a_directory_with_its_children() {
        let (mut arena, index) = setup();
        let (a, sub) = (node(&arena, "a"), node(&arena, "a/sub"));
        let list = without(listing(&arena, &index, a), "sub");
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert_eq!(r.removed, vec![sub]);
        assert_matches(&arena, &[("a/", 64), ("a/x", 100), ("a/y", 200), ("b", 50)]);
    }

    #[test]
    fn ignores_a_directory_inside_a_removed_tree() {
        let (mut arena, index) = setup();
        let (a, sub) = (node(&arena, "a"), node(&arena, "a/sub"));
        let list = without(listing(&arena, &index, a), "sub");
        reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert!(reconcile(&mut arena, &index, &mut Hardlinks::default(), sub, &[]).is_empty());
        assert_matches(&arena, &[("a/", 64), ("a/x", 100), ("a/y", 200), ("b", 50)]);
    }

    #[test]
    fn updates_a_modified_file() {
        let (mut arena, index) = setup();
        let (a, x) = (node(&arena, "a"), node(&arena, "a/x"));
        let mut list = without(listing(&arena, &index, a), "x");
        list.push(DirEntry { mtime: 2, ..file("x", 150) });
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert_eq!(r, Reconciled { changed: vec![x], ..Default::default() });
        assert_eq!(arena.mtime[x as usize], 2);
        assert_matches(&arena, &[("a/", 64), ("a/x", 150), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", 300), ("b", 50)]);
    }

    #[test]
    fn adds_a_file() {
        let (mut arena, index) = setup();
        let mut list = listing(&arena, &index, 0);
        list.push(file("c", 70));
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), 0, &list);
        assert_eq!(r, Reconciled { added: vec![node(&arena, "c")], ..Default::default() });
        assert_matches(&arena, &[BASE, &[("c", 70)]].concat());
    }

    #[test]
    fn adds_a_directory_then_grafts_its_walk() {
        let (mut arena, index) = setup();
        let mut list = listing(&arena, &index, 0);
        list.push(dir("d"));
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), 0, &list);
        let d = node(&arena, "d");
        assert_eq!(r, Reconciled { new_dirs: vec![d], ..Default::default() });

        arena.graft(&tree("d", &[("e", 10), ("f/", 64), ("f/g", 20)]), d);
        assert_matches(&arena, &[BASE, &[("d/", 64), ("d/e", 10), ("d/f/", 64), ("d/f/g", 20)]].concat());
        assert!((1..arena.len()).all(|i| (arena.parent[i] as usize) < i));
        assert_eq!(ChildIndex::build(&arena).children(d).len(), 2);
    }

    #[test]
    fn rescans_a_subtree_by_grafting_over_removed_children() {
        let (mut arena, index) = setup();
        let a = node(&arena, "a");
        for &c in index.children(a) {
            arena.mark_removed(c);
        }
        arena.graft(&tree("a", &[("x", 120), ("w/", 64), ("w/v", 5)]), a);
        assert_matches(&arena, &[("a/", 64), ("a/x", 120), ("a/w/", 64), ("a/w/v", 5), ("b", 50)]);
    }

    #[test]
    fn treats_a_rename_as_remove_and_add() {
        let (mut arena, index) = setup();
        let (a, x) = (node(&arena, "a"), node(&arena, "a/x"));
        let mut list = without(listing(&arena, &index, a), "x");
        list.push(file("x2", 100));
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert_eq!(r, Reconciled { removed: vec![x], added: vec![node(&arena, "a/x2")], ..Default::default() });
        assert_matches(&arena, &[("a/", 64), ("a/x2", 100), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", 300), ("b", 50)]);
    }

    #[test]
    fn a_reappearing_name_is_a_new_node() {
        let (mut arena, index) = setup();
        let b = node(&arena, "b");
        let list = listing(&arena, &index, 0);
        reconcile(&mut arena, &index, &mut Hardlinks::default(), 0, &without(list.clone(), "b"));

        let index = ChildIndex::build(&arena);
        let r = reconcile(
            &mut arena,
            &index,
            &mut Hardlinks::default(),
            0,
            &[without(list, "b"), vec![file("b", 80)]].concat(),
        );
        assert_eq!(r.added.len(), 1);
        assert_ne!(r.added[0], b);
        assert!(r.removed.is_empty() && r.changed.is_empty());
        assert_matches(&arena, &[("a/", 64), ("a/x", 100), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", 300), ("b", 80)]);
    }

    #[test]
    fn a_dir_replaced_by_a_file_is_removed_and_added() {
        let (mut arena, index) = setup();
        let (a, sub) = (node(&arena, "a"), node(&arena, "a/sub"));
        let mut list = without(listing(&arena, &index, a), "sub");
        list.push(file("sub", 7));
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list);
        assert_eq!(r, Reconciled { removed: vec![sub], added: vec![node(&arena, "a/sub")], ..Default::default() });
        assert_matches(&arena, &[("a/", 64), ("a/x", 100), ("a/y", 200), ("a/sub", 7), ("b", 50)]);
    }

    #[test]
    fn reconciling_the_same_listing_again_changes_nothing() {
        let (mut arena, index) = setup();
        let a = node(&arena, "a");
        let mut list = without(listing(&arena, &index, a), "y");
        list.retain(|e| e.name != b"x");
        list.extend([file("x", 1), file("w", 2), dir("new")]);
        assert!(!reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list).is_empty());
        let after = dump(&arena);

        let index = ChildIndex::build(&arena);
        assert!(reconcile(&mut arena, &index, &mut Hardlinks::default(), a, &list).is_empty());
        assert_eq!(dump(&arena), after);
    }

    #[test]
    #[ignore]
    fn bench_child_index_build_and_reconcile() {
        use std::io::Write;
        use std::time::Instant;

        let mut arena = Arena::with_capacity(2_000_000, 16 << 20);
        let mut level = vec![arena.push(NONE, b"r", 0, 1, Flags::DIR)];
        for _ in 0..3 {
            let mut next = Vec::with_capacity(level.len() * 30);
            for &p in &level {
                next.extend((0..30).map(|k| arena.push(p, format!("d{k}").as_bytes(), 4096, 1, Flags::DIR)));
            }
            level = next;
        }
        for &p in &level {
            for k in 0..72u64 {
                arena.push(p, format!("f{k}").as_bytes(), 4096 * (k + 1), 1, Flags::empty());
            }
        }
        let big = arena.push(0, b"big", 4096, 1, Flags::DIR);
        for k in 0..1000u64 {
            arena.push(big, format!("f{k}").as_bytes(), 4096 * (k + 1), 1, Flags::empty());
        }
        arena.aggregate();

        let t = Instant::now();
        let index = ChildIndex::build(&arena);
        let build = t.elapsed();

        let mut list = listing(&arena, &index, big);
        list.retain(|e| !e.name.ends_with(b"0"));
        for e in list.iter_mut().filter(|e| e.name.ends_with(b"1")) {
            e.size += 512;
        }
        list.extend((0..100).map(|k| file(&format!("new{k}"), 4096)));
        let t = Instant::now();
        let r = reconcile(&mut arena, &index, &mut Hardlinks::default(), big, &list);
        let rec = t.elapsed();
        assert_eq!((r.removed.len(), r.changed.len(), r.added.len()), (100, 100, 100));

        // The harness only captures the print macros; writing to stderr directly shows the
        // numbers without --nocapture.
        let n = arena.len();
        std::io::stderr()
            .write_all(
                format!("{n} nodes: ChildIndex::build {build:?}, reconcile of 1000 entries {rec:?}\n").as_bytes(),
            )
            .unwrap();
    }
}
