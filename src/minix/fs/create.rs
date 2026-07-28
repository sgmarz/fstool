//! Minix 3 File System Creation Routines
//!
//! © Stephen Marz
//! 8 June 2026
use crate::fs::{FileSystem, FileType};
use crate::minix::{MinixFileSystem, consts, consts::DIR_ENTRY_BYTES, direntry::DirEntry};
use crate::path::{fname_from_path, get_item, split_items, split_path};
use crate::stat;
use std::io::{self, Error, ErrorKind};

/// Create a new file, directory, symlink, device node, fifo, or socket at the specified
/// absolute path. The path is needed to create the directory entry to link the new node
/// into the filesystem. The new node is allocated on disk, not in the in-memory tree.
pub fn create(fs: &mut MinixFileSystem, abs_path: &String, ftype: FileType) -> io::Result<u64> {
    let inode_num = match ftype {
        FileType::Directory => create_dir(fs, abs_path)?,
        FileType::Regular => create_file(fs)?,
        FileType::Symlink => create_symlink(fs)?,
        FileType::BlockDevice | FileType::CharacterDevice => {
            create_node(fs, abs_path, ftype, 0, 0)?
        }
        FileType::Fifo => create_fifo(fs)?,
        FileType::Socket => create_socket(fs)?,
        _ => {
            return Err(Error::new(
                ErrorKind::InvalidInput,
                format!("invalid file type: {:?}", ftype),
            ));
        }
    };
    create_dentry(fs, inode_num, abs_path)?;
    Ok(inode_num)
}

/// Allocates a fresh inode, sets its mode, link count, and initialises size
/// to 0.  Returns the new inode number.
fn alloc_inode(fs: &mut MinixFileSystem, mode: u16, nlinks: u16) -> io::Result<u64> {
    let inode_num = fs.get_superblock_mut().allocate_inode()?;
    let inode = fs.get_inode_mut(inode_num)?;
    inode.set_mode(mode);
    inode.set_nlinks(nlinks as u32);
    inode.set_size(0);
    Ok(inode_num)
}

/// Creates a new directory at the specified absolute path.  Returns the new
/// inode number on success.  The caller is responsible for ensuring the inode is
/// allocated and initialized before calling create_dentry to link it into the tree.
fn create_dir(fs: &mut MinixFileSystem, abs_path: &String) -> io::Result<u64> {
    // rwxr-xr-x  (callers can chmod later)
    let mode = stat::S_IFDIR | 0o755;

    // Resolve the parent inode *before* allocating anything so that a missing
    // parent fails cheaply.
    let sequence = split_path(abs_path);
    let parent_inode_num: u64 = if sequence.len() > 1 {
        let parent_path = sequence[..sequence.len() - 1].join("/");
        get_item(fs, &parent_path)?.inode()
    }
    else {
        // Direct child of root — find the root through the "." entry.
        fs.tree
            .iter()
            .find(|it| it.name() == ".")
            .map(|it| it.inode())
            .ok_or_else(|| Error::new(ErrorKind::NotFound, "root inode not found"))?
    };

    // nlinks starts at 2: one for its entry in the parent, one for ".".
    let inode_num = alloc_inode(fs, mode, 2)?;

    // Allocate one data block to hold "." and "..".
    let block_num = fs.get_superblock_mut().allocate_block()?;
    let block_size = fs.get_superblock().get_block_size() as usize;
    let mut bdata = vec![0u8; block_size];

    // "." — points to this directory itself.
    if let Some(dot) = DirEntry::from_bytes_mut(&mut bdata[..]) {
        dot.inode = inode_num as u32;
        dot.name = [0; consts::DIR_ENTRY_NAME_SIZE];
        dot.name[0] = b'.';
    }

    // ".." — points to the parent directory.
    if let Some(dotdot) = DirEntry::from_bytes_mut(&mut bdata[DIR_ENTRY_BYTES as usize..]) {
        dotdot.inode = parent_inode_num as u32;
        dotdot.name = [0; consts::DIR_ENTRY_NAME_SIZE];
        dotdot.name[0] = b'.';
        dotdot.name[1] = b'.';
    }

    fs.write_block(block_num, &bdata)?;

    // Wire the block into the new inode and set the directory size.
    {
        let inode = fs.get_inode_mut(inode_num)?;
        let mut inode_blocks = inode.get_blocks();
        inode_blocks[0] = block_num;
        inode.set_blocks(&inode_blocks);
        inode.set_size((2 * DIR_ENTRY_BYTES) as u64);
    }

    // Every new subdirectory's ".." entry adds a hard link to the parent.
    {
        let parent_nlinks = fs.get_inode(parent_inode_num)?.get_nlinks();
        fs.get_inode_mut(parent_inode_num)?
            .set_nlinks(parent_nlinks + 1);
    }

    Ok(inode_num)
}

