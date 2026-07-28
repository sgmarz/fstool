//! Transfer Commands (e.g., put and get)
//!
//! © Stephen Marz
//! 8 June 2026
use crate::{
    cache::Item,
    fs::{FileType, SuperblockOperations},
    path::{build_path_deref, build_path_noderef, find_parent_mut, fname_from_path, get_item},
    repl::prompt_with,
};

use super::{Args, State, prompt_overwrite};
use chrono::Local;
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
};

const BUFFER_SIZE: u64 = 4096;

pub(super) fn do_get(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: get -[fv] <remote path> [local path]");
        return;
    }
    let (flags, path): (Vec<&str>, Vec<&str>) = args.iter().partition(|x| x.starts_with('-'));
    let force = flags.iter().any(|x| x.contains('f'));
    let verbose = flags.iter().any(|x| x.contains('v'));

    if path.is_empty() {
        println!("Usage: get -[fv] <remote path> [local path]");
        return;
    }

    // Canonicalize the path to the file we want to download.
    let remote_path = match build_path_deref(state.fs.as_mut(), &state.cwd, path[0]) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let local = if path.len() == 1 {
        // There is only one argument, remote is local, but just the filename part.
        fname_from_path(&remote_path)
    }
    else {
        // There are two arguments, so take the second as the upload filename part.
        path[1]
    };
    // Find the file to download.
    let p = match get_item(state.fs.as_ref(), &remote_path) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if p.is_dir() {
        println!("{}: Is a directory.", path[0]);
        return;
    }
    let inode = match state.fs.get_inode(p.inode()) {
        Ok(inode) => inode,
        Err(_) => {
            println!("{}: I/O Error.", path[0]);
            return;
        }
    };

    // Only support downloading regular files. Downloading directories recursively
    // will be a nightmare, so I'm not going to do that.
    if inode.get_file_type() != FileType::Regular {
        println!("{}: Not a regular file.", path[0]);
        return;
    }

    // Now, try to open the local file depending on the flag given to the command.
    let mut fl = if force {
        match File::create(local) {
            Ok(fl) => fl,
            Err(e) => {
                println!("{}: {}.", local, e);
                return;
            }
        }
    }
    else {
        // Force is not specified, ask the user if you want to overwrite the local file.
        match File::options().write(true).create_new(true).open(local) {
            Ok(fl) => fl,
            Err(_) => {
                if prompt_with("overwrite", local) {
                    match File::create(local) {
                        Ok(fl) => fl,
                        Err(e) => {
                            println!("{}: {}.", local, e);
                            return;
                        }
                    }
                }
                else {
                    // User responded NO to overwrite prompt.
                    return;
                }
            }
        }
    };
    let size = match inode.get_size() {
        0 => return,
        x => x,
    };
    let buffer_size = u64::min(size, BUFFER_SIZE);
    let mut total_bytes_read = 0;
    let mut buffer = vec![0u8; buffer_size as usize];
    for _ in (0..size).step_by(buffer_size as usize) {
        if let Ok(bytes_read) = state.fs.read_file(p.inode(), total_bytes_read, &mut buffer) {
            if let Err(e) = fl.write_all(&buffer[..bytes_read as usize]) {
                if total_bytes_read > 0 {
                    println!();
                }
                println!("{}: {}.", local, e);
                return;
            }
            total_bytes_read += bytes_read;
        }
        else {
            println!("{}: I/O Error.", path[0]);
            return;
        }
    }
    if verbose {
        println!(
            "{}: Downloaded {} bytes to {}.",
            path[0], total_bytes_read, local
        );
    }
}

