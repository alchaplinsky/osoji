use super::*;
use scan_core::{Flags, NodeId, NONE};
use std::path::{Path, PathBuf};

const NOW: i64 = 1_700_000_000;
const DAY: i64 = 86_400;
const HOME: &str = "/Users/alex";

pub(crate) struct Fixture {
    pub arena: Arena,
    pub index: ChildIndex,
    ids: Vec<(&'static str, NodeId)>,
}

impl Fixture {
    pub fn id(&self, path: &str) -> NodeId {
        self.ids.iter().find(|(p, _)| *p == path).unwrap_or_else(|| panic!("no node {path}")).1
    }
}

/// `(path, size, age in days, dir)`, parents before children.
const NODES: &[(&str, u64, i64, bool)] = &[
    ("/System", 0, 1, true),
    ("/Applications", 0, 1, true),
    ("/Applications/Safari.app", 50, 1, true),
    ("/Applications/Utilities", 0, 1, true),
    ("/Applications/Utilities/Terminal.app", 20, 1, true),
    ("/Applications/readme.app", 1, 1, false),
    ("/Users", 0, 1, true),
    ("/Users/alex", 0, 1, true),
    ("/Users/alex/.Trash", 5, 1, true),
    ("/Users/alex/Library", 0, 1, true),
    ("/Users/alex/Library/Application Support", 0, 1, true),
    ("/Users/alex/Library/Application Support/Slack", 30, 1, true),
    ("/Users/alex/Documents", 0, 1, true),
    ("/Users/alex/Documents/.Trash", 1, 1, true),
    ("/Users/alex/Documents/old.bin", 900, 400, false),
    ("/Users/alex/Documents/new.bin", 900, 2, false),
    ("/Users/alex/Documents/undated.bin", 900, 0, false),
    ("/Users/alex/proj", 0, 1, true),
    ("/Users/alex/proj/Cargo.toml", 1, 1, false),
    ("/Users/alex/proj/target", 400, 1, true),
    ("/Users/alex/other", 0, 1, true),
    ("/Users/alex/other/target", 400, 1, true),
    ("/Users/alex/Photos.photoslibrary", 700, 1, true),
];

pub(crate) fn fixture() -> Fixture {
    let mut arena = Arena::default();
    let root = arena.push(NONE, b"/", 0, NOW, Flags::DIR);
    let mut ids = vec![("/", root)];
    for &(path, size, age, dir) in NODES {
        let (parent, name) = path.rsplit_once('/').unwrap();
        let parent = if parent.is_empty() { root } else { ids.iter().find(|(p, _)| *p == parent).unwrap().1 };
        let mtime = if age == 0 { 0 } else { NOW - age * DAY };
        let flags = if dir { Flags::DIR } else { Flags::empty() };
        ids.push((path, arena.push(parent, name.as_bytes(), size, mtime, flags)));
    }
    arena.aggregate();
    let index = ChildIndex::build(&arena);
    Fixture { arena, index, ids }
}

pub(crate) fn ctx() -> PolicyContext {
    PolicyContext { home: PathBuf::from(HOME), now: NOW }
}

fn rule_toml(kind: &str, matcher: &str) -> String {
    format!("version = 1\n[[rule]]\nid = \"t\"\nkind = \"{kind}\"\nreason = \"Because.\"\nmatch = {{ {matcher} }}\n")
}

/// Paths a single cleanup rule with this `match` selects in the fixture.
fn matched(matcher: &str) -> Vec<String> {
    let f = fixture();
    let policy = Policy::from_toml(&rule_toml("cleanup", matcher)).unwrap();
    let resolved = policy.resolve(&f.arena, &f.index, &ctx());
    let mut paths: Vec<String> = resolved
        .cleanup_matches(&f.arena)
        .into_iter()
        .map(|(id, _)| f.arena.path(id).to_string_lossy().into_owned())
        .collect();
    paths.sort();
    paths
}

fn rule_error(src: &str) -> (String, String) {
    match Policy::from_toml(src) {
        Err(PolicyError::Rule { id, message }) => (id, message),
        other => panic!("expected a rule error, got {other:?}"),
    }
}

#[test]
fn bundled_policies_load() {
    let macos = Policy::from_toml(MACOS).unwrap();
    assert_eq!(macos.rules().iter().filter(|r| r.kind == Kind::Trash).count(), 1);
    assert!(macos.rules().iter().all(|r| (r.kind == Kind::Cleanup) == r.safety.is_some()));
    assert!(macos.rules().iter().any(|r| r.kind == Kind::Unexpected));
    assert!(macos.rules().iter().any(|r| r.id == "home"));
    assert!(Policy::from_toml(LINUX).unwrap().rules().is_empty());
    assert!(Policy::from_toml(WINDOWS).unwrap().rules().is_empty());
    assert!(Policy::for_current_os().is_ok());
}

#[test]
fn rejects_unknown_fields_with_the_rule_id() {
    let (id, message) = rule_error(&rule_toml("protected", "pth = \"/System\""));
    assert_eq!(id, "t");
    assert!(message.contains("pth"), "{message}");

    let src = "version = 1\n[[rule]]\nid = \"typo\"\nkind = \"risky\"\nreasn = \"x\"\nmatch = { name = \"a\" }\n";
    let (id, message) = rule_error(src);
    assert_eq!(id, "typo");
    assert!(message.contains("reasn"), "{message}");

    assert!(matches!(Policy::from_toml("version = 1\nrules = []\n"), Err(PolicyError::Syntax(_))));
}

#[test]
fn rejects_duplicate_ids() {
    let one = rule_toml("risky", "name = \"a\"");
    let two = one.replacen("version = 1\n", "", 1);
    assert_eq!(Policy::from_toml(&(one + &two)).unwrap_err(), PolicyError::DuplicateId("t".into()));
}

#[test]
fn rejects_safety_outside_cleanup() {
    let src = rule_toml("protected", "name = \"a\"").replace("reason =", "safety = \"safe\"\nreason =");
    let (id, message) = rule_error(&src);
    assert_eq!(id, "t");
    assert!(message.contains("safety"), "{message}");

    let ok = rule_toml("cleanup", "name = \"a\"").replace("reason =", "safety = \"safe\"\nreason =");
    assert_eq!(Policy::from_toml(&ok).unwrap().rule(0).safety, Some(Safety::Safe));
}

#[test]
fn rejects_empty_match_relative_paths_and_bad_values() {
    let (_, message) = rule_error(&rule_toml("risky", ""));
    assert!(message.contains("match"), "{message}");
    let (_, message) = rule_error(&rule_toml("risky", "under = \"Library\""));
    assert!(message.contains("under"), "{message}");
    let (_, message) = rule_error(&rule_toml("loud", "name = \"a\""));
    assert!(message.contains("loud"), "{message}");
    assert_eq!(Policy::from_toml("version = 2\n").unwrap_err(), PolicyError::Version(2));
}

#[test]
fn expands_tilde_to_home() {
    let home = Path::new(HOME);
    assert_eq!(expand("~", home), Some(PathBuf::from(HOME)));
    assert_eq!(expand("~/Library/Caches", home), Some(PathBuf::from("/Users/alex/Library/Caches")));
    assert_eq!(expand("/System", home), Some(PathBuf::from("/System")));
    assert_eq!(expand("~/Library", Path::new("")), None);
}

#[test]
fn finds_nodes_by_path() {
    let f = fixture();
    assert_eq!(find_node(&f.arena, &f.index, Path::new("/")), 0);
    assert_eq!(find_node(&f.arena, &f.index, Path::new("/Users/alex/proj/target")), f.id("/Users/alex/proj/target"));
    assert_eq!(find_node(&f.arena, &f.index, Path::new("/Users/bob")), NONE);
}

#[test]
fn finds_the_live_node_when_a_name_came_back() {
    let mut f = fixture();
    let old = f.id("/Users/alex/proj");
    f.arena.mark_removed(old);
    let new = f.arena.insert(f.id(HOME), b"proj", 0, NOW, Flags::DIR);
    let index = ChildIndex::build(&f.arena);
    assert_eq!(find_node(&f.arena, &index, Path::new("/Users/alex/proj")), new);
    assert_eq!(find_node(&f.arena, &index, Path::new("/Users/alex/proj/target")), NONE);
}

#[test]
fn matches_each_field() {
    assert_eq!(matched("path = \"~/proj\""), ["/Users/alex/proj"]);
    assert_eq!(matched("parent = \"~/proj\""), ["/Users/alex/proj/Cargo.toml", "/Users/alex/proj/target"]);
    assert_eq!(
        matched("under = \"/Applications\", dir = true"),
        ["/Applications/Safari.app", "/Applications/Utilities", "/Applications/Utilities/Terminal.app"]
    );
    assert_eq!(matched("name = \".Trash\""), ["/Users/alex/.Trash", "/Users/alex/Documents/.Trash"]);
    assert_eq!(
        matched("suffix = \".app\""),
        ["/Applications/Safari.app", "/Applications/Utilities/Terminal.app", "/Applications/readme.app"]
    );
    assert_eq!(matched("suffix = \".app\", dir = false"), ["/Applications/readme.app"]);
    assert_eq!(
        matched("parent = \"/Applications\", not_named = [\"Utilities\", \"readme.app\"]"),
        ["/Applications/Safari.app"]
    );
    assert_eq!(matched("prefix = \".\""), ["/Users/alex/.Trash", "/Users/alex/Documents/.Trash"]);
    assert_eq!(matched("parent = \"/Users\", not_path = [\"~\", \"/Users/missing\"]"), Vec::<String>::new());
    assert_eq!(
        matched("parent = \"~\", dir = true, not_path = [\"~/Library\", \"~/Documents\", \"~/proj\", \"~/other\"]"),
        ["/Users/alex/.Trash", "/Users/alex/Photos.photoslibrary"]
    );
    assert_eq!(matched("name = \"target\", sibling = \"Cargo.toml\""), ["/Users/alex/proj/target"]);
    assert_eq!(
        matched("parent = \"~/Documents\", min_bytes = 900"),
        ["/Users/alex/Documents/new.bin", "/Users/alex/Documents/old.bin", "/Users/alex/Documents/undated.bin"]
    );
    assert_eq!(matched("parent = \"~/Documents\", older_than_days = 180"), ["/Users/alex/Documents/old.bin"]);
}

#[test]
fn drops_rules_whose_paths_are_not_in_the_scan() {
    assert!(matched("under = \"/opt\"").is_empty());
    assert!(matched("path = \"~/Missing\"").is_empty());
}

#[test]
fn stamps_flags_and_records_the_strongest_rule() {
    let mut f = fixture();
    let mut src = MACOS.to_string();
    src.push_str("\n[[rule]]\nid = \"docs-protected\"\nkind = \"protected\"\nreason = \"Test.\"\nmatch = { path = \"~/Documents\" }\n");
    let policy = Policy::from_toml(&src).unwrap();
    let hits = policy.resolve(&f.arena, &f.index, &ctx()).stamp(&mut f.arena);

    let flags = |p: &str| f.arena.flags[f.id(p) as usize];
    let rule_id = |p: &str| hits.rule(f.id(p)).map(|i| policy.rule(i).id.as_str());

    for p in [
        "/System",
        "/Applications",
        "/Users",
        HOME,
        "/Users/alex/Library",
        "/Users/alex/Library/Application Support",
        "/Users/alex/.Trash",
    ] {
        assert!(flags(p).contains(Flags::PROTECTED), "{p}");
        assert!(!flags(p).contains(Flags::RISKY), "{p}");
    }
    assert_eq!(rule_id("/Users/alex/.Trash"), Some("trash"));
    assert!(flags("/Users/alex/.Trash").contains(Flags::TRASH_FOLDER));
    assert_eq!(f.arena.count_flag(Flags::TRASH_FOLDER), 1);
    assert!(!flags("/Users/alex/Documents/.Trash").intersects(Flags::PROTECTED | Flags::RISKY | Flags::TRASH_FOLDER));

    for p in ["/Applications/Safari.app", "/Applications/Utilities/Terminal.app", "/Users/alex/Photos.photoslibrary"] {
        assert!(flags(p).contains(Flags::RISKY), "{p}");
    }
    assert_eq!(rule_id("/Users/alex/Library/Application Support/Slack"), Some("app-data"));
    assert!(!flags("/Applications/readme.app").intersects(Flags::PROTECTED | Flags::RISKY));
    assert_eq!(hits.rule(f.id("/Users/alex/proj")), None);

    let docs = flags("/Users/alex/Documents");
    assert!(docs.contains(Flags::PROTECTED | Flags::RISKY));
    assert_eq!(rule_id("/Users/alex/Documents"), Some("docs-protected"));
    assert_eq!(f.arena.count_flag(Flags::UNEXPECTED), 0);
}

#[test]
fn stamp_ids_follows_sizes_across_min_bytes() {
    let mut f = fixture();
    let policy = Policy::from_toml(&rule_toml("unexpected", "under = \"~\", dir = true, min_bytes = 1000")).unwrap();
    let resolved = policy.resolve(&f.arena, &f.index, &ctx());
    let mut hits = resolved.stamp(&mut f.arena);
    let (docs, photos) = (f.id("/Users/alex/Documents"), f.id("/Users/alex/Photos.photoslibrary"));
    assert_eq!(hits.rule(docs), Some(0));
    assert_eq!(hits.rule(photos), None);

    f.arena.set_size(photos, 1500, NOW);
    resolved.stamp_ids(&mut f.arena, [photos], &mut hits);
    assert_eq!(f.arena.flags[photos as usize], Flags::DIR | Flags::UNEXPECTED);
    assert_eq!(hits.rule(photos), Some(0));

    let before = (f.arena.flags.clone(), hits.len());
    resolved.stamp_ids(&mut f.arena, [photos, photos], &mut hits);
    assert_eq!((f.arena.flags.clone(), hits.len()), before);

    for p in ["/Users/alex/Documents/new.bin", "/Users/alex/Documents/undated.bin"] {
        f.arena.set_size(f.id(p), 0, NOW);
    }
    resolved.stamp_ids(&mut f.arena, [docs], &mut hits);
    assert_eq!(f.arena.flags[docs as usize], Flags::DIR);
    assert_eq!(hits.rule(docs), None);

    // `other` grows past the threshold with its child, but only ids passed in are evaluated.
    let (other, target) = (f.id("/Users/alex/other"), f.id("/Users/alex/other/target"));
    f.arena.set_size(target, 1500, NOW);
    resolved.stamp_ids(&mut f.arena, [target], &mut hits);
    assert!(f.arena.flags[target as usize].contains(Flags::UNEXPECTED));
    assert_eq!(f.arena.flags[other as usize], Flags::DIR);
    assert_eq!(hits.rule(other), None);
}

#[test]
fn stamp_ids_protects_a_recreated_path() {
    let mut f = fixture();
    let policy = Policy::from_toml(&rule_toml("protected", "path = \"/System\"")).unwrap();
    let mut hits = policy.resolve(&f.arena, &f.index, &ctx()).stamp(&mut f.arena);

    let old = f.id("/System");
    f.arena.mark_removed(old);
    let new = f.arena.insert(0, b"System", 0, NOW, Flags::DIR);
    let index = ChildIndex::build(&f.arena);
    policy.resolve(&f.arena, &index, &ctx()).stamp_ids(&mut f.arena, [old, new], &mut hits);

    assert_eq!(f.arena.flags[new as usize], Flags::DIR | Flags::PROTECTED);
    assert_eq!(hits.rule(new), Some(0));
    assert_eq!(f.arena.flags[old as usize], Flags::DIR | Flags::REMOVED);
    assert_eq!(hits.rule(old), None);
}

#[test]
fn trash_rules_protect_the_folder_and_win_over_protected() {
    let mut f = fixture();
    let src = r#"
        version = 1
        [[rule]]
        id = "keep"
        kind = "protected"
        reason = "Keep it."
        match = { name = ".Trash" }
        [[rule]]
        id = "bin"
        kind = "trash"
        reason = "In the Trash."
        match = { parent = "~", name = ".Trash", dir = true }
    "#;
    let policy = Policy::from_toml(src).unwrap();
    let resolved = policy.resolve(&f.arena, &f.index, &ctx());
    let mut hits = resolved.stamp(&mut f.arena);
    let (trash, nested) = (f.id("/Users/alex/.Trash"), f.id("/Users/alex/Documents/.Trash"));

    // `stamp_ids` picks the same rule per node as the whole-arena pass.
    for pass in 0..2 {
        if pass == 1 {
            resolved.stamp_ids(&mut f.arena, [trash, nested], &mut hits);
        }
        assert_eq!(f.arena.flags[trash as usize], Flags::DIR | Flags::TRASH_FOLDER | Flags::PROTECTED);
        assert_eq!(policy.rule(hits.rule(trash).unwrap()).kind.as_str(), "trash");
        assert_eq!(f.arena.flags[nested as usize], Flags::DIR | Flags::PROTECTED);
        assert_eq!(policy.rule(hits.rule(nested).unwrap()).id, "keep");
    }
}

#[test]
fn skips_home_rules_without_a_home() {
    let mut f = fixture();
    let policy = Policy::from_toml(MACOS).unwrap();
    let ctx = PolicyContext { home: PathBuf::new(), now: NOW };
    policy.resolve(&f.arena, &f.index, &ctx).stamp(&mut f.arena);
    assert!(f.arena.flags[f.id("/System") as usize].contains(Flags::PROTECTED));
    assert!(!f.arena.flags[f.id(HOME) as usize].contains(Flags::PROTECTED));
}

#[test]
fn fills_age_and_size_into_reasons() {
    let f = fixture();
    let src = rule_toml("cleanup", "name = \"a\"").replace("Because.", "{size}, untouched for {age}.");
    let policy = Policy::from_toml(&src).unwrap();
    let reason = |p: &str| policy.reason_for(0, &f.arena, f.id(p), &ctx());
    assert_eq!(reason("/Users/alex/Documents/old.bin"), "900 B, untouched for 1 year.");
    assert_eq!(reason("/Users/alex/Documents/undated.bin"), "900 B, untouched for an unknown time.");
    assert_eq!(reason("/Users/alex/Documents"), "2.70 KB, untouched for 1 day.");
    assert_eq!(reason::span(59), "59 days");
    assert_eq!(reason::span(200), "6 months");
    assert_eq!(reason::span(800), "2 years");
    assert_eq!(reason::human_size(15_000_000_000), "15.0 GB");
    assert_eq!(reason::human_size(994_662_584_320), "994.7 GB");
}

#[test]
fn keeps_only_the_outermost_cleanup_matches() {
    let f = fixture();
    let src = rule_toml("cleanup", "parent = \"~\", not_named = [\"Library\"]")
        + "[[rule]]\nid = \"u\"\nkind = \"cleanup\"\nreason = \"x\"\nmatch = { name = \"target\" }\n"
        + "[[rule]]\nid = \"v\"\nkind = \"cleanup\"\nreason = \"x\"\nmatch = { under = \"/Applications\" }\n";
    let policy = Policy::from_toml(&src).unwrap();
    let all = policy.resolve(&f.arena, &f.index, &ctx()).cleanup_matches(&f.arena);
    let paths = |m: Vec<(NodeId, usize)>| {
        let mut p: Vec<String> = m.into_iter().map(|(id, _)| f.arena.path(id).to_string_lossy().into_owned()).collect();
        p.sort();
        p
    };
    let utilities = f.id("/Applications/Utilities");
    assert_eq!(
        paths(covering(&f.arena, all, |id| id == utilities)),
        [
            "/Applications/Safari.app",
            "/Applications/Utilities",
            "/Applications/readme.app",
            "/Users/alex/.Trash",
            "/Users/alex/Documents",
            "/Users/alex/Photos.photoslibrary",
            "/Users/alex/other",
            "/Users/alex/proj",
        ]
    );

    let only_targets = policy
        .resolve(&f.arena, &f.index, &ctx())
        .cleanup_matches(&f.arena)
        .into_iter()
        .filter(|&(_, r)| r == 1)
        .collect();
    let proj = f.id("/Users/alex/proj");
    assert_eq!(paths(covering(&f.arena, only_targets, |id| id == proj)), ["/Users/alex/other/target"]);
}

#[test]
fn under_holds_for_every_node_when_the_scan_is_rooted_inside_it() {
    let mut arena = Arena::default();
    let root = arena.push(NONE, b"/Users/alex/proj", 0, NOW, Flags::DIR);
    arena.push(root, b".gradle", 1, NOW, Flags::DIR);
    arena.aggregate();
    let index = ChildIndex::build(&arena);
    let policy = Policy::from_toml(&rule_toml("cleanup", "under = \"~\", name = \".gradle\"")).unwrap();
    assert_eq!(policy.resolve(&arena, &index, &ctx()).cleanup_matches(&arena), [(1, 0)]);
    let policy = Policy::from_toml(&rule_toml("cleanup", "under = \"/Applications\", name = \".gradle\"")).unwrap();
    assert!(policy.resolve(&arena, &index, &ctx()).cleanup_matches(&arena).is_empty());
}

/// `cargo test -p scan-policy --release -- --ignored` to time the pass at 10M nodes.
#[test]
#[ignore]
fn stamps_ten_million_nodes_under_a_second() {
    const COUNT: u32 = 10_000_000;
    const FANOUT: u32 = 16;
    let mut arena = Arena::with_capacity(COUNT as usize, COUNT as usize * 16);
    arena.push(NONE, b"/", 0, NOW, Flags::DIR);
    for i in 1..COUNT {
        let name = format!("entry{i}.dat");
        arena.push((i - 1) / FANOUT, name.as_bytes(), 4096, NOW, Flags::DIR);
    }
    let index = ChildIndex::build(&arena);
    let policy = Policy::from_toml(MACOS).unwrap();
    let start = std::time::Instant::now();
    policy.resolve(&arena, &index, &ctx()).stamp(&mut arena);
    let took = start.elapsed();
    println!("stamped {COUNT} nodes in {took:?}");
    assert!(took.as_secs_f64() < 1.0, "{took:?}");
}
