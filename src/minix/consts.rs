//! Minix 3 File System Constants
//!
//! © Stephen Marz
//! 8 June 2026
use super::{DirEntry, Inode, Superblock};
use std::mem::size_of;

/// Constants for the Minix 3 file system.
pub const DEFAULT_BLOCK_SIZE: u16 = 1024;
pub const BOOT_BLOCK_BYTES: u64 = 1024;
pub const SUPERBLOCK_BYTES: u64 = size_of::<Superblock>() as u64;
pub const SUPERBLOCK_PADDING: u64 = BOOT_BLOCK_BYTES - SUPERBLOCK_BYTES;
pub const ROOT_INODE: u64 = 1;
pub const INODE_BYTES: u64 = size_of::<Inode>() as u64;
pub const NUM_ZONE_POINTERS: usize = 10;
pub const ZONE_POINTER_BYTES: usize = 4;
pub type ZonePointerType = u32;
pub const INDIRECT_ZONE: usize = NUM_ZONE_POINTERS - 3;
pub const DINDIRECT_ZONE: usize = NUM_ZONE_POINTERS - 2;
pub const TINDIRECT_ZONE: usize = NUM_ZONE_POINTERS - 1;
pub const DIR_ENTRY_NAME_SIZE: usize = 60;
pub const DIR_ENTRY_BYTES: u64 = size_of::<DirEntry>() as u64;
// pub const DIR_ENTRIES_PER_BLOCK: u64 = BLOCK_SIZE / DIR_ENTRY_BYTES;
pub const MAX_FILE_SIZE: u32 = (1 << 31) - 1;
pub const DEFAULT_DISK_VERSION: u8 = 0;

pub const MINIX_MAGIC: u16 = 0x4d5a;

pub const MAX_SYMLINK_SIZE: usize = 64;

pub const CACHE_LINES: usize = 20;