fn create_file(fs: &mut MinixFileSystem) -> io::Result<u64> {
    // rw-r--r--
    let mode = stat::S_IFREG | 0o644;
    let inode_num = alloc_inode(fs, mode, 1)?;
    Ok(inode_num)
}

fn create_symlink(fs: &mut MinixFileSystem) -> io::Result<u64> {
    // Symlinks conventionally carry 0o777; the target's permissions govern
    // real access control.
    let mode = stat::S_IFLNK | 0o777;
    let inode_num = alloc_inode(fs, mode, 1)?;
    Ok(inode_num)
}

fn create_node(
    fs: &mut MinixFileSystem,
    abs_path: &String,
    node_type: FileType,
    major: u16,
    minor: u16,
) -> io::Result<u64> {
    assert!(node_type == FileType::BlockDevice || node_type == FileType::CharacterDevice);
    let mode = match node_type {
        FileType::BlockDevice => stat::S_IFBLK | 0o660,
        FileType::CharacterDevice => stat::S_IFCHR | 0o660,
        _ => unreachable!(),
    };
    let inode_num = alloc_inode(fs, mode, 1)?;
    // In Minix3 the device number is packed into zone[0]:
    //   bits 15..8 = major, bits 7..0 = minor.
    let dev = ((major as u64) << 8) | (minor as u64);
    let inode = fs.get_inode_mut(inode_num)?;
    let mut inode_blocks = inode.get_blocks();
    inode_blocks[0] = dev;
    inode.set_blocks(&inode_blocks);
    create_dentry(fs, inode_num, abs_path)?;
    Ok(inode_num)
}

fn create_fifo(fs: &mut MinixFileSystem) -> io::Result<u64> {
    let mode = stat::S_IFIFO | 0o644;
    let inode_num = alloc_inode(fs, mode, 1)?;
    Ok(inode_num)
}

fn create_socket(fs: &mut MinixFileSystem) -> io::Result<u64> {
    let mode = stat::S_IFSOCK | 0o600;
    let inode_num = alloc_inode(fs, mode, 1)?;
    Ok(inode_num)
}

