//! REPL commands related to stat-ing inodes and disk usage.
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, state::State};
use crate::{
    filetype::FileType,
    fs::InodeOperations,
    path::{build_path_deref, get_item},
    stat::{make_combined_list, unix_to_human},
};

fn print_inode(inode: &dyn InodeOperations, dent_size: u64) {
    assert!(dent_size > 0);
    println!("File type: {}", inode.get_file_type().to_string());
    println!(
        "Mode     : {:03o} ({})",
        inode.get_mode(),
        make_combined_list(inode)
    );
    match inode.get_file_type() {
        FileType::Directory => {
            println!(
                "Size     : {} bytes ({} entries)",
                inode.get_size(),
                inode.get_size() / dent_size
            );
        }
        FileType::BlockDevice | FileType::CharacterDevice | FileType::Fifo | FileType::Socket => {}
        _ => {
            println!("Size     : {} bytes", inode.get_size());
        }
    }
    println!("UID      : {}", inode.get_uid());
    println!("GID      : {}", inode.get_gid());
    println!(
        "Nlinks   : {} {}",
        inode.get_nlinks(),
        if inode.get_nlinks() == 0 {
            "(not allocated)"
        }
        else {
            ""
        }
    );
    println!(
        "Atime    : {} ({})",
        inode.get_atime(),
        unix_to_human(inode.get_atime())
    );
    println!(
        "Mtime    : {} ({})",
        inode.get_mtime(),
        unix_to_human(inode.get_mtime())
    );
    println!(
        "Ctime    : {} ({})",
        inode.get_ctime(),
        unix_to_human(inode.get_ctime())
    );
    match inode.get_file_type() {
        FileType::BlockDevice | FileType::CharacterDevice => {
            let bl = inode.get_blocks();
            let major = bl[0] >> 8;
            let minor = bl[0] & 0xFF;
            println!("Maj/Min  : {}/{}", major, minor);
        }
        FileType::Fifo | FileType::Socket => {}
        _ => {
            println!("Blocks   : {:?}", inode.get_blocks());
        }
    }
}

pub(super) fn do_istat(state: &mut State, args: Args) {
    if args.len() != 1 {
        println!("Usage: istat <number>");
        return;
    }
    // TODO: We probably should just remove this. dent size is going to change.
    let dent_size = 64;
    let num_result = args[0].parse::<u64>();
    if num_result.is_err() {
        println!("{}: could not convert to number.", &args[0]);
        return;
    }
    let num = num_result.unwrap();
    match state.fs.get_inode(num) {
        Ok(inode) => {
            println!("Inode    : {}", num);
            print_inode(inode, dent_size);
        }
        Err(e) => {
            eprintln!("istat: {}", e);
        }
    }
}

pub(super) fn do_stat(state: &mut State, args: Args) {
    if args.len() != 1 {
        println!("Usage: stat <path>");
        return;
    }
    // TODO: We probably should just remove this. dent size is going to change.
    let dent_size = 64;
    let abs_path = match build_path_deref(state.fs.as_mut(), &state.cwd, args[0]) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let inum = match get_item(state.fs.as_ref(), &abs_path) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    match state.fs.get_inode(inum.inode()) {
        Ok(i) => {
            println!("Inode    : {}", inum.inode());
            print_inode(i, dent_size);
        }
        Err(e) => {
            println!("{}", e);
            return;
        }
    }
}

/// Format `bytes` for display.
///
/// Without `-h`: 1K-blocks, matching GNU `du` default.
/// With    `-h`: human-readable suffix (K / M / G / T).
fn format_size(bytes: u64, human: bool) -> String {
    if !human {
        // Round up to the next 1 KiB boundary — same as GNU du.
        return format!("{}", (bytes + 1023) / 1024);
    }
    const STEPS: &[(u64, &str)] = &[
        (1 << 40, "T"),
        (1 << 30, "G"),
        (1 << 20, "M"),
        (1 << 10, "K"),
    ];
    for &(limit, unit) in STEPS {
        if bytes >= limit {
            return format!("{:.1}{}", bytes as f64 / limit as f64, unit);
        }
    }
    format!("{}B", bytes)
}