pub(super) fn do_put(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }

    let (flag_args, path_args): (Vec<&str>, Vec<&str>) =
        args.iter().partition(|x| x.starts_with('-'));

    if path_args.is_empty() {
        println!("Usage: put -[fv] <local path> [remote path]");
        return;
    }

    let force = flag_args.iter().any(|x| x.contains('f'));
    let verbose = flag_args.iter().any(|x| x.contains('v'));

    let (local, remote_arg) = if path_args.len() > 1 {
        (path_args[0], path_args[1].to_string())
    }
    else {
        // If no remote argument is given, use the local file's basename placed
        // into the current working directory.
        let p = Path::new(path_args[0])
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path_args[0].to_string());
        (path_args[0], p)
    };

    // Build the raw absolute destination (symlinks not yet resolved).
    let remote_raw = build_path_noderef(&state.cwd, &remote_arg);

    // Resolve the canonical destination path
    //
    // Three cases:
    //   a) Destination is an existing directory  - put file *inside* it.
    //   b) Destination is an existing file       - will be unlinked later.
    //   c) Destination does not exist            - canonicalize its parent.
    let remote: String = match build_path_deref(state.fs.as_mut(), &state.cwd, &remote_arg) {
        Ok(existing) => {
            match get_item(state.fs.as_ref(), &existing) {
                Ok(item) if item.is_dir() => {
                    // Place the file inside the directory using the local basename.
                    let fname = Path::new(local)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| local.to_string());
                    if existing == "/" {
                        format!("/{}", fname)
                    }
                    else {
                        format!("{}/{}", existing.trim_end_matches('/'), fname)
                    }
                }
                Ok(_) => existing, // Existing non-directory; handled later.
                Err(e) => {
                    println!("{}: {}", remote_arg, e);
                    return;
                }
            }
        }
        Err(_) => {
            // Destination does not yet exist — canonicalize its parent so we
            // know it's a valid location.  Fail early if the parent is missing.
            let fname = fname_from_path(&remote_raw).to_string();
            let parent_raw = Path::new(&remote_raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());
            let canonical_parent =
                match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}: {}", parent_raw, e);
                        return;
                    }
                };
            if canonical_parent == "/" {
                format!("/{}", fname)
            }
            else {
                format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
            }
        }
    };

    // If the remote exists as a regular file, unlink it
    if let Ok(item) = get_item(state.fs.as_ref(), &remote) {
        if !force && !prompt_overwrite(&remote) {
            return;
        }
        let ftype = match state.fs.get_inode(item.inode()) {
            Ok(inode) => inode.get_file_type(),
            Err(e) => {
                println!("{}: {}", remote, e);
                return;
            }
        };
        if ftype != FileType::Regular {
            println!("{}: Not a regular file.", remote);
            return;
        }
        if let Err(e) = state.fs.unlink(&remote) {
            println!("{}: {}", remote, e);
            return;
        }
    }

    // Open and measure the local file BEFORE doing any on-disk mutations.
    // This way we avoid creating an empty, potentially large, file on
    // the remote if the local file is missing or unreadable.
    //
    // This happens AFTER all remote-path validation so we don't open a file
    // descriptor we can't use.
    let mut fl = match File::open(local) {
        Ok(f) => f,
        Err(e) => {
            println!("{}: {}", local, e);
            return;
        }
    };
    let flsize = match fl.seek(SeekFrom::End(0)) {
        Ok(s) => s,
        Err(e) => {
            println!("{}: {}", local, e);
            return;
        }
    };
    if let Err(e) = fl.seek(SeekFrom::Start(0)) {
        println!("{}: {}", local, e);
        return;
    }

    // Verify there is enough free space
    let f = FreeData::new(state.fs.get_superblock());
    if f.inodes == 0 {
        println!("put: no free inodes.");
        return;
    }
    assert!(f.blk_size > 0);
    // Correct ceiling division — the original used (& !blk_size) which
    // rounded the wrong operand.
    let need_blocks = (flsize + f.blk_size - 1) / f.blk_size;
    if f.blocks < need_blocks {
        println!(
            "{}: file too large. Need {} blocks, only {} free.",
            remote, need_blocks, f.blocks
        );
        return;
    }

    // fs.create allocates the inode and writes the directory entry.
    // All further metadata is set by hand immediately after.
    let new_inode_num = match state.fs.create(&remote, FileType::Regular) {
        Ok(i) => i,
        Err(e) => {
            println!("{}: {}", remote, e);
            return;
        }
    };

    // Initialize inode metadata
    let now = Local::now().timestamp() as u64;
    let mode = 0o666 & !state.umask; // rw-rw-rw- minus umask; no execute bits
    {
        let inode = match state.fs.get_inode_mut(new_inode_num) {
            Ok(i) => i,
            Err(e) => {
                println!("{}: {}", remote, e);
                return;
            }
        };
        // Should we move some of this into the create function? Seems like we might miss something, or
        // if the inode API changes we might forget to update this code and all the other code
        // that does this manually.
        inode.set_file_type(FileType::Regular);
        inode.set_mode(mode);
        inode.set_uid(state.uid);
        inode.set_gid(state.gid);
        inode.set_size(0); // truncate in write_file will set the real size
        inode.set_atime(now);
        inode.set_mtime(now);
        inode.set_ctime(now);
        inode.set_nlinks(1);
    }

    // Insert the new entry into the in-memory directory tree
    {
        let parent_tree = match find_parent_mut(state.fs.as_mut(), &remote) {
            Ok(t) => t,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        parent_tree.push(Item::new(
            fname_from_path(&remote).to_string(),
            new_inode_num,
            FileType::Regular,
        ));
    }

    // Read the entire file into one buffer so write_file receives a single
    // contiguous slice.  write_file calls truncate internally, which allocates
    // all the necessary data blocks before writing begins.
    if flsize > 0 {
        let mut buffer = vec![0u8; flsize as usize];
        if let Err(e) = fl.read_exact(&mut buffer) {
            println!("{}: {}", local, e);
            // Best-effort cleanup: remove the empty inode we just created.
            let _ = state.fs.unlink(&remote);
            return;
        }
        if let Err(e) = state.fs.write_file(new_inode_num, 0, &buffer) {
            println!("{}: {}", remote, e);
            let _ = state.fs.unlink(&remote);
            return;
        }
    }
    if verbose {
        println!("{} -> {} ({} bytes).", local, remote, flsize);
    }
    state.changed();
}

