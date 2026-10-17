use scan_core::{Arena, NodeId};

const DAY: i64 = 86_400;
const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

fn plural(n: i64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit}")
    } else {
        format!("{n} {unit}s")
    }
}

/// "45 days", "3 months" or "2 years".
pub(crate) fn span(days: i64) -> String {
    if days < 60 {
        plural(days, "day")
    } else if days < 365 {
        plural(days / 30, "month")
    } else {
        plural(days / 365, "year")
    }
}

/// Sizes the way the UI prints them: "512 B", "1.50 GB", "12.3 GB".
pub(crate) fn human_size(bytes: u64) -> String {
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    match i {
        0 => format!("{bytes} B"),
        _ if v < 10.0 => format!("{v:.2} {}", UNITS[i]),
        _ => format!("{v:.1} {}", UNITS[i]),
    }
}

/// Replace `{age}` with how long ago the node was modified and `{size}` with its size.
pub(crate) fn fill(reason: &str, arena: &Arena, id: NodeId, now: i64) -> String {
    let i = id as usize;
    let mut out = reason.to_string();
    if out.contains("{age}") {
        let mtime = arena.mtime[i];
        let age = if mtime > 0 { span((now - mtime).max(0) / DAY) } else { "an unknown time".into() };
        out = out.replace("{age}", &age);
    }
    if out.contains("{size}") {
        out = out.replace("{size}", &human_size(arena.size[i]));
    }
    out
}
