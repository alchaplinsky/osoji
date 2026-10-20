use crate::lookup::{expand, find_node, xdg_data_home};
use crate::schema::{Kind, Rule};
use scan_core::{Arena, ChildIndex, Flags, NodeId, NONE};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::path::PathBuf;

const DAY: i64 = 86_400;

pub struct PolicyContext {
    pub home: PathBuf,
    /// Unix seconds, compared against node mtimes.
    pub now: i64,
    /// What `$XDG_DATA_HOME` in a rule path stands for: the user's XDG data directory.
    pub data_home: PathBuf,
}

impl PolicyContext {
    /// With `data_home` taken from the environment (`xdg_data_home`).
    pub fn new(home: PathBuf, now: i64) -> Self {
        let data_home = xdg_data_home(&home);
        Self { home, now, data_home }
    }

    /// A rule path made absolute: `~` is the home, `$XDG_DATA_HOME` the data directory.
    pub fn expand(&self, path: &str) -> Option<PathBuf> {
        expand(path, &self.home, &self.data_home)
    }
}

/// Which nodes a rule can match, narrowest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    Node(NodeId),
    Children(NodeId),
    All,
}

/// A rule with its paths resolved to nodes and its strings borrowed as bytes.
struct Compiled<'a> {
    rule: usize,
    kind: Kind,
    path: Option<NodeId>,
    parent: Option<NodeId>,
    under: Option<NodeId>,
    name: Option<&'a [u8]>,
    suffix: Option<&'a [u8]>,
    prefix: Option<&'a [u8]>,
    not_named: &'a [String],
    not_at: Vec<NodeId>,
    dir: Option<bool>,
    sibling: Option<&'a [u8]>,
    min_bytes: Option<u64>,
    mtime_before: Option<i64>,
}

impl Compiled<'_> {
    fn scope(&self) -> Scope {
        match (self.path, self.parent) {
            (Some(n), _) => Scope::Node(n),
            (None, Some(p)) => Scope::Children(p),
            (None, None) => Scope::All,
        }
    }
}

/// A policy bound to one scan: path fields are resolved to node ids once.
pub struct Resolved<'a> {
    index: &'a ChildIndex,
    rules: Vec<Compiled<'a>>,
}

/// The strongest rule that applies to each flagged node.
#[derive(Debug, Default)]
pub struct PolicyHits {
    rules: HashMap<NodeId, usize>,
}

impl PolicyHits {
    /// Index of the rule behind a node's TRASH_FOLDER, PROTECTED, RISKY or UNEXPECTED flag.
    pub fn rule(&self, id: NodeId) -> Option<usize> {
        self.rules.get(&id).copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (NodeId, usize)> + '_ {
        self.rules.iter().map(|(&id, &rule)| (id, rule))
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

/// Every flag `stamp` sets, which `stamp_ids` clears before evaluating a node again.
pub const POLICY_FLAGS: Flags =
    Flags::TRASH_FOLDER.union(Flags::PROTECTED).union(Flags::RISKY).union(Flags::UNEXPECTED);

fn flag_of(kind: Kind) -> Flags {
    match kind {
        Kind::Trash => Flags::TRASH_FOLDER | Flags::PROTECTED,
        Kind::Protected => Flags::PROTECTED,
        Kind::Risky => Flags::RISKY,
        Kind::Unexpected => Flags::UNEXPECTED,
        Kind::Cleanup => Flags::empty(),
    }
}

fn severity(kind: Kind) -> u8 {
    match kind {
        Kind::Trash => 4,
        Kind::Protected => 3,
        Kind::Risky => 2,
        Kind::Unexpected => 1,
        Kind::Cleanup => 0,
    }
}

/// A node's verdict so far: the strongest matching rule, its kind, and every matching rule's flags.
type Verdict = (usize, Kind, Flags);

/// Fold one more matching rule into a verdict: trash over protected over risky over unexpected,
/// then the earliest in the file.
fn fold(v: &mut Verdict, rule: usize, kind: Kind) {
    v.2 |= flag_of(kind);
    if (severity(kind), std::cmp::Reverse(rule)) > (severity(v.1), std::cmp::Reverse(v.0)) {
        (v.0, v.1) = (rule, kind);
    }
}

fn resolve_path(
    arena: &Arena,
    index: &ChildIndex,
    ctx: &PolicyContext,
    path: &Option<String>,
) -> Result<Option<NodeId>, ()> {
    let Some(path) = path else {
        return Ok(None);
    };
    let id = ctx.expand(path).map_or(NONE, |p| find_node(arena, index, &p));
    if id == NONE {
        Err(())
    } else {
        Ok(Some(id))
    }
}

/// Like `resolve_path`, but a scan rooted inside `under` is all under it, so the condition drops.
fn resolve_under(
    arena: &Arena,
    index: &ChildIndex,
    ctx: &PolicyContext,
    under: &Option<String>,
) -> Result<Option<NodeId>, ()> {
    let inside = |p: &String| !arena.is_empty() && ctx.expand(p).is_some_and(|p| arena.path(0).starts_with(p));
    match resolve_path(arena, index, ctx, under) {
        Err(()) if under.as_ref().is_some_and(inside) => Ok(None),
        other => other,
    }
}

fn compile<'a>(
    idx: usize,
    rule: &'a Rule,
    arena: &Arena,
    index: &ChildIndex,
    ctx: &PolicyContext,
) -> Option<Compiled<'a>> {
    let m = &rule.matcher;
    let at = |p| resolve_path(arena, index, ctx, p).ok();
    Some(Compiled {
        rule: idx,
        kind: rule.kind,
        path: at(&m.path)?,
        parent: at(&m.parent)?,
        under: resolve_under(arena, index, ctx, &m.under).ok()?,
        name: m.name.as_deref().map(str::as_bytes),
        suffix: m.suffix.as_deref().map(str::as_bytes),
        prefix: m.prefix.as_deref().map(str::as_bytes),
        not_named: &m.not_named,
        not_at: m
            .not_path
            .iter()
            .filter_map(|p| ctx.expand(p))
            .map(|p| find_node(arena, index, &p))
            .filter(|&id| id != NONE)
            .collect(),
        dir: m.dir,
        sibling: m.sibling.as_deref().map(str::as_bytes),
        min_bytes: m.min_bytes,
        mtime_before: m.older_than_days.map(|d| ctx.now - i64::from(d) * DAY),
    })
}

