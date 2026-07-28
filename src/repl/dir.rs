//! Directory Commands (e.g., mkdir, rmdir, chdir)
//!
//! © Stephen Marz
//! 8 June 2026
use super::Args;
use super::state::State;
use crate::cache::Item;
use crate::path::{
    build_path_deref, find_parent_mut, find_parent_ref, fname_from_path, get_item, split_path,
};
use crate::{fs::FileType, path::build_path_noderef};
use chrono::Local;
use std::{
    io::{self, BufRead, Write},
    path::Path,
};

// Helpers

/// Prompt "mkdir: create directory 'path'? [y/N] " and return true if the
/// user replies "y" or "yes" (case-insensitive).  Any I/O error is treated
/// as a "no" so the caller can abort safely.
fn prompt_mkdir(path: &str) -> bool {
    print!("mkdir: create directory '{}'? [y/N] ", path);
    std::io::stdout().flush().ok();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).ok();
    matches!(line.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

/// Core directory-creation logic.
///
/// `path` may be absolute or relative to `state.cwd`.  The function:
///   1. Resolves the canonical absolute path.
///   2. Allocates an inode via `fs.create`.
///   3. Sets all inode metadata.
///   4. Inserts the new node (with `.` and `..`) into the in-memory tree.
///
/// The caller is responsible for:
///   - Verifying the path does not already exist.
///   - Calling `state.changed()` after a successful return.
fn mkdir_one(state: &mut State, path: &str) -> io::Result<()> {
    // Step 1 — Raw absolute path (symlinks not yet resolved).
    let raw = build_path_noderef(&state.cwd, path);
    let fname = fname_from_path(&raw).to_string();

    let parent_raw = Path::new(&raw)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());

    // Step 2 — Canonicalize the parent (must already exist).
    let canonical_parent = build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw)?;

    let canonical_abs = if canonical_parent == "/" {
        format!("/{}", fname)
    }
    else {
        format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
    };

    // Step 3 — Allocate inode + disk-level directory entry.
    let new_inode_num = state.fs.create(&canonical_abs, FileType::Directory)?;

    // Step 4 — Set inode metadata.
    // TODO: Change this to use the create() API.
    let dot_dotdot_size = 64 * 2;
    let mode = state.dir_umask();
    let now = Local::now().timestamp() as u64;
    {
        let inode = state.fs.get_inode_mut(new_inode_num)?;
        inode.set_file_type(FileType::Directory);
        inode.set_mode(mode);
        inode.set_uid(state.uid);
        inode.set_gid(state.gid);
        inode.set_size(dot_dotdot_size);
        inode.set_atime(now);
        inode.set_mtime(now);
        inode.set_ctime(now);
        inode.set_nlinks(1);
    }

    // Step 5 — Update the in-memory tree.
    {
        let parent_tree = find_parent_mut(state.fs.as_mut(), &canonical_abs)?;

        // Read the parent's own inode number from its "." entry.
        let parent_inode = parent_tree
            .iter()
            .find(|it| it.name() == ".")
            .map(|it| it.inode())
            .unwrap_or(1);

        let dot = Item::new_dir(".".to_string(), new_inode_num, vec![]);
        let dotdot = Item::new_dir("..".to_string(), parent_inode, vec![]);
        parent_tree.push(Item::new_dir(fname, new_inode_num, vec![dot, dotdot]));
        // Mutable borrow of state.fs ends here.
    }

    Ok(())
}

