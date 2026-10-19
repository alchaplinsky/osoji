//! Keeping a finished scan current without walking it again: one directory at a time,
//! from a fresh listing of that directory.
//!
//! `reconcile` appends nodes the `ChildIndex` has never seen, so the index is stale right
//! after it: reconcile a whole batch of directories, then build the index again. New
//! directories come back in `new_dirs` holding only their own blocks; the caller walks each one and `graft`s it.
//!
//! Hard links keep the scan's invariant: for every inode with links inside the scan, exactly one
//! live node carries the bytes and the others are `HARDLINK_DUP` at size 0. `Hardlinks` records
//! which node that is. When it leaves through `detach` (sweeps included), the first live link
//! takes over its size; a new link to a carried inode comes in as a dup. The arena's `dev`/`ino`
//! columns find the carrier of an inode the registry never saw, a file that had a single link at
//! scan time.

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

    /// Make `id` the member that carries the bytes, registering it if needed.
    fn promote(&mut self, key: (u64, u64), id: NodeId) {
        let m = self.members.entry(key).or_default();
        m.retain(|&n| n != id);
        m.insert(0, id);
        self.inode_of.insert(id, key);
    }

    /// Drop `id`. Returns its inode and whether it was the counted member.
    fn forget(&mut self, id: NodeId) -> Option<((u64, u64), bool)> {
        let key = self.inode_of.remove(&id)?;
        let m = self.members.get_mut(&key)?;
        let counted = m.first() == Some(&id);
        m.retain(|&n| n != id);
        if m.is_empty() {
            self.members.remove(&key);
        }
        Some((key, counted))
    }

    /// The counted member, if it still counts: a node detached straight through the arena leaves
    /// the registry stale.
    fn carrier(&self, arena: &Arena, key: (u64, u64)) -> Option<NodeId> {
        self.counted(key.0, key.1).filter(|&n| !is_gone(arena, n))
    }
}

/// The arena keeps the device in 32 bits.
fn short((dev, ino): (u64, u64)) -> (u32, u64) {
    (dev as u32, ino)
}

/// Live files counting in full for each `wanted` inode, in one pass over the arena: the registry
/// only knows inodes that had several links at scan time, so a new link may point at a file that
/// had one.
fn find_carriers(arena: &Arena, wanted: &HashSet<(u32, u64)>) -> HashMap<(u32, u64), NodeId> {
    let mut found = HashMap::new();
    if wanted.is_empty() {
        return found;
    }
    let skip = Flags::DIR | Flags::SYMLINK | Flags::HARDLINK_DUP;
    for i in 0..arena.len() {
        let key = (arena.dev[i], arena.ino[i]);
        if key.1 == 0 || arena.flags[i].intersects(skip) || !wanted.contains(&key) || found.contains_key(&key) {
            continue;
        }
        if !is_gone(arena, i as NodeId) {
            found.insert(key, i as NodeId);
            if found.len() == wanted.len() {
                break;
            }
        }
    }
    found
}

/// Whether a live node carries `key`, registering the carrier `find_carriers` turned up.
fn carried(arena: &Arena, links: &mut Hardlinks, found: &HashMap<(u32, u64), NodeId>, key: (u64, u64)) -> bool {
    if links.carrier(arena, key).is_some() {
        return true;
    }
    let Some(&n) = found.get(&short(key)) else { return false };
    links.promote(key, n);
    true
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

/// A listed name that now points at another inode (an atomic save replaces the file) is another
/// file, which matters when either is a link: the old inode may need a new carrier.
fn relinked(arena: &Arena, links: &Hardlinks, c: NodeId, e: &DirEntry) -> bool {
    let i = c as usize;
    let linked = e.nlink > 1 || links.inode(c).is_some();
    linked && arena.ino[i] != 0 && e.ino != 0 && (arena.dev[i], arena.ino[i]) != short((e.dev, e.ino))
}

/// How a node left the tree: the user trashed it, or it vanished from disk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Detached {
    Trashed,
    Removed,
}