/// Compute the effective canonical destination path for `cp` / `mv`.
///
/// POSIX rules:
/// * If `dst` already resolves to an **existing directory**, return
///   `<dst>/<src_basename>` so the source is placed *inside* the directory.
/// * If `dst` does **not** exist, canonicalize its parent and return
///   `<canonical_parent>/<dst_basename>`.  Errors if the parent doesn't exist.
/// * If `dst` exists but is **not** a directory, return it as-is; the caller
///   decides whether to overwrite.
fn resolve_dst(state: &mut State, src_canonical: &str, dst: &str) -> io::Result<String> {
    match build_path_deref(state.fs.as_mut(), &state.cwd, dst) {
        Ok(existing) => {
            let item = get_item(state.fs.as_ref(), &existing)?;
            let ftype = state.fs.get_inode(item.inode())?.get_file_type();
            if ftype == FileType::Directory {
                // Copy/move *into* the existing directory.
                let fname = fname_from_path(src_canonical);
                if existing == "/" {
                    Ok(format!("/{}", fname))
                }
                else {
                    Ok(format!("{}/{}", existing.trim_end_matches('/'), fname))
                }
            }
            else {
                Ok(existing)
            }
        }
        Err(_) => {
            // dst doesn't exist yet — validate its parent and build the full path.
            let raw = build_path_noderef(&state.cwd, dst);
            let fname = fname_from_path(&raw).to_string();
            let parent_raw = Path::new(&raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());
            let canonical_parent = build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw)?;
            if canonical_parent == "/" {
                Ok(format!("/{}", fname))
            }
            else {
                Ok(format!(
                    "{}/{}",
                    canonical_parent.trim_end_matches('/'),
                    fname
                ))
            }
        }
    }
}

/// Create a directory at `dst_canonical`, initialise its inode metadata, and
/// insert it into the in-memory tree.  Extracted from `do_mkdir` so both
/// `cp` and `mv` can reuse the logic without duplication.
fn mkdir_at(
    state: &mut State,
    dst_canonical: &str,
    mode: u16,
    uid: u32,
    gid: u32,
) -> io::Result<u64> {
    let new_inode_num = state
        .fs
        .create(&dst_canonical.to_string(), FileType::Directory)?;
    // TODO: this needs to be changed to use the create() API.
    let dot_dotdot_size = 64 * 2;
    let now = Local::now().timestamp() as u64;
    {
        let inode = state.fs.get_inode_mut(new_inode_num)?;
        inode.set_file_type(FileType::Directory);
        inode.set_mode(mode);
        inode.set_uid(uid);
        inode.set_gid(gid);
        inode.set_size(dot_dotdot_size);
        inode.set_atime(now);
        inode.set_mtime(now);
        inode.set_ctime(now);
        inode.set_nlinks(1);
    }
    {
        let parent_tree = find_parent_mut(state.fs.as_mut(), dst_canonical)?;
        let parent_inode = parent_tree
            .iter()
            .find(|it| it.name() == ".")
            .map(|it| it.inode())
            .unwrap_or(1);
        let fname = fname_from_path(dst_canonical).to_string();
        let dot = Item::new_dir(".".to_string(), new_inode_num, vec![]);
        let dotdot = Item::new_dir("..".to_string(), parent_inode, vec![]);
        parent_tree.push(Item::new_dir(fname, new_inode_num, vec![dot, dotdot]));
    }
    Ok(new_inode_num)
}

