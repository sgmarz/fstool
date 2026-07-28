//! Miscellaneous Commands, such as setuid, setgid, etc.
//!
//! © Stephen Marz
//! 8 June 2026
use super::{Args, State};
use crate::{
    cache::Item,
    fs::FileType,
    path::{
        add_item, build_path_deref, build_path_noderef, find_parent_mut, fname_from_path, get_item,
    },
    stat,
};
use chrono::Local;
use std::path::Path;

pub(super) fn do_uid(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("{}", state.uid);
        return;
    }
    let uid_result = match args[0].parse::<u32>() {
        Ok(u) => u,
        Err(_) => {
            println!("{}: could not convert UID to number.", &args[0]);
            return;
        }
    };
    state.uid = uid_result;
}

pub(super) fn do_gid(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("{}", state.gid);
        return;
    }
    let gid_result = match args[0].parse::<u32>() {
        Ok(g) => g,
        Err(_) => {
            println!("{}: could not convert GID to number.", &args[0]);
            return;
        }
    };
    state.gid = gid_result;
}

pub(super) fn do_umask(state: &mut State, args: Args) {
    if args.is_empty() {
        println!("{:<03o}", state.umask);
        return;
    }
    let mask_result = match u16::from_str_radix(&args[0], 8) {
        Ok(m) if m & !0o777 != 0 => {
            println!(
                "umask: invalid mask: {:o}. Mask must be an octal number.",
                m
            );
            return;
        }
        Ok(m) => m & 0o777,
        Err(_) => {
            println!(
                "umask: invalid mask: {}. Must be an octal number.",
                &args[0]
            );
            return;
        }
    };
    state.umask = mask_result;
}

pub(super) fn do_df(state: &mut State, _args: Args) {
    let sbo = state.fs.get_superblock();
    let inode_data = sbo.get_num_inodes();
    let block_data = sbo.get_num_blocks();
    assert!(inode_data.total() > 0);
    assert!(block_data.total() > 0);
    let inode_taken_pct = inode_data.taken as f64 * 100.0 / inode_data.total() as f64;
    let inode_free_pct = 100.0 - inode_taken_pct;
    let block_taken_pct = block_data.taken as f64 * 100.0 / block_data.total() as f64;
    let block_free_pct = 100.0 - block_taken_pct;
    println!("{} inodes.", inode_data.total());
    println!("   {} taken ({:.2}%).", inode_data.taken, inode_taken_pct);
    println!("   {} free ({:.2}%).", inode_data.free, inode_free_pct);
    println!(
        "{} blocks ({} total bytes / {} byte block size).",
        block_data.total(),
        sbo.get_block_size() * block_data.total(),
        sbo.get_block_size()
    );
    println!(
        "   {} taken ({} bytes) ({:.2}%).",
        block_data.taken,
        block_data.taken * sbo.get_block_size(),
        block_taken_pct
    );
    println!(
        "   {} free ({} bytes) ({:.2}%).",
        block_data.free,
        block_data.free * sbo.get_block_size(),
        block_free_pct
    );
}