/// `Arena::mark_trashed` / `mark_removed` that keeps one carrier per inode: every link `id` or
/// its subtree carried hands its bytes to the first live link left outside, and leaves the
/// registry. `index` must hold the subtree. Returns the bytes the tree lost, so a counted link
/// whose inode lives on elsewhere frees nothing. A node already gone, or under a gone ancestor,
/// returns 0.
pub fn detach(arena: &mut Arena, index: &ChildIndex, links: &mut Hardlinks, id: NodeId, how: Detached) -> u64 {
    if is_gone(arena, id) {
        return 0;
    }
    // Detaching zeroes the node's own size; its descendants keep theirs.
    let size = arena.size[id as usize];
    let lost = match how {
        Detached::Trashed => arena.mark_trashed(id),
        Detached::Removed => arena.mark_removed(id),
    };
    if links.is_empty() {
        return lost;
    }
    let mut handed = 0;
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        // A child detached earlier already handed its links on.
        stack.extend(index.children(n).iter().filter(|&&c| !arena.flags[c as usize].intersects(GONE)));
        let Some((key, true)) = links.forget(n) else { continue };
        let Some(&heir) = links.members(key.0, key.1).iter().find(|&&m| !is_gone(arena, m)) else { continue };
        let i = heir as usize;
        let bytes = if n == id { size } else { arena.size[n as usize] };
        arena.flags[i].remove(Flags::HARDLINK_DUP);
        arena.set_size(heir, bytes, arena.mtime[i]);
        links.promote(key, heir);
        handed += bytes;
    }
    lost.saturating_sub(handed)
}

/// Bring `dir`'s direct children in line with `listing`, matching by name. Children
/// already trashed or removed count as absent, so a name that comes back is a new node.
/// A child whose kind changed (file to directory or back), or a link whose name now points at
/// another inode, is removed and added again.
/// Directories in both are left alone: their totals follow their own children, which
/// the watcher reports as a separate directory.
///
/// Does nothing when `dir` or an ancestor is gone: a deleted tree also reports its subdirectories.
pub fn reconcile(
    arena: &mut Arena,
    index: &ChildIndex,
    links: &mut Hardlinks,
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
        let entry = listed
            .get(arena.name(c))
            .filter(|e| e.flags.contains(Flags::DIR) == is_dir && !relinked(arena, links, c, e));
        let Some(e) = entry else {
            detach(arena, index, links, c, Detached::Removed);
            out.removed.push(c);
            continue;
        };
        kept.insert(e.name.as_slice());
        if e.ino != 0 {
            (arena.dev[i], arena.ino[i]) = short((e.dev, e.ino));
        }
        if is_dir {
            continue;
        }
        if flags.contains(Flags::HARDLINK_DUP) {
            // The only link left, or the one that counted is gone: this one carries the bytes now.
            let key = links.inode(c).unwrap_or((e.dev, e.ino));
            if e.nlink <= 1 || links.carrier(arena, key).is_none() {
                links.promote(key, c);
                arena.flags[i].remove(Flags::HARDLINK_DUP);
            }
        }
        let size = if arena.flags[i].contains(Flags::HARDLINK_DUP) { 0 } else { e.size };
        if size != arena.size[i] || e.mtime != arena.mtime[i] {
            arena.set_size(c, size, e.mtime);
            out.changed.push(c);
        }
    }

    let added: Vec<&DirEntry> = listing.iter().filter(|e| !kept.contains(e.name.as_slice())).collect();
    let linked = |e: &DirEntry| e.nlink > 1 && e.ino != 0 && !e.flags.intersects(Flags::DIR | Flags::SYMLINK);
    let wanted: HashSet<(u32, u64)> = added
        .iter()
        .filter(|e| linked(e) && links.carrier(arena, (e.dev, e.ino)).is_none())
        .map(|e| short((e.dev, e.ino)))
        .collect();
    let found = find_carriers(arena, &wanted);
    for e in added {
        let key = (e.dev, e.ino);
        let dup = linked(e) && carried(arena, links, &found, key);
        let (size, flags) = if dup { (0, e.flags | Flags::HARDLINK_DUP) } else { (e.size, e.flags) };
        let id = arena.insert(dir, &e.name, size, e.mtime, flags);
        (arena.dev[id as usize], arena.ino[id as usize]) = short(key);
        if linked(e) {
            links.add(e.dev, e.ino, id, !dup);
        }
        if e.flags.contains(Flags::DIR) {
            out.new_dirs.push(id);
        } else {
            out.added.push(id);
        }
    }
    out
}