/// True when `ancestor` is a proper ancestor of `id`. Ids only grow away from the root, so the walk
/// stops as soon as it passes below `ancestor`.
fn descends_from(arena: &Arena, id: NodeId, ancestor: NodeId) -> bool {
    let mut p = arena.parent[id as usize];
    while p != NONE && p >= ancestor {
        if p == ancestor {
            return true;
        }
        p = arena.parent[p as usize];
    }
    false
}

impl<'a> Resolved<'a> {
    pub(crate) fn new(rules: &'a [Rule], arena: &Arena, index: &'a ChildIndex, ctx: &PolicyContext) -> Self {
        let rules = rules.iter().enumerate().filter_map(|(i, r)| compile(i, r, arena, index, ctx)).collect();
        Self { index, rules }
    }

    fn matches(&self, r: &Compiled, arena: &Arena, id: NodeId) -> bool {
        let i = id as usize;
        if r.dir.is_some_and(|d| arena.flags[i].contains(Flags::DIR) != d) {
            return false;
        }
        let name = arena.name(id);
        if r.name.is_some_and(|n| n != name) || r.suffix.is_some_and(|s| !name.ends_with(s)) {
            return false;
        }
        if r.prefix.is_some_and(|p| !name.starts_with(p)) {
            return false;
        }
        if r.not_named.iter().any(|n| n.as_bytes() == name) || r.not_at.contains(&id) {
            return false;
        }
        if r.min_bytes.is_some_and(|m| arena.size[i] < m) {
            return false;
        }
        if let Some(before) = r.mtime_before {
            let mtime = arena.mtime[i];
            if mtime <= 0 || mtime > before {
                return false;
            }
        }
        let parent = arena.parent[i];
        if r.path.is_some_and(|p| p != id) || r.parent.is_some_and(|p| p != parent) {
            return false;
        }
        if r.under.is_some_and(|u| !descends_from(arena, id, u)) {
            return false;
        }
        match r.sibling {
            None => true,
            Some(s) => parent != NONE && self.index.children(parent).iter().any(|&c| c != id && arena.name(c) == s),
        }
    }

    /// Call `hit(node, rule)` for every match of the rules `want` selects. Rules pinned to a path or a
    /// parent test only those nodes; the rest share one pass over the `sweep` ids.
    fn each_match(
        &self,
        arena: &Arena,
        sweep: Range<NodeId>,
        want: impl Fn(Kind) -> bool,
        mut hit: impl FnMut(NodeId, usize, Kind),
    ) {
        let mut swept = Vec::new();
        for r in self.rules.iter().filter(|r| want(r.kind)) {
            match r.scope() {
                Scope::Node(n) => {
                    if self.matches(r, arena, n) {
                        hit(n, r.rule, r.kind);
                    }
                }
                Scope::Children(p) => {
                    for &c in self.index.children(p) {
                        if self.matches(r, arena, c) {
                            hit(c, r.rule, r.kind);
                        }
                    }
                }
                Scope::All => swept.push(r),
            }
        }
        if swept.is_empty() {
            return;
        }
        for id in sweep {
            for r in &swept {
                if self.matches(r, arena, id) {
                    hit(id, r.rule, r.kind);
                }
            }
        }
    }

