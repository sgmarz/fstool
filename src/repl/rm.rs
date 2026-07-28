//! File Removal Command (rm)
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, State, prompt_with};
use crate::{fs::FileType, path::build_path_noderef, path::get_item};
use std::io;

/// Remove `path` from the image, recursing into it first if it is a directory
/// (post-order: children are always unlinked before their parent).
///
/// The borrow pattern is the same used in `cp_item`:
///   1. Collect child names into an owned `Vec<String>` inside a block so
///      the shared borrow of `state.fs` ends before the recursive calls need
///      a mutable borrow.
///   2. Recurse.
///   3. Prompt (if `-i`), then unlink the node itself.
fn rm_recursive(state: &mut State, path: &str, interactive: bool) -> io::Result<()> {
    // Collect child names (shared borrow ends at '}')
    let child_names: Vec<String> = {
        let item = get_item(state.fs.as_ref(), path)?;
        if item.is_dir() {
            item.next()
                .map(|tree| {
                    tree.iter()
                        .filter(|it| it.name() != "." && it.name() != "..")
                        .map(|it| it.name().to_string())
                        .collect()
                })
                .unwrap_or_default()
        }
        else {
            vec![]
        }
        // Shared borrow of state.fs released here.
    };

    // Recurse into children (post-order)
    for child in &child_names {
        let child_path = if path == "/" {
            format!("/{}", child)
        }
        else {
            format!("{}/{}", path.trim_end_matches('/'), child)
        };
        rm_recursive(state, &child_path, interactive)?;
    }

    // Prompt then remove this node
    if interactive && !prompt_with("delete", path) {
        // User said no — leave this node (and implicitly its now-absent
        // children) alone.  This matches GNU rm -i behaviour where saying
        // "no" to a directory after all its contents have been removed still
        // leaves the empty directory in place.
        return Ok(());
    }

    state.changed();
    state.fs.unlink(&path.to_string())
}

// Remove Command

pub(super) fn do_rm(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.is_empty() {
        println!("Usage: rm -[ifr] <path>");
        return;
    }

    let (flags, args): (Vec<&str>, Vec<&str>) =
        args.into_iter().partition(|x: &&&str| x.starts_with('-'));

    if args.is_empty() {
        println!("Usage: rm -[ifr] <path>");
        return;
    }

    let force = flags.iter().any(|x| x.contains('f'));
    let interactive = flags.iter().any(|x| x.contains('i')) && !force;
    // Accept both -r and -R (GNU convention).
    let recursive = flags.iter().any(|x| x.contains('r') || x.contains('R'));

    let path = build_path_noderef(&state.cwd, args[0]);

    // Resolve the target item
    let item = match get_item(state.fs.as_ref(), &path) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let ftype = match state.fs.get_inode(item.inode()) {
        Ok(i) => i.get_file_type(),
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Directory branch
    if ftype == FileType::Directory {
        if !recursive {
            println!("rm: cannot remove '{}': Is a directory.", path);
            return;
        }
        if let Err(e) = rm_recursive(state, &path, interactive) {
            println!("{}", e);
            return;
        }
        return;
    }

    // Non-directory branch (existing behaviour, unchanged)
    if interactive && !prompt_with("delete", &path) {
        return;
    }
    if let Err(e) = state.fs.unlink(&path) {
        println!("{}", e);
        return;
    }
    state.changed();
}