// ── cp ────────────────────────────────────────────────────────────────────────

/// Copy one item from `src_canonical` to `dst_canonical`.
/// Called recursively for directory subtrees when `-r` is active.
fn cp_item(
    state: &mut State,
    src_canonical: &str,
    dst_canonical: &str,
    recursive: bool,
    verbose: bool,
) -> io::Result<()> {
    let src_inode_num = get_item(state.fs.as_ref(), src_canonical)?.inode();
    let (ftype, src_size, src_mode, src_uid, src_gid) = {
        let inode = state.fs.get_inode(src_inode_num)?;
        (
            inode.get_file_type(),
            inode.get_size(),
            inode.get_mode(),
            inode.get_uid(),
            inode.get_gid(),
        )
    };

    match ftype {
        // ── Directory ─────────────────────────────────────────────────────────
        FileType::Directory => {
            if !recursive {
                return Err(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("omitting directory '{}'", src_canonical),
                ));
            }

            // Collect child names *before* mutating the tree so we hold no
            // shared borrow while mkdir_at / cp_item take mutable ones.
            let child_names: Vec<String> = get_item(state.fs.as_ref(), src_canonical)?
                .next()
                .map(|tree| {
                    tree.iter()
                        .filter(|it| it.name() != "." && it.name() != "..")
                        .map(|it| it.name().to_string())
                        .collect()
                })
                .unwrap_or_default();

            mkdir_at(state, dst_canonical, src_mode, src_uid, src_gid)?;

            if verbose {
                println!("'{}' -> '{}'", src_canonical, dst_canonical);
            }

            for child in child_names {
                let child_src = format!("{}/{}", src_canonical.trim_end_matches('/'), child);
                let child_dst = format!("{}/{}", dst_canonical.trim_end_matches('/'), child);
                cp_item(state, &child_src, &child_dst, recursive, verbose)?;
            }
        }

        // ── Regular file ──────────────────────────────────────────────────────
        FileType::Regular => {
            let new_inode_num = state
                .fs
                .create(&dst_canonical.to_string(), FileType::Regular)?;
            let now = Local::now().timestamp() as u64;
            {
                let inode = state.fs.get_inode_mut(new_inode_num)?;
                inode.set_file_type(FileType::Regular);
                inode.set_mode(src_mode);
                inode.set_uid(src_uid);
                inode.set_gid(src_gid);
                inode.set_size(0);
                inode.set_atime(now);
                inode.set_mtime(now);
                inode.set_ctime(now);
                inode.set_nlinks(1);
            }
            if src_size > 0 {
                let mut buf = vec![0u8; src_size as usize];
                state.fs.read_file(src_inode_num, 0, &mut buf)?;
                state.fs.write_file(new_inode_num, 0, &buf)?;
            }
            {
                let parent_tree = find_parent_mut(state.fs.as_mut(), dst_canonical)?;
                // ⚠ Adjust Item::new() to match your cache module's leaf constructor.
                parent_tree.push(Item::new(
                    fname_from_path(dst_canonical).to_string(),
                    new_inode_num,
                    FileType::Regular,
                ));
            }
            if verbose {
                println!("'{}' -> '{}'", src_canonical, dst_canonical);
            }
        }

        // ── Symbolic link ─────────────────────────────────────────────────────
        FileType::Symlink => {
            let target = state.fs.read_symlink(src_inode_num)?;
            let new_inode_num = state
                .fs
                .create(&dst_canonical.to_string(), FileType::Symlink)?;
            state.fs.write_symlink(new_inode_num, &target)?;
            let now = Local::now().timestamp() as u64;
            {
                let inode = state.fs.get_inode_mut(new_inode_num)?;
                inode.set_mode(src_mode);
                inode.set_uid(src_uid);
                inode.set_gid(src_gid);
                inode.set_atime(now);
                inode.set_mtime(now);
                inode.set_ctime(now);
                inode.set_nlinks(1);
            }
            {
                let parent_tree = find_parent_mut(state.fs.as_mut(), dst_canonical)?;
                parent_tree.push(Item::new(
                    fname_from_path(dst_canonical).to_string(),
                    new_inode_num,
                    FileType::Symlink,
                ));
            }
            if verbose {
                println!("'{}' -> '{}'", src_canonical, dst_canonical);
            }
        }

        other => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("cannot copy {:?} file '{}'", other, src_canonical),
            ));
        }
    }

    Ok(())
}