pub(super) fn do_rmdir(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.len() != 1 {
        println!("Usage: rmdir <path>");
        return;
    }

    // Resolve symlinks so we operate on the real canonical path.
    let path = match build_path_deref(state.fs.as_mut(), &state.cwd, &args[0]) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let fname = fname_from_path(&path).to_string();

    // find_parent_ref validates every intermediate component is a directory
    // and hands us the parent tree so we can look up the final item without
    // a second full traversal.  The block scope ends the immutable borrow of
    // state.fs before the get_inode call below.
    let item = {
        let parent = match find_parent_ref(state.fs.as_ref(), &path) {
            Ok(t) => t,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        match parent.iter().find(|it| it.name().eq(&fname)) {
            Some(i) => i.clone(),
            None => {
                println!(
                    "rmdir: failed to remove '{}': No such file or directory.",
                    path
                );
                return;
            }
        }
        // `parent` — and thus the immutable borrow of state.fs — drops here.
    };

    // TODO: Change this to use the unlink() API.
    let empty_dir_size = 64 * 2;

    let inode = match state.fs.get_inode(item.inode()) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Check type before size for a clearer error message when the target isn't a dir.
    if inode.get_file_type() != FileType::Directory {
        println!("rmdir: failed to remove '{}': Not a directory.", path);
        return;
    }

    if inode.get_size() > empty_dir_size {
        println!("rmdir: failed to remove '{}': Directory not empty.", path);
        return;
    }

    if let Err(e) = state.fs.unlink(&path) {
        println!("{}", e);
        return;
    }
    state.changed();
}

pub(super) fn do_mkdir(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }

    // Flags: -i = interactive (prompt before each create)
    //        -p = parents (create intermediate directories as needed)
    // With no flags, interactive defaults to false.
    let (interactive, parents, path) = match args.len() {
        1 => (false, false, args[0]),
        2 => {
            (
                args[0].chars().any(|c| c == 'i'),
                args[0].chars().any(|c| c == 'p'),
                args[1],
            )
        }
        _ => {
            println!("Usage: mkdir [-ip] <path>");
            return;
        }
    };

    if parents {
        // Parents '-p' mode
        //
        // Walk every component of the raw absolute path.  Existing directories
        // are silently accepted.  Missing ones are created one at a time.
        // "." and ".." are resolved but never passed to mkdir_one.

        let raw = build_path_noderef(&state.cwd, path);
        let components = split_path(&raw);

        // `prefix` holds the canonical form of the path built so far.
        let mut prefix = String::from("/");

        for component in &components {
            let candidate = if prefix == "/" {
                format!("/{}", component)
            }
            else {
                format!("{}/{}", prefix.trim_end_matches('/'), component)
            };

            // Resolve "." and ".." by updating prefix only — never create them.
            if *component == "." || *component == ".." {
                if let Ok(resolved) = build_path_deref(state.fs.as_mut(), &[], &candidate) {
                    prefix = resolved;
                }
                continue;
            }

            match build_path_deref(state.fs.as_mut(), &[], &candidate) {
                Ok(resolved) => {
                    // Path already exists — verify it is a directory.
                    //
                    // We use a nested block so the get_inode borrow ends before
                    // the next loop iteration touches state.fs again.
                    let is_dir = {
                        get_item(state.fs.as_ref(), &resolved)
                            .ok()
                            .and_then(|item| state.fs.get_inode(item.inode()).ok())
                            .map(|inode| inode.get_file_type() == FileType::Directory)
                            .unwrap_or(true) // unknown type → let the OS sort it out
                    };
                    if !is_dir {
                        println!("mkdir: cannot create directory '{}': Not a directory", path);
                        return;
                    }
                    prefix = resolved;
                }

                Err(_) => {
                    // Does not exist — optionally prompt, then create.
                    if interactive && !prompt_mkdir(&candidate) {
                        // User declined.  For -p -i, stopping here is intentional:
                        // if the user says "no" to an intermediate directory, the
                        // rest of the path cannot be created either.
                        return;
                    }
                    match mkdir_one(state, &candidate) {
                        Ok(()) => {
                            state.changed();
                            // `candidate` is a freshly created real directory so
                            // no symlink resolution is needed — use it directly.
                            prefix = candidate;
                        }
                        Err(e) => {
                            println!("mkdir: cannot create directory '{}': {}", candidate, e);
                            return;
                        }
                    }
                }
            }
        }
    }
    else {
        // Normal (non -p) mode
        //
        // Reject if the target already exists.
        if build_path_deref(state.fs.as_mut(), &state.cwd, path).is_ok() {
            println!("{}: File exists.", path);
            return;
        }

        // -i: ask before creating.
        if interactive && !prompt_mkdir(path) {
            return;
        }

        match mkdir_one(state, path) {
            Ok(()) => state.changed(),
            Err(e) => println!("{}", e),
        }
    }
}

pub(super) fn do_chdir(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("Usage: cd <path>");
        return;
    }
    let to = match build_path_deref(state.fs.as_mut(), &state.cwd, args[0]) {
        Ok(x) => x,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Verify every component is a directory by asking for the parent tree.
    // find_parent_ref walks the full chain and errors on any non-directory.
    match find_parent_ref(state.fs.as_ref(), &to) {
        Err(e) => {
            println!("{}", e);
            return;
        }
        Ok(parent) => {
            let fname = fname_from_path(&to);
            match parent.iter().find(|it| it.name() == fname) {
                Some(p) if !p.is_dir() => {
                    println!("{}: Not a directory.", fname);
                    return;
                }
                _ => {}
            }
        }
    }

    // build_path_deref already resolved . and .., so the components are clean.
    state.cwd = split_path(&to).iter().map(|s| s.to_string()).collect();
}
