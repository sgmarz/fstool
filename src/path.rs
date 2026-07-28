//! Path manipulation utilities.
//!
//! All functions operate against the in-memory directory tree or on path
//! strings.
//!
//! © Stephen Marz
//! 8 June 2026
use super::{cache::Item, fs::FileSystem};
use crate::{cache::Tree, fs::FileType};
use std::collections::{HashSet, VecDeque};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Render `path` as a Unix-style string, always using `/` as the separator.
///
/// This is necessary for cross-platform correctness: [`PathBuf`] internally
/// uses `\` on Windows, but Unix-style FS paths always use `/`.
fn path_to_unix_string(path: &Path) -> String {
    let mut out = String::new();
    for component in path.components() {
        match component {
            Component::RootDir => out.push('/'),
            Component::Normal(name) => {
                if !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(name.to_str().unwrap_or(""));
            }
            Component::CurDir | Component::ParentDir => {
                if !out.ends_with('/') {
                    out.push('/');
                }
                out.push_str(component.as_os_str().to_str().unwrap_or(""));
            }
            _ => {}
        }
    }
    if out.is_empty() {
        out.push('/');
    }
    out
}

/// Combine `cwd` and `name` into an absolute path **without** resolving
/// symbolic links.
///
/// If `name` is already absolute, `cwd` is ignored.  The resulting string is
/// not lexically normalised — `.` and `..` components are preserved as-is.
pub fn build_path_noderef(cwd: &[String], name: &str) -> String {
    let path = if Path::new(name).is_absolute() {
        PathBuf::from(name)
    }
    else {
        let mut p = PathBuf::from("/");
        for component in cwd {
            p.push(component);
        }
        p.push(name);
        p
    };
    path_to_unix_string(&path)
}

/// Combine `cwd` and `name` into a fully-resolved absolute path, following
/// symbolic links (analogous to `realpath`), but operating on the image's
/// in-memory directory tree rather than the host file system.
///
/// Symbolic link loops are detected via inode tracking and reported as an
/// [`io::Error`] with kind [`io::ErrorKind::InvalidInput`].
pub fn build_path_deref(fs: &mut dyn FileSystem, cwd: &[String], name: &str) -> io::Result<String> {
    let raw = build_path_noderef(cwd, name);

    // Decompose the raw path into a work queue.  When a symlink is followed,
    // its target's components are prepended so they are resolved in the
    // correct context (relative to `resolved` at that point).
    let mut queue: VecDeque<String> = split_path(&raw).into_iter().map(str::to_owned).collect();

    let mut resolved = PathBuf::from("/");
    // Track which symlink inodes we have already followed to detect loops.
    let mut seen_symlinks: HashSet<u64> = HashSet::new();
    while let Some(component) = queue.pop_front() {
        match component.as_str() {
            "." => continue,
            ".." => {
                // `PathBuf::pop` is a no-op at the root, which is the right
                // behaviour — we cannot go above `/`.
                resolved.pop();
            }
            seg => {
                let mut candidate = resolved.clone();
                candidate.push(seg);
                let candidate_str = path_to_unix_string(&candidate);

                let item = get_item(fs, &candidate_str)?;
                let inode_num = item.inode();
                let ftype = fs.get_inode(inode_num)?.get_file_type();

                match ftype {
                    FileType::Symlink => {
                        // `HashSet::insert` returns false if already present.
                        if !seen_symlinks.insert(inode_num) {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidInput,
                                format!("symbolic link loop detected at {}", candidate_str),
                            ));
                        }
                        let target = fs.read_symlink(inode_num)?;
                        if Path::new(&target).is_absolute() {
                            // Absolute symlink — restart resolution from root.
                            resolved = PathBuf::from("/");
                        }
                        // Prepend target components so they are processed next.
                        for tc in split_path(&target).into_iter().rev().map(str::to_owned) {
                            queue.push_front(tc);
                        }
                    }
                    FileType::Directory => {
                        resolved = candidate;
                    }
                    _ => {
                        resolved = candidate;
                        // Any remaining components after a non-directory are invalid.
                        if !queue.is_empty() {
                            return Err(io::Error::new(
                                io::ErrorKind::NotADirectory,
                                format!("{} is not a directory", candidate_str),
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(path_to_unix_string(&resolved))
}

// Path decomposition

/// Return the final component of `path` (equivalent to `basename`).
///
/// ```
/// assert_eq!(fname_from_path("/foo/bar/baz.txt"), "baz.txt");
/// assert_eq!(fname_from_path("plain"),            "plain");
/// ```
pub fn fname_from_path(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

/// Split `path` into its non-empty components, preserving `..` and `.`.
///
/// The leading `/` is stripped; `"/"` yields an empty [`Vec`].
///
/// ```
/// assert_eq!(split_path("/foo/bar/../baz"), vec!["foo", "bar", "..", "baz"]);
/// assert_eq!(split_path("/"),              Vec::<&str>::new());
/// ```
pub fn split_path(path: &str) -> Vec<&str> {
    Path::new(path)
        .components()
        .filter_map(|c| {
            match c {
                Component::Normal(name) => name.to_str(),
                Component::ParentDir => Some(".."),
                Component::CurDir => Some("."),
                _ => None,
            }
        })
        .collect()
}

// Tree navigation

/// Walk down the directory tree along every component of `abs_path` *except*
/// the last one, returning a shared reference to the [`Tree`] that directly
/// contains the final component.
///
/// Returns `Err` if:
/// - the filesystem has no tree,
/// - `abs_path` is the root (`/`) and therefore has no parent,
/// - any intermediate component is not found, or
/// - any intermediate component is not a directory.
///
/// # Example
/// ```
/// // Given /usr/local/bin:
/// let parent = find_parent_ref(fs, "/usr/local/bin")?;
/// // `parent` is the Tree for /usr/local — i.e. the one containing "bin".
/// ```
pub fn find_parent_ref<'fs>(fs: &'fs dyn FileSystem, abs_path: &str) -> io::Result<&'fs Tree> {
    let components = split_path(abs_path);

    // The root has no parent.
    if components.is_empty() {
        // Return the root tree
        return fs
            .get_tree()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"));
    }

    let mut current: &Tree = fs
        .get_tree()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    // Walk every component except the last.
    for &ph in &components[..components.len() - 1] {
        match current.iter().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                current = p.next().expect("directory item has no child tree");
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}", ph, abs_path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory", abs_path),
                ));
            }
        }
    }

    Ok(current)
}