pub(super) fn do_chmod(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.len() < 2 {
        println!("Usage: chmod <mode> <path>");
        println!("  mode: octal (e.g. 755) or");
        println!("    symbolic (e.g. u+x, a=rw, u+s,g-w)");
        println!("      symbolic modes can be comma-separated for multiple operations.");
        return;
    }

    // Resolve the target path, following symlinks.
    let fname = match build_path_deref(state.fs.as_mut(), &state.cwd, args[1]) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    // Look up the in-memory tree item.
    let item = match get_item(state.fs.as_ref(), &fname) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Read the current inode so we can derive the new mode from the old one
    // (needed for symbolic ops like '+' and '-').
    let (current_mode, is_dir) = match state.fs.get_inode(item.inode()) {
        Ok(inode) => {
            (
                inode.get_mode(),
                inode.get_file_type() == FileType::Directory,
            )
        }
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Parse the mode string (octal or symbolic).
    let new_perm_bits = match stat::parse_symbolic_mode(args[0], current_mode, is_dir) {
        Ok(bits) => bits,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Write back: preserve the file-type bits (S_IFMT), replace the lower 12.
    match state.fs.get_inode_mut(item.inode()) {
        Ok(inode) => inode.set_mode((current_mode & stat::S_IFMT) | new_perm_bits),
        Err(e) => {
            println!("{}", e);
            return;
        }
    }

    state.changed();
}

pub(super) fn do_chown(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.len() < 2 {
        println!("Usage: chown <owner>[:group] <path>");
        return;
    }
    // Parse the owner and group
    let owner_group: Vec<&str> = args[0].split(':').collect();
    if owner_group.len() > 2 {
        println!("Invalid owner/group format. Use <owner>:<group>.");
        return;
    }
    let owner_str = owner_group[0];
    let group_str = if owner_group.len() == 2 {
        Some(owner_group[1])
    }
    else {
        None
    };
    let owner_result = match owner_str.parse::<u32>() {
        Ok(o) => o,
        Err(_) => {
            println!("{}: could not convert owner to number.", owner_str);
            return;
        }
    };
    let fname = match build_path_deref(state.fs.as_mut(), &state.cwd, args[1]) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };
    let item = match get_item(state.fs.as_ref(), &fname) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    let inode = match state.fs.get_inode_mut(item.inode()) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if let Some(group) = group_str {
        match group.parse::<u32>() {
            Ok(g) => {
                inode.set_gid(g);
            }
            Err(_) => {
                println!("{}: could not convert group to number.", group);
                return;
            }
        };
    }
    inode.set_uid(owner_result);
    state.changed();
}

/// A minimal xorshift64 PRNG.  Not cryptographically secure, but more than
/// adequate for filling a file with non-repeating test data, and requires no
/// external crates.
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    /// Seed from the number of nanoseconds since the Unix epoch.  Falls back
    /// to a hard-coded odd constant if the system clock is unavailable.
    fn from_system_time() -> Self {
        let now = Local::now();
        let secs = now.timestamp() as u64;
        let nanos = now.timestamp_subsec_nanos() as u64;
        let seed = nanos ^ secs.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        Self {
            state: if seed == 0 {
                0xdead_beef_cafe_f00d
            }
            else {
                seed
            },
        }
    }

    /// Produce the next pseudo-random u64.
    #[inline]
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    /// Fill `buf` with pseudo-random bytes.
    fn fill(&mut self, buf: &mut [u8]) {
        let mut i = 0;
        while i + 8 <= buf.len() {
            buf[i..i + 8].copy_from_slice(&self.next_u64().to_le_bytes());
            i += 8;
        }
        // Handle the remaining 0-7 bytes.
        if i < buf.len() {
            let tail = self.next_u64().to_le_bytes();
            let end = buf.len() - i;
            buf[i..].copy_from_slice(&tail[..end]);
        }
    }
}