/// Creates a directory entry for the given inode at the specified absolute path.
/// Returns the inode number on success.  The caller is responsible for ensuring the
/// inode is allocated and initialized before calling this function.
pub fn create_dentry(fs: &mut MinixFileSystem, inode: u64, abs_path: &String) -> io::Result<u64> {
    // Verify the inode is actually allocated.
    if let Ok(val) = fs.imap.is_set(inode as usize) {
        if !val {
            return Err(Error::new(
                ErrorKind::NotConnected,
                format!("inode {} is not set as taken.", inode),
            ));
        }
    }
    else {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("inode {} is out of bounds.", inode),
        ));
    }

    // Fail fast if the path already exists in the tree.
    if split_items(fs, abs_path).is_ok() {
        return Err(Error::new(ErrorKind::AlreadyExists, "entry already exists"));
    }

    let sequence = split_path(abs_path);
    if sequence.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "cannot create dentry for root directory",
        ));
    }

    let fname = fname_from_path(abs_path);
    let fname_len = fname.len();

    // Validate length early so we don't do unnecessary disk I/O.
    if fname_len > consts::DIR_ENTRY_NAME_SIZE {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!(
                "filename {} is too long. Max length is {}",
                fname,
                consts::DIR_ENTRY_NAME_SIZE
            ),
        ));
    }

    // Locate the parent directory tree item.
    let dir = if sequence.len() > 1 {
        get_item(fs, &sequence[..sequence.len() - 1].join("/"))?
    }
    else {
        if let Some(p) = fs.tree.iter().find(|it| it.name().eq(".")) {
            p.clone()
        }
        else {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("{}: No such file or directory.", abs_path),
            ));
        }
    };

    let dir_inode_num = dir.inode();
    let block_size = fs.get_superblock().get_block_size() as usize;
    let blocks = fs.get_inode(dir_inode_num)?.get_blocks();
    let mut bdata = vec![0u8; block_size];

    // Scan existing direct blocks for a free slot (inode == 0).
    for &blnum in blocks[..consts::INDIRECT_ZONE as usize]
        .iter()
        .filter(|&&b| b != 0)
    {
        fs.read_block(blnum, &mut bdata)?;
        for d in (0..block_size).step_by(DIR_ENTRY_BYTES as usize) {
            if let Some(dent) = DirEntry::from_bytes_mut(&mut bdata[d..]) {
                if dent.inode == 0 {
                    dent.inode = inode as u32;
                    // Zero the name field entirely before writing so there are
                    // no leftover bytes from a previously deleted entry.
                    dent.name = [0; consts::DIR_ENTRY_NAME_SIZE];
                    dent.name[..fname_len].copy_from_slice(fname.as_bytes());
                    fs.write_block(blnum, &bdata)?;
                    // Update the directory inode's size to account for the new entry.
                    let dir_entry_size = consts::DIR_ENTRY_BYTES as u64;
                    let dir_inode_mut = fs.get_inode_mut(dir_inode_num)?;
                    let old_size = dir_inode_mut.get_size();
                    dir_inode_mut.set_size(old_size + dir_entry_size);
                    return Ok(inode);
                }
            }
        }
    }

    // No free slot found.  If there is an unused direct-zone
    // pointer in the parent inode, allocate a fresh block and put the
    // new entry at position 0.
    let free_zone_slot = blocks[..consts::INDIRECT_ZONE as usize]
        .iter()
        .position(|&b| b == 0);

    if let Some(slot) = free_zone_slot {
        let new_block = fs.get_superblock_mut().allocate_block()?;

        // Initialise the new block to all-zero, then write our entry first.
        bdata.iter_mut().for_each(|b| *b = 0);
        if let Some(dent) = DirEntry::from_bytes_mut(&mut bdata[..]) {
            dent.inode = inode as u32;
            dent.name = [0; consts::DIR_ENTRY_NAME_SIZE];
            dent.name[..fname_len].copy_from_slice(fname.as_bytes());
        }

        fs.write_block(new_block, &bdata)?;

        // Point the parent inode at the new block and grow its size.
        {
            let dir_entry_size = consts::DIR_ENTRY_BYTES as u64;
            let dir_inode_mut = fs.get_inode_mut(dir_inode_num)?;
            let mut dir_inode_blocks = dir_inode_mut.get_blocks();
            dir_inode_blocks[slot] = new_block;
            dir_inode_mut.set_blocks(&dir_inode_blocks);
            let old_size = dir_inode_mut.get_size();
            dir_inode_mut.set_size(old_size + dir_entry_size);
        }

        return Ok(inode);
    }

    // All direct zones are occupied and we don't yet handle indirect blocks
    // for the parent directory here.
    Err(Error::new(
        ErrorKind::Other,
        "no free direntry slot and no unused direct zone in parent directory",
    ))
}