    /// Set TRASH_FOLDER, PROTECTED, RISKY and UNEXPECTED on every matching node and record, per node, the
    /// strongest rule (trash over protected over risky over unexpected, then the earliest in the file).
    /// A trash rule also sets PROTECTED: the Trash itself is never swept.
    pub fn stamp(&self, arena: &mut Arena) -> PolicyHits {
        let mut found: HashMap<NodeId, Verdict> = HashMap::new();
        self.each_match(
            arena,
            0..arena.len() as NodeId,
            |k| k != Kind::Cleanup,
            |id, rule, kind| fold(found.entry(id).or_insert((rule, kind, Flags::empty())), rule, kind),
        );
        let mut hits = PolicyHits { rules: HashMap::with_capacity(found.len()) };
        for (id, (rule, _, flags)) in found {
            arena.flags[id as usize] |= flags;
            hits.rules.insert(id, rule);
        }
        hits
    }

    /// `stamp` again for nodes a live update touched, added or recreated: size and mtime rules
    /// may now hold or no longer hold, so each node loses its policy flags and hit and is tested
    /// against every rule. Resolve against an index that holds new nodes, so a recreated pinned
    /// path resolves to its new id.
    pub fn stamp_ids(&self, arena: &mut Arena, ids: impl IntoIterator<Item = NodeId>, hits: &mut PolicyHits) {
        for id in ids {
            arena.flags[id as usize].remove(POLICY_FLAGS);
            hits.rules.remove(&id);
            let mut verdict: Option<Verdict> = None;
            for r in self.rules.iter().filter(|r| r.kind != Kind::Cleanup && self.matches(r, arena, id)) {
                fold(verdict.get_or_insert((r.rule, r.kind, Flags::empty())), r.rule, r.kind);
            }
            if let Some((rule, _, flags)) = verdict {
                arena.flags[id as usize] |= flags;
                hits.rules.insert(id, rule);
            }
        }
    }

    /// Every node a cleanup rule matches, with the first matching rule, ordered by node id.
    /// Nested matches are all reported; callers decide whether a match covers its descendants.
    pub fn cleanup_matches(&self, arena: &Arena) -> Vec<(NodeId, usize)> {
        let mut found: HashMap<NodeId, usize> = HashMap::new();
        self.each_match(
            arena,
            0..arena.len() as NodeId,
            |k| k == Kind::Cleanup,
            |id, rule, _| {
                if !arena.flags[id as usize].contains(Flags::TRASHED) {
                    let best = found.entry(id).or_insert(rule);
                    *best = (*best).min(rule);
                }
            },
        );
        let mut out: Vec<_> = found.into_iter().collect();
        out.sort_unstable();
        out
    }
}

/// Drop every match that sits inside another match or inside a node `opaque` holds closed, so a
/// suggested folder stands for everything in it.
pub fn covering(arena: &Arena, matches: Vec<(NodeId, usize)>, opaque: impl Fn(NodeId) -> bool) -> Vec<(NodeId, usize)> {
    let matched: HashSet<NodeId> = matches.iter().map(|&(id, _)| id).collect();
    matches
        .into_iter()
        .filter(|&(id, _)| {
            let mut p = arena.parent[id as usize];
            while p != NONE {
                if matched.contains(&p) || opaque(p) {
                    return false;
                }
                p = arena.parent[p as usize];
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{ctx, fixture};
    use crate::Policy;

    #[test]
    fn pins_path_and_parent_rules_to_their_nodes() {
        let f = fixture();
        let src = r#"
            version = 1
            [[rule]]
            id = "direct"
            kind = "protected"
            reason = "x"
            match = { path = "/System" }
            [[rule]]
            id = "children"
            kind = "risky"
            reason = "x"
            match = { parent = "~", dir = true }
            [[rule]]
            id = "anywhere"
            kind = "risky"
            reason = "x"
            match = { suffix = ".app" }
        "#;
        let policy = Policy::from_toml(src).unwrap();
        let resolved = policy.resolve(&f.arena, &f.index, &ctx());
        let scopes: Vec<Scope> = resolved.rules.iter().map(Compiled::scope).collect();
        assert_eq!(scopes, [Scope::Node(f.id("/System")), Scope::Children(f.id("/Users/alex")), Scope::All]);
    }
}