pub(super) fn do_randomb(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    // args[0] = number of bytes
    // args[1] = destination path
    if args.len() < 2 {
        println!("Usage: randbytes <size> <remote path>");
        return;
    }

    let byte_count: u64 = match args[0].parse() {
        Ok(n) => n,
        Err(_) => {
            println!("{}: invalid byte count.", args[0]);
            return;
        }
    };

    // Resolve or create the destination file
    let canonical = match build_path_deref(state.fs.as_mut(), &state.cwd, args[1]) {
        Ok(path) => {
            // File already exists — make sure it isn't a directory.
            match get_item(state.fs.as_ref(), &path) {
                Ok(item) if item.is_dir() => {
                    println!("{}: Is a directory.", args[1]);
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            }
            path
        }
        Err(_) => {
            // File does not exist — create it.

            // Raw absolute path (symlinks not yet resolved).
            let raw = build_path_noderef(&state.cwd, args[1]);
            let fname = fname_from_path(&raw).to_string();

            // Canonicalize the parent directory.
            let parent_raw = Path::new(&raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());

            let canonical_parent =
                match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };

            // Assemble the canonical absolute path for the new file.
            let canonical_abs = if canonical_parent == "/" {
                format!("/{}", fname)
            }
            else {
                format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
            };

            // Allocate the inode and write the disk-level dir entry.
            let new_inode_num = match state.fs.create(&canonical_abs, FileType::Regular) {
                Ok(i) => i,
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            };

            // Set inode metadata (empty for now; write_file sets size).
            let now = Local::now().timestamp() as u64;
            let mode = state.file_umask();
            {
                let inode = state.fs.get_inode_mut(new_inode_num).unwrap();
                inode.set_file_type(FileType::Regular);
                inode.set_mode(mode);
                inode.set_uid(state.uid);
                inode.set_gid(state.gid);
                inode.set_size(0);
                inode.set_atime(now);
                inode.set_mtime(now);
                inode.set_ctime(now);
                inode.set_nlinks(1);
            }

            // Insert the new entry into the in-memory directory tree.
            {
                let parent_tree = match find_parent_mut(state.fs.as_mut(), &canonical_abs) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };
                parent_tree.push(Item::new(fname, new_inode_num, FileType::Regular));
            }
            canonical_abs
        }
    };

    // Resolve the inode number for the (now-guaranteed-existing) file
    let inode_num = match get_item(state.fs.as_ref(), &canonical) {
        Ok(item) => item.inode(),
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // This grows or shrinks the file, allocating / freeing blocks as needed.
    // If byte_count happens to equal the current size, truncate is a no-op.
    if let Err(e) = state.fs.truncate(inode_num, byte_count) {
        println!("{}", e);
        return;
    }

    // Generate the full buffer in one allocation so write_file gets a single
    // contiguous slice and can handle block boundaries internally.
    if byte_count > 0 {
        let mut rng = Xorshift64::from_system_time();
        let mut buf = vec![0u8; byte_count as usize];
        rng.fill(&mut buf);

        if let Err(e) = state.fs.write_file(inode_num, 0, &buf) {
            println!("{}", e);
            return;
        }
    }

    // Update timestamps
    let now = Local::now().timestamp() as u64;
    if let Ok(inode) = state.fs.get_inode_mut(inode_num) {
        inode.set_mtime(now);
        inode.set_ctime(now);
    }

    state.changed();
}

pub(super) fn do_randomt(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    // args[0] = number of bytes   args[1] = destination path
    if args.len() < 2 {
        println!("Usage: randtext <size> <remote path>");
        return;
    }

    // Parse byte count
    let byte_count: u64 = match args[0].parse() {
        Ok(n) => n,
        Err(_) => {
            println!("{}: invalid byte count.", args[0]);
            return;
        }
    };

    // Resolve or create the destination file
    let canonical = match build_path_deref(state.fs.as_mut(), &state.cwd, args[1]) {
        Ok(path) => {
            // File already exists — reject directories.
            match get_item(state.fs.as_ref(), &path) {
                Ok(item) if item.is_dir() => {
                    println!("{}: Is a directory.", args[1]);
                    return;
                }
                Ok(_) => {}
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            }
            path
        }
        Err(_) => {
            // File does not exist — create it (mirrors do_touch).
            let raw = build_path_noderef(&state.cwd, args[1]);
            let fname = fname_from_path(&raw).to_string();

            let parent_raw = Path::new(&raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string());

            let canonical_parent =
                match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };

            let canonical_abs = if canonical_parent == "/" {
                format!("/{}", fname)
            }
            else {
                format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
            };

            let new_inode_num = match state.fs.create(&canonical_abs, FileType::Regular) {
                Ok(i) => i,
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            };

            let now = Local::now().timestamp() as u64;
            let mode = state.file_umask();
            {
                let inode = state.fs.get_inode_mut(new_inode_num).unwrap();
                inode.set_file_type(FileType::Regular);
                inode.set_mode(mode);
                inode.set_uid(state.uid);
                inode.set_gid(state.gid);
                inode.set_size(0);
                inode.set_atime(now);
                inode.set_mtime(now);
                inode.set_ctime(now);
                inode.set_nlinks(1);
            }
            {
                let parent_tree = match find_parent_mut(state.fs.as_mut(), &canonical_abs) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };
                parent_tree.push(Item::new(fname, new_inode_num, FileType::Regular));
            }

            canonical_abs
        }
    };

    // Resolve the inode number
    let inode_num = match get_item(state.fs.as_ref(), &canonical) {
        Ok(item) => item.inode(),
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Truncate to the requested size
    if let Err(e) = state.fs.truncate(inode_num, byte_count) {
        println!("{}", e);
        return;
    }

    // Generate random printable ASCII text and write
    if byte_count > 0 {
        const ALPHABET: &[u8] = b" !\"#$%&'()*+,-./0123456789:;<=>?@\
              ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`\
              abcdefghijklmnopqrstuvwxyz{|}~\n";

        let mut rng = Xorshift64::from_system_time();
        let mut buf = vec![0u8; byte_count as usize];

        let mut i = 0;
        while i < buf.len() {
            let rnd = rng.next_u64();
            for shift in [0u32, 8, 16, 24, 32, 40, 48, 56] {
                if i >= buf.len() {
                    break;
                }
                let b = ((rnd >> shift) & 0xFF) as usize;
                buf[i] = ALPHABET[b % ALPHABET.len()];
                i += 1;
            }
        }

        if let Err(e) = state.fs.write_file(inode_num, 0, &buf) {
            println!("{}", e);
            return;
        }
    }

    // Update timestamps
    let now = Local::now().timestamp() as u64;
    if let Ok(inode) = state.fs.get_inode_mut(inode_num) {
        inode.set_mtime(now);
        inode.set_ctime(now);
    }

    state.changed();
}

