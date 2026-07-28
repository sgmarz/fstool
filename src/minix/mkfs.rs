//! Make Minix 3 File System Routines
//!
//! © Stephen Marz
//! 8 June 2026
use super::consts::{
    BOOT_BLOCK_BYTES, DEFAULT_BLOCK_SIZE, DIR_ENTRY_BYTES, INODE_BYTES, SUPERBLOCK_BYTES,
    SUPERBLOCK_PADDING,
};
use super::{DirEntry, Inode, MinixFileSystem, Superblock};
use crate::{fs::MakeFileSystem, stat::S_IFDIR};
use std::{
    fs, io,
    io::Seek,
    io::Write,
    slice::from_raw_parts,
    time::{SystemTime, UNIX_EPOCH},
};

// Make 32M the minimum size for a Minix file system. If it is any smaller
// the placement of the blocks becomes compressed, and the math can potentially
// overlap them.
const MINIMUM_SIZE: u64 = 32 << 20;

impl MakeFileSystem for MinixFileSystem {
    fn mkfs(stream: &mut fs::File, size: u64) -> io::Result<()> {
        if size < MINIMUM_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "specified size is too small",
            ));
        }
        make_filesystem(stream, size)
    }
}

/// Utility for writing the superblock to a file
fn write_superblock(file: &mut fs::File, sb: &Superblock) -> io::Result<()> {
    let buffer = unsafe {
        from_raw_parts(
            (sb as *const Superblock) as *const u8,
            SUPERBLOCK_BYTES as usize,
        )
    };
    file.write_all(buffer)?;
    // The super block doesn't take the entire block, so we pad it out so the
    // Inode bitmap starts aligned on the block size.
    file.write_all(&vec![0u8; SUPERBLOCK_PADDING as usize])?;
    Ok(())
}

/// Write the Inode bitmap, which comes right after the SuperBlock aligned
/// on the block size.
fn write_imap(file: &mut fs::File, imap_blocks: u32) -> io::Result<()> {
    let imap_size = imap_blocks as usize * DEFAULT_BLOCK_SIZE as usize;
    // Take invalid inode 0 and root inode 1
    file.write_all(&[3u8])?;
    // Everything else is set to "not taken".
    file.write_all(&vec![0u8; imap_size - 1])?;
    Ok(())
}

/// Write the Zone bitmap, which comes after the Inode bitmap aligned on
/// the block size.
fn write_zmap(file: &mut fs::File, zmap_blocks: u32) -> io::Result<()> {
    let zmap_size = zmap_blocks as usize * DEFAULT_BLOCK_SIZE as usize;
    // Take invalid block 0 and root data block 1
    file.write_all(&[3u8])?;
    // Everything else is set to "not taken".
    file.write_all(&vec![0u8; zmap_size - 1])?;
    Ok(())
}

/// Write the root inode, which is inode #1 for the root directory.
fn write_root_inode(file: &mut fs::File, data_zone: u32) -> io::Result<()> {
    let tm = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as u32;
    let root_inode = Inode {
        mode: S_IFDIR | 0o755,
        nlinks: 2, // there are two links to the root which are . and ..
        uid: 0,
        gid: 0,
        size: DIR_ENTRY_BYTES as u32 * 2, // . and ..
        atime: tm,
        mtime: tm,
        ctime: tm,
        zones: [data_zone, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    let buffer = unsafe {
        from_raw_parts(
            (&root_inode as *const Inode) as *const u8,
            INODE_BYTES as usize,
        )
    };
    file.write_all(buffer)?;
    Ok(())
}

/// Write the root directory `/` and the `.` and `..` directories. This is
/// the smallest structure we can have in an 'empty' file system.
fn write_root_data(file: &mut fs::File) -> io::Result<()> {
    let dir_entries = [DirEntry::new(1, "."), DirEntry::new(1, "..")];
    for entry in &dir_entries {
        let buffer = unsafe {
            from_raw_parts(
                (entry as *const DirEntry) as *const u8,
                DIR_ENTRY_BYTES as usize,
            )
        };
        file.write_all(buffer)?;
    }
    Ok(())
}

/// Make a new filesystem on the given file with the given size (in bytes).
fn make_filesystem(file: &mut fs::File, size: u64) -> io::Result<()> {
    // Block size check was moved into individual make_filesystem functions because
    // some file systems, such as ExFAT have a smaller block size (512).
    if size & (DEFAULT_BLOCK_SIZE as u64 - 1) != 0 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "file size is not a multiple of the block size",
        ));
    }
    let size = size as u32;
    let num_blocks = size / DEFAULT_BLOCK_SIZE as u32;
    // 1 inode per 3 blocks is a common ratio for minix filesystems. This is actually just
    // a guess, but it seems to work for most situations when the file system is sufficiently
    // large.
    let num_inodes = num_blocks / 3;
    let imap_blocks =
        ((num_inodes + DEFAULT_BLOCK_SIZE as u32 * 8 - 1) / (DEFAULT_BLOCK_SIZE as u32 * 8)) as u32;
    let zmap_blocks =
        ((num_blocks + DEFAULT_BLOCK_SIZE as u32 * 8 - 1) / (DEFAULT_BLOCK_SIZE as u32 * 8)) as u32;
    let inode_blocks = (num_inodes * INODE_BYTES as u32 + DEFAULT_BLOCK_SIZE as u32 - 1)
        / DEFAULT_BLOCK_SIZE as u32;
    // 1 block for boot, 1 block for superblock, then the imap and zmap blocks, then the inode blocks
    let first_data_zone = 2 + imap_blocks + zmap_blocks + inode_blocks;

    // Skip the boot block, which is one block. The superblock will be written immediately after it, so we seek to the start of the superblock.
    file.seek(io::SeekFrom::Start(BOOT_BLOCK_BYTES))?;

    let superblock = Superblock::new_with(
        num_inodes,
        imap_blocks as u16,
        zmap_blocks as u16,
        first_data_zone as u16,
        num_blocks,
        DEFAULT_BLOCK_SIZE,
    );
    write_superblock(file, &superblock)?;
    write_imap(file, imap_blocks)?;
    write_zmap(file, zmap_blocks)?;
    write_root_inode(file, first_data_zone)?;
    file.seek(io::SeekFrom::Start(
        first_data_zone as u64 * DEFAULT_BLOCK_SIZE as u64,
    ))?;
    write_root_data(file)?;

    Ok(())
}