/// Mutable variant of [`find_parent_ref`]: returns an exclusive reference to
/// the [`Tree`] that directly contains the final component of `abs_path`.
///
/// The borrow threads the `'fs` lifetime through each `next_mut()` call so
/// that only one level of the tree is mutably borrowed at a time — this is
/// safe under Rust's NLL rules as long as `Tree::next_mut` ties its output
/// lifetime to the tree's own data, not to `&mut self`.
pub fn find_parent_mut<'fs>(
    fs: &'fs mut dyn FileSystem,
    abs_path: &str,
) -> io::Result<&'fs mut Tree> {
    let components = split_path(abs_path);

    if components.is_empty() {
        return fs
            .get_tree_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"));
    }

    let mut current: &mut Tree = fs
        .get_tree_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    for &ph in &components[..components.len() - 1] {
        match current.iter_mut().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                current = p.next_mut().expect("directory item has no child tree");
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}", ph, abs_path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory", abs_path),
                ));
            }
        }
    }

    Ok(current)
}

/// Look up `abs_path` in the filesystem's in-memory directory tree and return
/// the corresponding [`Item`].
pub fn get_item(fs: &dyn FileSystem, abs_path: &str) -> io::Result<Item> {
    let tree = fs
        .get_tree()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    // If the components are empty, that tells us we are at the root.
    let components = match split_path(abs_path) {
        c if c.is_empty() => {
            // The root directory is represented by an empty component list, so
            // return the root item directly.
            return tree
                .iter()
                .find(|it| it.name() == ".")
                .cloned()
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::Other, "filesystem tree has no root item")
                });
        }
        c => c,
    };

    // Walk down to the parent directory.
    let mut current = tree;
    for &ph in &components[..components.len() - 1] {
        match current.iter().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                current = p.next().expect("directory item has no child tree");
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}.", ph, abs_path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory.", abs_path),
                ));
            }
        }
    }

    // Find the final component.
    let fname = fname_from_path(abs_path);
    current
        .iter()
        .find(|it| it.name() == fname)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: No such file or directory.", abs_path),
            )
        })
}