pub(super) fn do_cp(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.is_empty() {
        println!("Usage: cp [options] <from path> <to path>");
        return;
    }

    let (flag_args, path_args): (Vec<_>, Vec<_>) = args.iter().partition(|a| a.starts_with('-'));
    if path_args.len() < 2 {
        println!("Usage: cp [-rRiv] <from> <to>");
        return;
    }

    let recursive = flag_args
        .iter()
        .any(|f: &&str| f.contains('r') || f.contains('R'));
    let interactive = flag_args.iter().any(|f: &&str| f.contains('i'));
    let verbose = flag_args.iter().any(|f: &&str| f.contains('v'));
    let src = path_args[0];
    let dst = path_args[1];

    // Resolve source — must already exist.
    let src_canonical = match build_path_deref(state.fs.as_mut(), &state.cwd, src) {
        Ok(p) => p,
        Err(e) => {
            println!("cp: {}: {}", src, e);
            return;
        }
    };
    if src_canonical == "/" {
        println!("cp: cannot copy the root directory.");
        return;
    }

    // Resolve effective destination.
    let dst_canonical = match resolve_dst(state, &src_canonical, dst) {
        Ok(p) => p,
        Err(e) => {
            println!("cp: {}", e);
            return;
        }
    };

    // Sanity guards.
    if src_canonical == dst_canonical {
        println!("cp: '{}' and '{}' are the same file.", src, dst);
        return;
    }
    if dst_canonical.starts_with(&format!("{}/", src_canonical)) {
        println!(
            "cp: cannot copy '{}' into its own subdirectory '{}'.",
            src, dst
        );
        return;
    }

    // If the effective destination exists and is not a directory, optionally
    // prompt then unlink it before overwriting.
    if let Ok(existing) = build_path_deref(state.fs.as_mut(), &state.cwd, &dst_canonical) {
        let dst_is_dir = get_item(state.fs.as_ref(), &existing)
            .ok()
            .and_then(|it| state.fs.get_inode(it.inode()).ok())
            .map(|in_| in_.get_file_type() == FileType::Directory)
            .unwrap_or(false);

        if !dst_is_dir {
            if interactive && !prompt_overwrite(&dst_canonical) {
                return;
            }
            if let Err(e) = state.fs.unlink(&dst_canonical) {
                println!("cp: cannot overwrite '{}': {}", dst_canonical, e);
                return;
            }
        }
    }

    if let Err(e) = cp_item(state, &src_canonical, &dst_canonical, recursive, verbose) {
        println!("cp: {}", e);
        return;
    }
    state.changed();
}

// ── mv ────────────────────────────────────────────────────────────────────────

/// Patch the on-disk `..` entry inside the directory now located at
/// `dir_canonical` so it points at `new_parent_inode`, mirror that change in
/// the in-memory tree, and adjust hard-link counts on both parent inodes.
///
/// Every directory's `..` entry is a hard link to its parent, so moving a
/// directory transfers one link from the old parent to the new one.
fn update_dotdot(
    state: &mut State,
    dir_canonical: &str,
    new_parent_inode: u64,
    old_parent_inode: u64,
) -> io::Result<()> {
    // ── On-disk update ────────────────────────────────────────────────────────
    let dir_inode_num = get_item(state.fs.as_ref(), dir_canonical)?.inode();
    let first_block = state.fs.get_inode(dir_inode_num)?.get_blocks()[0];

    if first_block != 0 {
        let bsize = state.fs.get_superblock().get_block_size() as usize;
        let mut bdata = vec![0u8; bsize];
        state.fs.read_block(first_block, &mut bdata)?;
        // `..` is always the second entry, immediately after `.`.
        state.fs.write_block(first_block, &bdata)?;
    }

    // ── In-memory tree update ─────────────────────────────────────────────────
    // Navigate to the moved directory in its new parent tree, find its `..`
    // child, and replace it with one pointing at new_parent_inode.
    let dst_fname = fname_from_path(dir_canonical).to_string();
    let parent_tree = find_parent_mut(state.fs.as_mut(), dir_canonical)?;
    if let Some(moved_dir) = parent_tree.iter_mut().find(|it| it.name().eq(&dst_fname)) {
        if let Some(children) = moved_dir.next_mut() {
            if let Some(idx) = children.iter().position(|it| it.name().eq("..")) {
                children[idx] = Item::new_dir("..".to_string(), new_parent_inode, vec![]);
            }
        }
    }

    // ── nlinks adjustment ─────────────────────────────────────────────────────
    // The moved directory's `..` transferred from old_parent to new_parent,
    // so old_parent loses one hard link and new_parent gains one.
    if old_parent_inode != new_parent_inode {
        let old_n = state.fs.get_inode(old_parent_inode)?.get_nlinks();
        state
            .fs
            .get_inode_mut(old_parent_inode)?
            .set_nlinks(old_n.saturating_sub(1));
        let new_n = state.fs.get_inode(new_parent_inode)?.get_nlinks();
        state
            .fs
            .get_inode_mut(new_parent_inode)?
            .set_nlinks(new_n + 1);
    }

    Ok(())
}

