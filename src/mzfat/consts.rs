//! MzFAT Constants
//!
//! © Stephen Marz
//! 8 June 2026

pub const MZFAT_MAGIC: u16 = 0xF4A2;

pub const BLOCKS_PER_CLUSTER: u64 = 4;

pub const SUPERBLOCK_SIZE: usize = 16;

pub const FAT_EOC: u32 = 0xFFFF_FFFF;

pub const DEFAULT_BLOCK_SIZE: u64 = 4096;

pub const DIR_ENTRY_SIZE: usize = 32;