/// Remove the in-memory tree entry for `abs_path`.
///
/// Silently succeeds if the entry is already absent (mirrors `Vec::retain`
/// semantics).
pub fn remove_item(fs: &mut dyn FileSystem, abs_path: &str) -> io::Result<()> {
    let tree = fs
        .get_tree_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    let components = split_path(abs_path);
    let mut current = tree;

    for &ph in components[..components.len().saturating_sub(1)].iter() {
        match current.iter_mut().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                current = p.next_mut().expect("directory item has no child tree");
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}.", ph, abs_path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory.", abs_path),
                ));
            }
        }
    }

    let fname = fname_from_path(abs_path);
    current.retain(|item| item.name() != fname);
    Ok(())
}

/// Insert a new entry for `abs_path` pointing at `inode_num` into the
/// in-memory directory tree.
///
/// The parent directory must already be present in the tree.
pub fn add_item(fs: &mut dyn FileSystem, abs_path: &str, inode_num: u64) -> io::Result<()> {
    // Resolve the file type from the inode so callers don't have to pass it.
    let ftype = fs.get_inode(inode_num)?.get_file_type();

    let tree = fs
        .get_tree_mut()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    let components = split_path(abs_path);
    if components.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "cannot add a tree entry for the root directory",
        ));
    }

    let mut current = tree;
    for &ph in &components[..components.len() - 1] {
        match current.iter_mut().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                current = p.next_mut().expect("directory item has no child tree");
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}.", ph, abs_path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory.", abs_path),
                ));
            }
        }
    }

    let fname = fname_from_path(abs_path);
    current.push(Item::new(fname.to_string(), inode_num, ftype));
    Ok(())
}

/// A resolved pair of tree items: the parent directory and the target entry.
pub struct SplitItems {
    pub dir_part: Item,
    pub file_part: Item,
}

impl SplitItems {
    pub fn new(dir_part: Item, file_part: Item) -> Self {
        Self {
            dir_part,
            file_part,
        }
    }
}

/// Resolve `path` and return both the parent-directory item and the target
/// item as a [`SplitItems`].
///
/// The parent inode is read from the `.` pseudo-entry that every well-formed
/// Minix directory contains.  If `.` is absent (malformed image), the parent
/// is re-derived by walking from the root via [`get_item`].
pub fn split_items(fs: &dyn FileSystem, path: &str) -> io::Result<SplitItems> {
    let tree = fs
        .get_tree()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "filesystem has no tree"))?;

    let components = split_path(path);

    // Navigate down to the parent directory.
    let mut current = tree;
    for &ph in components[..components.len().saturating_sub(1)].iter() {
        match current.iter().find(|it| it.name() == ph) {
            Some(p) if p.is_dir() => {
                match p.next() {
                    Some(n) => {
                        current = n;
                    }
                    None => {
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!("directory {} has no child tree", ph),
                        ));
                    }
                }
            }
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{} is not a directory in path {}.", ph, path),
                ));
            }
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("{}: No such file or directory.", path),
                ));
            }
        }
    }

    let fname = components
        .last()
        .copied()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "empty path"))?;

    let file_part = current
        .iter()
        .find(|it| it.name() == fname)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{}: No such file or directory.", path),
            )
        })?;

    // The `.` entry in a directory always points to that directory's own
    // inode, giving us the parent item without a second traversal.
    // Fall back to an explicit root walk if the image is malformed and `.`
    // is absent.
    let dir_part = if let Some(dot) = current.iter().find(|it| it.name() == ".") {
        dot.clone()
    }
    else {
        let parent_path = Path::new(path)
            .parent()
            .map(path_to_unix_string)
            .unwrap_or_else(|| "/".to_string());
        get_item(fs, &parent_path)?
    };

    Ok(SplitItems::new(dir_part, file_part))
}