pub(super) fn do_mv(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.is_empty() {
        println!("Usage: mv <from path> <to path>");
        return;
    }

    let (flag_args, path_args): (Vec<_>, Vec<_>) = args.iter().partition(|a| a.starts_with('-'));
    if path_args.len() < 2 {
        println!("Usage: mv [-if] <from> <to>");
        return;
    }

    let interactive = flag_args.iter().any(|f: &&str| f.contains('i'));
    let force = flag_args.iter().any(|f: &&str| f.contains('f'));
    let src = path_args[0];
    let dst = path_args[1];

    // ── Resolve source ────────────────────────────────────────────────────────
    let src_canonical = match build_path_deref(state.fs.as_mut(), &state.cwd, src) {
        Ok(p) => p,
        Err(e) => {
            println!("mv: {}: {}", src, e);
            return;
        }
    };
    if src_canonical == "/" {
        println!("mv: cannot move the root directory.");
        return;
    }

    // ── Resolve effective destination ─────────────────────────────────────────
    let dst_canonical = match resolve_dst(state, &src_canonical, dst) {
        Ok(p) => p,
        Err(e) => {
            println!("mv: {}", e);
            return;
        }
    };

    if src_canonical == dst_canonical {
        println!("mv: '{}' and '{}' are the same file.", src, dst);
        return;
    }
    if dst_canonical.starts_with(&format!("{}/", src_canonical)) {
        println!(
            "mv: cannot move '{}' into its own subdirectory '{}'.",
            src, dst
        );
        return;
    }

    // ── Source metadata ───────────────────────────────────────────────────────
    let src_item = match get_item(state.fs.as_ref(), &src_canonical) {
        Ok(i) => i,
        Err(e) => {
            println!("mv: {}", e);
            return;
        }
    };
    let src_inode_num = src_item.inode();
    let src_ftype = match state.fs.get_inode(src_inode_num) {
        Ok(i) => i.get_file_type(),
        Err(e) => {
            println!("mv: {}", e);
            return;
        }
    };

    // ── Handle existing destination ───────────────────────────────────────────
    if let Ok(existing_dst) = build_path_deref(state.fs.as_mut(), &state.cwd, &dst_canonical) {
        let dst_item = match get_item(state.fs.as_ref(), &existing_dst) {
            Ok(i) => i,
            Err(e) => {
                println!("mv: {}", e);
                return;
            }
        };
        let dst_ftype = match state.fs.get_inode(dst_item.inode()) {
            Ok(i) => i.get_file_type(),
            Err(e) => {
                println!("mv: {}", e);
                return;
            }
        };

        // POSIX type-compatibility checks.
        if src_ftype == FileType::Directory && dst_ftype != FileType::Directory {
            println!(
                "mv: cannot overwrite non-directory '{}' with directory '{}'.",
                dst, src
            );
            return;
        }
        if src_ftype != FileType::Directory && dst_ftype == FileType::Directory {
            println!(
                "mv: cannot overwrite directory '{}' with non-directory '{}'.",
                dst, src
            );
            return;
        }

        // When both -i and -f are given, interactive wins (matches GNU mv).
        if interactive && !force && !prompt_overwrite(&dst_canonical) {
            return;
        }

        // Unlink the existing non-directory destination.  Directory-into-
        // directory was already handled by resolve_dst.
        if dst_ftype != FileType::Directory {
            if let Err(e) = state.fs.unlink(&dst_canonical) {
                println!("mv: cannot overwrite '{}': {}", dst_canonical, e);
                return;
            }
        }
    }

    // ── Rename (same filesystem — no data copy) ───────────────────────────────
    //
    //  Strategy
    //  ─────────
    //  1. Bump source nlinks so unlink() cannot free the inode prematurely.
    //  2. Clone the in-memory Item (and its full subtree for directories).
    //  3. create_dentry  — write the new on-disk directory entry.
    //  4. unlink         — zero the old on-disk entry, decrement nlinks, and
    //                      remove the Item from the in-memory tree.
    //  5. Re-insert the cloned Item (renamed) into the destination parent tree.
    //  6. For directory sources: patch `..` on disk and in the tree, and
    //     transfer one hard link from the old parent to the new one.

    // Step 1 — protect the inode.
    {
        let n = match state.fs.get_inode(src_inode_num) {
            Ok(i) => i.get_nlinks(),
            Err(e) => {
                println!("mv: {}", e);
                return;
            }
        };
        if let Err(e) = state
            .fs
            .get_inode_mut(src_inode_num)
            .map(|i| i.set_nlinks(n + 1))
        {
            println!("mv: {}", e);
            return;
        }
    }

    // Step 2 — clone the whole Item subtree (safe: get_item returns a clone).
    let cloned_children: Option<Vec<Item>> = src_item.next().map(|t| t.clone());

    // Step 3 — write the new on-disk directory entry for the destination.
    if let Err(e) = state.fs.link(src_inode_num, &dst_canonical) {
        // Roll back the nlinks bump on failure.
        if let Ok(i) = state.fs.get_inode_mut(src_inode_num) {
            let n = i.get_nlinks();
            i.set_nlinks(n.saturating_sub(1));
        }
        println!("mv: {}", e);
        return;
    }

    // Step 4 — zero the old on-disk entry and remove it from the tree.
    if let Err(e) = state.fs.unlink(&src_canonical) {
        println!("mv: {}", e);
        return;
    }

    // Step 5 — insert the renamed Item into the destination parent tree.
    let dst_fname = fname_from_path(&dst_canonical).to_string();
    {
        let parent_tree = match find_parent_mut(state.fs.as_mut(), &dst_canonical) {
            Ok(t) => t,
            Err(e) => {
                println!("mv: {}", e);
                return;
            }
        };
        let new_item = match cloned_children {
            // Directory: preserve the entire in-memory subtree.
            Some(children) => Item::new_dir(dst_fname.clone(), src_inode_num, children),
            // Non-directory leaf.
            // ⚠ Adjust Item::new() to match your cache module's constructor.
            None => Item::new(dst_fname.clone(), src_inode_num, src_ftype),
        };
        parent_tree.push(new_item);
    }

    // Step 6 — for directory sources, fix `..` and parent link counts.
    if src_ftype == FileType::Directory {
        // New parent: read its inode number from the "." entry in its own tree.
        let new_parent_inode = find_parent_mut(state.fs.as_mut(), &dst_canonical)
            .ok()
            .and_then(|t| t.iter().find(|it| it.name() == ".").map(|it| it.inode()))
            .unwrap_or(1);

        // Old parent: canonicalize the parent component of the original source path.
        let old_parent_inode = Path::new(&src_canonical)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .and_then(|p| build_path_deref(state.fs.as_mut(), &[], &p).ok())
            .and_then(|p| get_item(state.fs.as_ref(), &p).ok())
            .map(|it| it.inode())
            .unwrap_or(1);

        if let Err(e) = update_dotdot(state, &dst_canonical, new_parent_inode, old_parent_inode) {
            println!("mv: warning: could not update '..': {}", e);
        }
    }

    state.changed();
}

pub(super) fn do_save(state: &mut State, _args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if let Err(e) = state.fs.write_to_backing() {
        println!("Error saving filesystem: {}", e);
        return;
    }
    println!("Filesystem saved.");
    state.reset_changed();
}

struct FreeData {
    pub blocks: u64,
    pub inodes: u64,
    pub blk_size: u64,
}
impl FreeData {
    fn new(sb: &dyn SuperblockOperations) -> Self {
        Self {
            blocks: sb.get_num_blocks().free,
            inodes: sb.get_num_inodes().free,
            blk_size: sb.get_block_size(),
        }
    }
}
