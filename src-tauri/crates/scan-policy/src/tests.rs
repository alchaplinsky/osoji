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
    assert!(macos.rules().iter().all(|r| matches!(r.kind, Kind::Protected | Kind::Risky)));
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
fn matches_each_field() {
    assert_eq!(matched("path = \"~/proj\""), ["/Users/alex/proj"]);
    assert_eq!(matched("parent = \"~/proj\""), ["/Users/alex/proj/Cargo.toml", "/Users/alex/proj/target"]);
    assert_eq!(matched("under = \"/Applications\", dir = true"), [
        "/Applications/Safari.app",
        "/Applications/Utilities",
        "/Applications/Utilities/Terminal.app"
    ]);
    assert_eq!(matched("name = \".Trash\""), ["/Users/alex/.Trash", "/Users/alex/Documents/.Trash"]);
    assert_eq!(matched("suffix = \".app\""), [
        "/Applications/Safari.app",
        "/Applications/Utilities/Terminal.app",
        "/Applications/readme.app"
    ]);
    assert_eq!(matched("suffix = \".app\", dir = false"), ["/Applications/readme.app"]);
    assert_eq!(matched("parent = \"/Applications\", not_named = [\"Utilities\", \"readme.app\"]"), ["/Applications/Safari.app"]);
    assert_eq!(matched("name = \"target\", sibling = \"Cargo.toml\""), ["/Users/alex/proj/target"]);
    assert_eq!(matched("parent = \"~/Documents\", min_bytes = 900"), [
        "/Users/alex/Documents/new.bin",
        "/Users/alex/Documents/old.bin",
        "/Users/alex/Documents/undated.bin"
    ]);
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

    for p in ["/System", "/Applications", "/Users", HOME, "/Users/alex/Library", "/Users/alex/Library/Application Support", "/Users/alex/.Trash"] {
        assert!(flags(p).contains(Flags::PROTECTED), "{p}");
        assert!(!flags(p).contains(Flags::RISKY), "{p}");
    }
    assert_eq!(rule_id("/Users/alex/.Trash"), Some("trash"));
    assert!(!flags("/Users/alex/Documents/.Trash").intersects(Flags::PROTECTED | Flags::RISKY));

    for p in ["/Applications/Safari.app", "/Applications/Utilities/Terminal.app", "/Users/alex/Photos.photoslibrary"] {
        assert!(flags(p).contains(Flags::RISKY), "{p}");
    }
    assert_eq!(rule_id("/Users/alex/Library/Application Support/Slack"), Some("app-data"));
    assert!(!flags("/Applications/readme.app").intersects(Flags::PROTECTED | Flags::RISKY));
    assert_eq!(hits.rule(f.id("/Users/alex/proj")), None);

    let docs = flags("/Users/alex/Documents");
    assert!(docs.contains(Flags::PROTECTED | Flags::RISKY));
    assert_eq!(rule_id("/Users/alex/Documents"), Some("docs-protected"));
    assert!(policy.rules().iter().all(|r| r.kind != Kind::Unexpected));
    assert_eq!(f.arena.count_flag(Flags::UNEXPECTED), 0);
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