/// Retrieve `(FileType, size_in_bytes)` for the root directory.
///
/// `get_item(fs, "/")` always fails because `split_path("/")` yields an empty
/// vec.  The root inode number is recovered instead from the "." entry that
/// every Minix3 directory carries in its tree.
fn root_stat(state: &mut State) -> Option<(FileType, u64)> {
    let inode_num = state
        .fs
        .get_tree()?
        .iter()
        .find(|it| it.name() == ".")?
        .inode();
    let inode = state.fs.get_inode(inode_num).ok()?;
    Some((inode.get_file_type(), inode.get_size()))
}

/// Collect the names of all direct children of the root directory,
/// excluding "." and "..".
///
/// Same motivation as `root_stat`: `get_item(fs, "/")` cannot be used for
/// root so we read the tree directly.
fn root_children(state: &State) -> Vec<String> {
    state
        .fs
        .get_tree()
        .map(|tree| {
            tree.iter()
                .filter(|it| it.name() != "." && it.name() != "..")
                .map(|it| it.name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

struct DuOpts {
    human_readable: bool, // -h : human-readable sizes
    // -P is stored but the implementation always treats symlinks as opaque
    // (i.e. the physical / no-dereference behaviour) to avoid loops.
    // The flag is accepted for GNU compatibility.
    #[allow(dead_code)]
    physical: bool, // -P : don't dereference symlinks
    summarize: bool,          // -s : only print totals for each argument
    grand_total: bool,        // -c : print a grand total line at the end
    max_depth: Option<usize>, // -dN: only print entries <= N levels deep
    threshold: u64,           // -tN: skip entries smaller than N bytes
}

/// Walk `path` depth-first, accumulate disk usage, and append
/// `(display_path, total_bytes)` entries to `output` in **post-order**
/// (deepest children appear before their parents).
///
/// Returns the total bytes consumed by `path` and everything beneath it.
fn du_item(
    state: &mut State,
    path: &str,
    depth: usize,
    opts: &DuOpts,
    output: &mut Vec<(String, u64)>,
) -> u64 {
    let blk = state.fs.get_superblock().get_block_size() as u64;

    // ── Stat this entry ───────────────────────────────────────────────────────
    //
    // Root ("/") is special: get_item(fs, "/") always errors because
    // split_path("/") returns an empty vec.  Every other path goes through
    // the normal get_item → get_inode route.
    let (ftype, file_size) = if path == "/" {
        match root_stat(state) {
            Some(s) => s,
            None => {
                eprintln!("du: /: I/O error");
                return 0;
            }
        }
    }
    else {
        let item = match get_item(state.fs.as_ref(), path) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("du: {}: {}", path, e);
                return 0;
            }
        };
        match state.fs.get_inode(item.inode()) {
            Ok(inode) => (inode.get_file_type(), inode.get_size()),
            Err(e) => {
                eprintln!("du: {}: {}", path, e);
                return 0;
            }
        }
    };

    // Round file_size up to the next block boundary.
    let own_size = if file_size == 0 {
        0
    }
    else {
        (file_size + blk - 1) / blk * blk
    };

    // ── Non-directory ─────────────────────────────────────────────────────────
    // Symlinks are never followed (prevents loops; equivalent to -P behaviour).
    // Regular files, devices, FIFOs, etc. contribute to their parent's total
    // but are only printed when they are the direct target (depth == 0).
    if ftype != FileType::Directory {
        if depth == 0 && own_size >= opts.threshold {
            output.push((path.to_string(), own_size));
        }
        return own_size;
    }

    // ── Directory: collect child names ────────────────────────────────────────
    //
    // Children are collected into an owned Vec<String> so the shared borrow
    // of state.fs ends before the recursive calls below need it.
    //
    // Root is again special: its "." entry doesn't yield a subtree through
    // get_item, so we read state.fs.get_tree() directly.
    let child_names: Vec<String> = if path == "/" {
        root_children(state)
    }
    else {
        let item = match get_item(state.fs.as_ref(), path) {
            Ok(i) => i,
            Err(_) => return own_size,
        };
        item.next()
            .map(|tree| {
                tree.iter()
                    .filter(|it| it.name() != "." && it.name() != "..")
                    .map(|it| it.name().to_string())
                    .collect()
            })
            .unwrap_or_default()
        // Shared borrow of state.fs ends here.
    };

    // ── Recurse into children (pre-order traversal, post-order output) ────────
    let mut total = own_size;
    for child in &child_names {
        let child_path = if path == "/" {
            format!("/{}", child)
        }
        else {
            format!("{}/{}", path.trim_end_matches('/'), child)
        };
        total += du_item(state, &child_path, depth + 1, opts, output);
    }

    // ── Decide whether to emit a line for this directory ──────────────────────
    //
    // Depth rules (mirroring GNU du):
    //   -s            → only depth 0 (the argument itself); equivalent to -d0
    //   -dN           → depths 0 through N inclusive
    //   (neither)     → all depths
    //
    // Note: -s takes precedence over -d if both are given.
    let within_depth = if opts.summarize {
        depth == 0
    }
    else {
        opts.max_depth.map_or(true, |d| depth <= d)
    };

    if within_depth && total >= opts.threshold {
        // Post-order: appended after all children have been appended above.
        output.push((path.to_string(), total));
    }

    total
}

pub(super) fn do_du(state: &mut State, args: Args) {
    // Split flags from path arguments
    let (flags, paths): (Vec<&str>, Vec<&str>) =
        args.into_iter().partition(|x: &&&str| x.starts_with('-'));

    // Flags
    let human_readable = flags.iter().any(|f| f.contains('h'));
    let physical = flags.iter().any(|f| f.contains('P'));
    let summarize = flags.iter().any(|f| f.contains('s'));
    let grand_total = flags.iter().any(|f| f.contains('c'));

    // -dN : maximum depth to display (0 means only the specified paths themselves)
    // Only the embedded form is supported (e.g. "-d2"), because a standalone
    // "-d 2" would place "2" in the `paths` list after the partition.
    let max_depth: Option<usize> = flags
        .iter()
        .filter_map(|f| {
            f.trim_start_matches('-')
                .strip_prefix('d')
                .and_then(|n| n.parse().ok())
        })
        .next();

    // -tN : threshold in bytes (skip entries smaller than this)
    let threshold: u64 = flags
        .iter()
        .filter_map(|f| {
            f.trim_start_matches('-')
                .strip_prefix('t')
                .and_then(|n| n.parse().ok())
        })
        .next()
        .unwrap_or(0);

    let opts = DuOpts {
        human_readable,
        physical,
        summarize,
        grand_total,
        max_depth,
        threshold,
    };

    // Resolve target paths
    // If no paths were given, default to the current working directory.
    let targets: Vec<String> = if paths.is_empty() {
        let cwd = if state.cwd.is_empty() {
            "/".to_string()
        }
        else {
            format!("/{}", state.cwd.join("/"))
        };
        vec![cwd]
    }
    else {
        paths
            .iter()
            .filter_map(|p| {
                match build_path_deref(state.fs.as_mut(), &state.cwd, p) {
                    Ok(canonical) => Some(canonical),
                    Err(e) => {
                        println!("du: {}: {}", p, e);
                        None
                    }
                }
            })
            .collect()
    };

    // Run du on each target
    let mut grand_bytes: u64 = 0;

    for target in &targets {
        let mut output: Vec<(String, u64)> = Vec::new();
        let total = du_item(state, target, 0, &opts, &mut output);
        grand_bytes += total;

        // Output is already in post-order (children before parents).
        for (path, size) in &output {
            println!("{}\t{}", format_size(*size, opts.human_readable), path);
        }
    }

    // Grand total (-c)
    if opts.grand_total {
        println!("{}\ttotal", format_size(grand_bytes, opts.human_readable));
    }
}