/// `Arena::graft` for a sub-walk, keeping one carrier per inode: sub links to an inode the scan
/// already carries become dups first, shrinking the sub root's total with them, and the sub
/// registry joins the scan's.
pub fn graft(arena: &mut Arena, links: &mut Hardlinks, sub: &mut Arena, sub_links: Hardlinks, under: NodeId) {
    if sub.is_empty() {
        return;
    }
    // Sub node j lands at base + j.
    let base = arena.len() as NodeId - 1;
    let wanted: HashSet<(u32, u64)> =
        sub_links.members.keys().filter(|&&k| links.carrier(arena, k).is_none()).map(|&k| short(k)).collect();
    let found = find_carriers(arena, &wanted);
    for (key, members) in sub_links.members {
        let held = carried(arena, links, &found, key);
        for (k, &j) in members.iter().enumerate() {
            let s = j as usize;
            if held && !sub.flags[s].contains(Flags::HARDLINK_DUP) {
                sub.set_size(j, 0, sub.mtime[s]);
                sub.flags[s] |= Flags::HARDLINK_DUP;
            }
            links.add(key.0, key.1, base + j, !held && k == 0);
        }
    }
    arena.graft(sub, under);
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
        use std::os::unix::ffi::OsStrExt;
        let want = Path::new(std::ffi::OsStr::from_bytes(arena.name(0))).join(path);
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
                size: if arena.flags[c].contains(Flags::HARDLINK_DUP) { LINK } else { arena.size[c] },
                mtime: arena.mtime[c],
                flags: arena.flags[c] & kind,
                dev: u64::from(arena.dev[c]),
                ino: arena.ino[c],
                nlink: if arena.ino[c] == 0 { 1 } else { 2 },
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

    /// Size of every hard-linked file in the link tests; `listing` reports it for dups.
    const LINK: u64 = 100;
    const LINKED: &[(&str, u64)] =
        &[("a/", 64), ("a/x", LINK), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)];

    /// Give each path an inode on device 1 the way a walker would: the first link of an inode
    /// counts, later ones become dups.
    fn link(arena: &mut Arena, links: &mut Hardlinks, paths: &[(&str, u64)]) {
        for &(path, ino) in paths {
            let id = node(arena, path);
            let i = id as usize;
            (arena.dev[i], arena.ino[i]) = (1, ino);
            let counted = links.counted(1, ino).is_none();
            if !counted {
                arena.set_size(id, 0, arena.mtime[i]);
                arena.flags[i] |= Flags::HARDLINK_DUP;
            }
            links.add(1, ino, id, counted);
        }
    }

    fn linked_setup(paths: &[(&str, u64)]) -> (Arena, ChildIndex, Hardlinks) {
        let mut arena = tree("r", LINKED);
        let mut links = Hardlinks::default();
        link(&mut arena, &mut links, paths);
        let index = ChildIndex::build(&arena);
        (arena, index, links)
    }

    fn link_entry(name: &str, size: u64, ino: u64) -> DirEntry {
        DirEntry { dev: 1, ino, nlink: 2, ..file(name, size) }
    }

    /// The registry's carrier for `ino`, checked against the arena: live, counting, and the
    /// only live member that does.
    fn carrier(arena: &Arena, links: &Hardlinks, ino: u64) -> NodeId {
        let c = links.counted(1, ino).unwrap();
        assert!(!is_gone(arena, c) && !arena.flags[c as usize].contains(Flags::HARDLINK_DUP));
        for &m in &links.members(1, ino)[1..] {
            assert!(is_gone(arena, m) || arena.flags[m as usize].contains(Flags::HARDLINK_DUP));
        }
        c
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
    fn removing_the_counted_link_promotes_the_next() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        let (total, a) = (arena.size[0], node(&arena, "a"));
        let list = without(listing(&arena, &index, a), "x");
        reconcile(&mut arena, &index, &mut links, a, &list);
        assert_matches(&arena, &[("a/", 64), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)]);
        assert_eq!(arena.size[0], total);
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
        assert_eq!(links.members(1, 7).len(), 1);
    }

    #[test]
    fn removing_a_directory_hands_its_links_on() {
        let (mut arena, index, mut links) = linked_setup(&[("a/sub/z", 7), ("b", 7)]);
        let a = node(&arena, "a");
        let list = without(listing(&arena, &index, a), "sub");
        reconcile(&mut arena, &index, &mut links, a, &list);
        assert_matches(&arena, &[("a/", 64), ("a/x", LINK), ("a/y", 200), ("b", LINK)]);
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
    }

    #[test]
    fn a_new_link_to_a_carried_inode_is_a_dup() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7)]);
        let mut list = listing(&arena, &index, 0);
        list.push(link_entry("c", LINK, 7));
        let r = reconcile(&mut arena, &index, &mut links, 0, &list);
        let c = node(&arena, "c");
        assert_eq!(r.added, vec![c]);
        assert!(arena.flags[c as usize].contains(Flags::HARDLINK_DUP));
        assert_matches(&arena, &[LINKED, &[("c", 0)]].concat());
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "a/x"));
        assert_eq!(links.members(1, 7), [node(&arena, "a/x"), c]);
        assert_eq!((arena.dev[c as usize], arena.ino[c as usize]), (1, 7));
    }

    #[test]
    fn a_new_link_to_a_file_scanned_with_one_link_is_found_by_inode() {
        let (mut arena, index, mut links) = linked_setup(&[]);
        let y = node(&arena, "a/y") as usize;
        (arena.dev[y], arena.ino[y]) = (1, 9);
        let mut list = listing(&arena, &index, 0);
        // `c` links to the scanned `a/y`; `d` and `e` are two new links to an inode nobody had.
        list.extend([link_entry("c", 200, 9), link_entry("d", 30, 11), link_entry("e", 30, 11)]);
        reconcile(&mut arena, &index, &mut links, 0, &list);
        assert_matches(&arena, &[LINKED, &[("c", 0), ("d", 30), ("e", 0)]].concat());
        assert_eq!(carrier(&arena, &links, 9), y as NodeId);
        assert_eq!(carrier(&arena, &links, 11), node(&arena, "d"));
        assert_eq!(links.members(1, 9), [y as NodeId, node(&arena, "c")]);
    }

    #[test]
    fn a_dup_left_with_one_link_is_promoted() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        let (total, a, b) = (arena.size[0], node(&arena, "a"), node(&arena, "b"));
        // The root's listing arrives before `a`'s: `b` already shows the link is gone.
        let mut list = listing(&arena, &index, 0);
        list.iter_mut().find(|e| e.name == b"b").unwrap().nlink = 1;
        let r = reconcile(&mut arena, &index, &mut links, 0, &list);
        assert_eq!(r.changed, vec![b]);
        assert_eq!(links.counted(1, 7), Some(b));

        let list = without(listing(&arena, &index, a), "x");
        reconcile(&mut arena, &index, &mut links, a, &list);
        assert_matches(&arena, &[("a/", 64), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)]);
        assert_eq!(arena.size[0], total);
        assert_eq!(carrier(&arena, &links, 7), b);
        assert_eq!(links.members(1, 7), [b]);
    }

    #[test]
    fn a_dup_whose_carrier_was_trashed_is_promoted() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        arena.mark_trashed(node(&arena, "a/x"));
        let list = listing(&arena, &index, 0);
        reconcile(&mut arena, &index, &mut links, 0, &list);
        assert_matches(&arena, &[("a/", 64), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)]);
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
    }

    #[test]
    fn trashing_the_counted_link_promotes_the_survivor() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        let (total, x) = (arena.size[0], node(&arena, "a/x"));
        assert_eq!(detach(&mut arena, &index, &mut links, x, Detached::Trashed), 0);
        assert!(arena.flags[x as usize].contains(Flags::TRASHED));
        assert_eq!(arena.size[0], total);
        assert_matches(&arena, &[("a/", 64), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)]);
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
        assert_eq!(links.members(1, 7), [node(&arena, "b")]);
    }

    #[test]
    fn trashing_a_directory_hands_its_links_to_a_survivor_outside() {
        // `a/x` counts; `a/sub/z` is a dup inside the same tree, so only `b` may take over.
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("a/sub/z", 7), ("b", 7)]);
        let (total, a, b) = (arena.size[0], node(&arena, "a"), node(&arena, "b"));
        let lost = detach(&mut arena, &index, &mut links, a, Detached::Trashed);
        assert_eq!(lost, 64 + 200 + 64);
        assert_eq!(arena.size[0], total - lost);
        assert_matches(&arena, &[("b", LINK)]);
        assert_eq!(carrier(&arena, &links, 7), b);
        assert_eq!(links.members(1, 7), [b]);
    }

    #[test]
    fn detaching_twice_is_a_no_op() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        let (a, x) = (node(&arena, "a"), node(&arena, "a/x"));
        detach(&mut arena, &index, &mut links, a, Detached::Trashed);
        let after = dump(&arena);
        assert_eq!(detach(&mut arena, &index, &mut links, a, Detached::Removed), 0);
        assert_eq!(detach(&mut arena, &index, &mut links, x, Detached::Trashed), 0);
        assert_eq!(dump(&arena), after);
        assert!(!arena.flags[a as usize].contains(Flags::REMOVED));
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
    }

    #[test]
    fn a_counted_link_replaced_by_a_new_file_hands_its_bytes_on() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7), ("b", 7)]);
        let (a, x) = (node(&arena, "a"), node(&arena, "a/x"));
        let mut list = without(listing(&arena, &index, a), "x");
        list.push(DirEntry { nlink: 1, ..link_entry("x", 120, 21) });
        let r = reconcile(&mut arena, &index, &mut links, a, &list);
        let new_x = node(&arena, "a/x");
        assert_eq!(r, Reconciled { removed: vec![x], added: vec![new_x], ..Default::default() });
        assert_matches(
            &arena,
            &[("a/", 64), ("a/x", 120), ("a/y", 200), ("a/sub/", 64), ("a/sub/z", LINK), ("b", LINK)],
        );
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "b"));
        assert_eq!(links.inode(new_x), None);
    }

    #[test]
    fn graft_demotes_links_the_scan_already_carries() {
        let (mut arena, index, mut links) = linked_setup(&[("a/x", 7)]);
        let y = node(&arena, "a/y") as usize;
        (arena.dev[y], arena.ino[y]) = (1, 12);
        let mut list = listing(&arena, &index, 0);
        list.push(dir("d"));
        let d = reconcile(&mut arena, &index, &mut links, 0, &list).new_dirs[0];

        // `e` links to the carried `a/x`, `f` to `a/y` (one link at scan time), `g` and `h` to each other.
        let mut sub = tree("d", &[("e", LINK), ("f", 200), ("g", 40), ("h", 40)]);
        let mut sub_links = Hardlinks::default();
        link(&mut sub, &mut sub_links, &[("e", 7), ("f", 12), ("g", 13), ("h", 13)]);
        graft(&mut arena, &mut links, &mut sub, sub_links, d);

        assert_matches(&arena, &[LINKED, &[("d/", 64), ("d/e", 0), ("d/f", 0), ("d/g", 40), ("d/h", 0)]].concat());
        assert_eq!(carrier(&arena, &links, 7), node(&arena, "a/x"));
        assert_eq!(carrier(&arena, &links, 12), y as NodeId);
        assert_eq!(carrier(&arena, &links, 13), node(&arena, "d/g"));
        assert_eq!(links.members(1, 7), [node(&arena, "a/x"), node(&arena, "d/e")]);
        assert_eq!(links.inode(node(&arena, "d/h")), Some((1, 13)));
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