pub(super) fn do_mknod(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    fn mknod_usage() {
        println!("Usage: mknod <name> <type> <major> <minor>");
        println!("  type: b (block device); c (character device); p (pipe/FIFO); s (socket)");
    }
    if args.len() < 2 {
        mknod_usage();
        return;
    }

    // ── Parse type ────────────────────────────────────────────────────────────
    let ftype = match args[1] {
        "b" | "block" => FileType::BlockDevice,
        "c" | "char" => FileType::CharacterDevice,
        "p" | "pipe" | "fifo" => FileType::Fifo,
        "s" | "socket" => FileType::Socket,
        other => {
            println!(
                "{}: invalid node type. Use 'b' (block), 'c' (char), 'p' (pipe/FIFO), or 's' (socket).",
                other
            );
            return;
        }
    };

    if ftype == FileType::BlockDevice || ftype == FileType::CharacterDevice {
        if args.len() < 4 {
            mknod_usage();
            return;
        }
    }

    // ── Parse major / minor ───────────────────────────────────────────────────
    let major: u16 = if ftype == FileType::BlockDevice || ftype == FileType::CharacterDevice {
        match args[2].parse() {
            Ok(n) => n,
            Err(_) => {
                println!("{}: invalid major number.", args[2]);
                return;
            }
        }
    }
    else {
        0
    };
    let minor: u16 = if ftype == FileType::BlockDevice || ftype == FileType::CharacterDevice {
        match args[3].parse() {
            Ok(n) => n,
            Err(_) => {
                println!("{}: invalid minor number.", args[3]);
                return;
            }
        }
    }
    else {
        0
    };

    // ── Resolve or build the canonical destination path ───────────────────────
    if build_path_deref(state.fs.as_mut(), &state.cwd, args[0]).is_ok() {
        println!("{}: File exists.", args[0]);
        return;
    }

    let raw = build_path_noderef(&state.cwd, args[0]);
    let fname = fname_from_path(&raw).to_string();

    let parent_raw = Path::new(&raw)
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());

    let canonical_parent = match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
        Ok(p) => p,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    let canonical_abs = if canonical_parent == "/" {
        format!("/{}", fname)
    }
    else {
        format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
    };

    // Allocate the inode and disk-level directory entry
    // fs.create dispatches to create_node internally, but always uses major=0,
    // minor=0.  We patch zone[0] with the real device number immediately after.
    let new_inode_num = match state.fs.create(&canonical_abs, ftype) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };

    // Set inode metadata
    let now = Local::now().timestamp() as u64;
    {
        let mode = state.file_umask();
        let inode = match state.fs.get_inode_mut(new_inode_num) {
            Ok(i) => i,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        inode.set_file_type(ftype);
        inode.set_mode(mode);
        inode.set_uid(state.uid);
        inode.set_gid(state.gid);
        inode.set_size(0);
        inode.set_atime(now);
        inode.set_mtime(now);
        inode.set_ctime(now);
        inode.set_nlinks(1);
        inode.set_node(major, minor);
    }

    // Insert the new entry into the in-memory directory tree
    {
        let parent_tree = match find_parent_mut(state.fs.as_mut(), &canonical_abs) {
            Ok(t) => t,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        parent_tree.push(Item::new(fname, new_inode_num, ftype));
    }

    state.changed();
}

fn make_symlink(state: &mut State, target: &str, link: &str) {
    let link = link.to_string();
    let fname = fname_from_path(&link).to_string();

    let new_inode_num = match state.fs.create(&link, FileType::Symlink) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    // Set inode metadata.
    {
        let now = Local::now().timestamp() as u64;
        let inode = state.fs.get_inode_mut(new_inode_num).unwrap();
        inode.set_file_type(FileType::Symlink);
        // Symlinks typically have 777 permissions; the target's permissions govern access.
        inode.set_mode(0o777);
        inode.set_uid(state.uid);
        inode.set_gid(state.gid);
        inode.set_size(target.len() as u64);
        inode.set_atime(now);
        inode.set_mtime(now);
        inode.set_ctime(now);
        inode.set_nlinks(1);
    }

    // Allocate the block. For symlinks, one block should be enough to hold the target path.
    let block_num = match state.fs.get_superblock_mut().allocate_block() {
        Ok(b) => b,
        Err(e) => {
            let _ = state.fs.unlink(&link);
            println!("{}", e);
            return;
        }
    };
    if let Err(e) = state.fs.write_block(block_num, target.as_bytes()) {
        let _ = state.fs.unlink(&link);
        println!("{}", e);
        return;
    }
    // Update the inode to point to the block.
    {
        let inode = state.fs.get_inode_mut(new_inode_num).unwrap();
        let mut blocks = inode.get_blocks();
        blocks[0] = block_num;
        inode.set_blocks(&blocks);
    }

    // Insert the new entry into the in-memory directory tree.
    let parent_tree = match find_parent_mut(state.fs.as_mut(), &link) {
        Ok(t) => t,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    parent_tree.push(Item::new_symlink(fname, new_inode_num, target.to_string()));
}

fn make_hardlink(state: &mut State, target: &str, link: &str) {
    let item = match get_item(state.fs.as_ref(), target) {
        Ok(i) => i,
        Err(e) => {
            println!("{}", e);
            return;
        }
    };
    if item.is_dir() {
        println!("{}: Cannot hard link to a directory.", target);
        return;
    }
    let inode_num = item.inode();
    {
        let inode = match state.fs.get_inode_mut(inode_num) {
            Ok(i) => i,
            Err(e) => {
                println!("{}", e);
                return;
            }
        };
        let now = Local::now().timestamp() as u64;
        // Increment the link count on the target inode.
        let nlinks = inode.get_nlinks();
        inode.set_nlinks(nlinks + 1);
        // Update the mtime and ctime on the target inode.
        inode.set_mtime(now);
        inode.set_ctime(now);
    }
    // Create the link in the file system.
    match state.fs.link(inode_num, &String::from(link)) {
        Ok(_) => {}
        Err(e) => {
            // Roll back the link count increment on failure.
            if let Ok(inode) = state.fs.get_inode_mut(inode_num) {
                let nlinks = inode.get_nlinks();
                inode.set_nlinks(nlinks - 1);
            }
            println!("{}", e);
            return;
        }
    }

    // Create the link in the in-memory directory tree.
    match add_item(state.fs.as_mut(), &link, inode_num) {
        Ok(()) => (),
        Err(e) => {
            // Roll back the link count increment on failure.
            if let Ok(inode) = state.fs.get_inode_mut(inode_num) {
                let nlinks = inode.get_nlinks();
                inode.set_nlinks(nlinks - 1);
            }
            println!("{}", e);
            return;
        }
    }
}

pub(super) fn do_ln(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    let (flag_args, path_args) = args
        .iter()
        .partition::<Vec<&str>, _>(|f| f.starts_with('-'));
    if path_args.len() != 2 {
        println!("Usage: ln -[sf] <target path> <link path>");
        return;
    }
    let soft = flag_args.iter().any(|f| f.contains('s'));
    let force = flag_args.iter().any(|f| f.contains('f'));
    let target = path_args[0];
    let link = path_args[1];

    match build_path_deref(state.fs.as_mut(), &state.cwd, link) {
        Ok(canonical) => {
            // File already exists: force it?
            if !force {
                println!("{}: File exists.", link);
                return;
            }
            // We want it forced. First, get rid of the old one.
            let _ = state.fs.unlink(&canonical);
            if soft {
                make_symlink(state, target, &canonical);
            }
            else {
                make_hardlink(state, target, &canonical);
            }
        }

        Err(_) => {
            // File does not exist
            // Step 1 — Build the raw absolute path (symlinks not yet resolved).
            let raw = build_path_noderef(&state.cwd, link);
            let fname = fname_from_path(&raw).to_string();

            // Step 2 — Canonicalize the parent directory.
            //          Path::parent() gives us the directory portion without
            //          any manual index arithmetic.
            let parent_raw = Path::new(&raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or("/".to_string());

            let canonical_parent =
                match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };

            // Step 3 — Assemble the canonical absolute path for the new file.
            let canonical_abs = if canonical_parent == "/" {
                format!("/{}", fname)
            }
            else {
                format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
            };
            if soft {
                make_symlink(state, target, &canonical_abs);
            }
            else {
                make_hardlink(state, target, &canonical_abs);
            }
        }
    }

    state.changed();
}

pub(super) fn do_touch(state: &mut State, args: Args) {
    if !state.write {
        println!("Filesystem is read-only.");
        return;
    }
    if args.len() != 1 {
        println!("Usage: touch <file>");
        return;
    }

    let now = Local::now().timestamp() as u64;

    match build_path_deref(state.fs.as_mut(), &state.cwd, args[0]) {
        Ok(canonical) => {
            // ── File already exists: update timestamps only ────────────────
            let item = match get_item(state.fs.as_ref(), &canonical) {
                Ok(i) => i,
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            };
            match state.fs.get_inode_mut(item.inode()) {
                Ok(inode) => {
                    inode.set_atime(now);
                    inode.set_mtime(now);
                    // ctime changes whenever metadata changes (POSIX).
                    inode.set_ctime(now);
                }
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            }
        }

        Err(_) => {
            // ── File does not exist: create an empty regular file ──────────

            // Step 1 — Build the raw absolute path (symlinks not yet resolved).
            let raw = build_path_noderef(&state.cwd, args[0]);
            let fname = fname_from_path(&raw).to_string();

            // Step 2 — Canonicalize the parent directory.
            //          Path::parent() gives us the directory portion without
            //          any manual index arithmetic.
            let parent_raw = Path::new(&raw)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or("/".to_string());

            let canonical_parent =
                match build_path_deref(state.fs.as_mut(), &state.cwd, &parent_raw) {
                    Ok(p) => p,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };

            // Step 3 — Assemble the canonical absolute path for the new file.
            let canonical_abs = if canonical_parent == "/" {
                format!("/{}", fname)
            }
            else {
                format!("{}/{}", canonical_parent.trim_end_matches('/'), fname)
            };

            // Step 4 — Allocate the inode and write the disk-level dir entry.
            let new_inode_num = match state.fs.create(&canonical_abs, FileType::Regular) {
                Ok(i) => i,
                Err(e) => {
                    println!("{}", e);
                    return;
                }
            };

            // Step 5 — Set inode metadata.
            let mode = state.file_umask();
            {
                let inode = state.fs.get_inode_mut(new_inode_num).unwrap();
                inode.set_file_type(FileType::Regular);
                inode.set_mode(mode);
                inode.set_uid(state.uid);
                inode.set_gid(state.gid);
                inode.set_size(0);
                inode.set_atime(now);
                inode.set_mtime(now);
                inode.set_ctime(now);
                inode.set_nlinks(1);
            }

            // Step 6 — Insert the new entry into the in-memory directory tree.
            //
            // The block scope ensures the mutable borrow of state.fs ends
            // before state.changed() uses it.
            {
                let parent_tree = match find_parent_mut(state.fs.as_mut(), &canonical_abs) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("{}", e);
                        return;
                    }
                };
                parent_tree.push(Item::new_file(fname, new_inode_num));
            }
        }
    }

    state.changed();
}

pub(super) fn do_remount_ro(state: &mut State, _args: Args) {
    if !state.write {
        println!("Filesystem is already read-only.");
        return;
    }
    if state.changes {
        println!("Cannot remount read-only: there are unsaved changes.");
        return;
    }
    state.write = false;
    println!("Filesystem remounted read-only.");
}
