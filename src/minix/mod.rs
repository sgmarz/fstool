//! Minix 3 Filesystem Module
//!
//! © Stephen Marz
//! 8 June 2026
pub mod consts;
pub mod direntry;
pub mod fs;
pub mod inode;
pub mod mkfs;
pub mod superblock;

use crate::fs::FileSystem;

// Exports
pub use super::bitmap::Bitmap;
pub use direntry::DirEntry;
pub use fs::MinixFileSystem;
pub use fs::check_valid;
pub use inode::Inode;
pub use superblock::Superblock;

use std::io;

pub(in crate::minix) fn get_indirect_zone(
    fs: &mut MinixFileSystem,
    zone: u64,
) -> io::Result<Vec<consts::ZonePointerType>> {
    let mut buf = vec![0u8; fs.superblock.block_size as usize];
    fs.read_block(zone, &mut buf)?;

    Ok(buf
        .chunks_exact(consts::ZONE_POINTER_BYTES as usize)
        .map(|chunk| chunk.iter().rfold(0, |acc, &b| (acc << 8) | b as u32))
        .collect())
}
