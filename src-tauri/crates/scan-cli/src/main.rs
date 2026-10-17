use clap::{Parser, ValueEnum};
use scan_core::{Arena, Flags, Progress, ScanConfig};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

#[derive(Copy, Clone, Debug, Default, ValueEnum)]
enum Walker {
    /// getattrlistbulk with a work-stealing deque; macOS only
    #[cfg(target_os = "macos")]
    #[cfg_attr(target_os = "macos", default)]
    Bulk,
    /// jwalk with per-entry lstat; the portable reference
    #[cfg_attr(not(target_os = "macos"), default)]
    Jwalk,
}

#[derive(Parser, Debug)]
#[command(about = "Headless disk scanner for benchmarks")]
struct Args {
    /// Directory to scan
    root: PathBuf,
    #[arg(long, value_enum, default_value_t = Walker::default())]
    walker: Walker,
    /// Worker threads (default: min(cores, 8))
    #[arg(long)]
    threads: Option<usize>,
    /// Descend into other mounted filesystems
    #[arg(long)]
    cross_mounts: bool,
    /// Leave worker threads at default QoS (default: user-initiated, P-cores)
    #[arg(long)]
    no_qos: bool,
    /// Print the N largest children of the root
    #[arg(long, default_value_t = 12)]
    top: usize,
    /// Emit one JSON line of metrics and nothing else
    #[arg(long)]
    json: bool,
}

fn max_rss_mb() -> f64 {
    unsafe {
        let mut ru: libc::rusage = std::mem::zeroed();
        libc::getrusage(libc::RUSAGE_SELF, &mut ru);
        // ru_maxrss is bytes on macOS and kilobytes on Linux.
        let bytes = if cfg!(target_os = "linux") { ru.ru_maxrss * 1024 } else { ru.ru_maxrss };
        bytes as f64 / (1024.0 * 1024.0)
    }
}

fn human(bytes: u64) -> String {
    const U: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.2} {}", U[i])
    }
}

fn print_top(arena: &Arena, top: usize) {
    let mut kids: Vec<u32> = arena.children(0).collect();
    kids.sort_by_key(|&k| std::cmp::Reverse(arena.size[k as usize]));
    println!("\n{:>12}  {:>9}  {:<5}  name", "size", "items", "flags");
    for k in kids.iter().take(top) {
        let i = *k as usize;
        let f = arena.flags[i];
        let tag = format!(
            "{}{}{}{}",
            if f.contains(Flags::DIR) { "d" } else { "-" },
            if f.contains(Flags::SYMLINK) { "l" } else { "-" },
            if f.contains(Flags::MOUNT) { "m" } else { "-" },
            if f.contains(Flags::ERROR) { "!" } else { "-" },
        );
        println!(
            "{:>12}  {:>9}  {:<5}  {}",
            human(arena.size[i]),
            arena.items[i],
            tag,
            String::from_utf8_lossy(arena.name(*k))
        );
    }
}

fn main() {
    let args = Args::parse();
    let threads =
        args.threads.unwrap_or_else(|| std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8));
    let cfg = ScanConfig { root: args.root.clone(), threads, cross_mounts: args.cross_mounts, qos: !args.no_qos };

    #[cfg(target_os = "macos")]
    scan_macos::disable_dataless_materialization();
    scan_core::raise_fd_limit();

    let progress = Progress::default();
    let cancel = AtomicBool::new(false);
    let t0 = Instant::now();
    let result = match args.walker {
        #[cfg(target_os = "macos")]
        Walker::Bulk => scan_macos::walk(&cfg, &progress, &cancel),
        Walker::Jwalk => scan_core::jwalk_walker::walk(&cfg, &progress, &cancel),
    };
    let elapsed = t0.elapsed();
    let arena = match result {
        Ok(a) => a,
        Err(e) => {
            eprintln!("scan failed: {e}");
            std::process::exit(1);
        }
    };

    let files = progress.files.load(Ordering::Relaxed);
    let dirs = progress.dirs.load(Ordering::Relaxed);
    let bytes = progress.bytes.load(Ordering::Relaxed);
    let errors = progress.errors.load(Ordering::Relaxed);
    let dups = arena.count_flag(Flags::HARDLINK_DUP);
    let mounts = arena.count_flag(Flags::MOUNT);
    let rss = max_rss_mb();
    let walker = format!("{:?}", args.walker).to_lowercase();

    if args.json {
        println!(
            "{{\"walker\":\"{walker}\",\"qos\":{},\"threads\":{threads},\"elapsed_ms\":{:.1},\"entries\":{},\"files\":{files},\"dirs\":{dirs},\"bytes\":{bytes},\"root_size\":{},\"errors\":{errors},\"hardlink_dups\":{dups},\"mounts\":{mounts},\"max_rss_mb\":{rss:.1}}}",
            cfg.qos,
            elapsed.as_secs_f64() * 1000.0,
            arena.len(),
            arena.size[0],
        );
        return;
    }

    println!("walker={walker} threads={threads} root={}", args.root.display());
    println!("entries={} files={files} dirs={dirs} errors={errors} hardlink_dups={dups} mounts={mounts}", arena.len());
    println!("total={} ({bytes} B)  root_size={}", human(bytes), human(arena.size[0]));
    println!(
        "elapsed={:.3}s  max_rss={rss:.0} MB  rate={:.0} entries/s",
        elapsed.as_secs_f64(),
        arena.len() as f64 / elapsed.as_secs_f64()
    );
    print_top(&arena, args.top);
}
