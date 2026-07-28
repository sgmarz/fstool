//! mzFAT Super block
//!
//! © Stephen Marz
//! 8 June 2026
use std::slice::from_raw_parts;

use super::consts;
use crate::fs::{AllocationData, SuperblockOperations};
use std::io;

#[repr(C, packed)]
#[derive(Debug, Default)]
pub struct Superblock {
    pub block_shift: u8,
    pub fat_block_offset: u8,
    pub fat_num_blocks: u16,
    pub bitmap_block_offset: u16,
    pub bitmap_num_blocks: u16,
    pub root_block_offset: u16,
    pub num_data_blocks: u32,
    pub magic: u16,
}

impl Superblock {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        debug_assert!(bytes.len() == consts::SUPERBLOCK_SIZE);
        unsafe { std::ptr::read(bytes.as_ptr() as *const Superblock) }
    }

    pub fn to_bytes(&self) -> &[u8] {
        unsafe {
            from_raw_parts(
                self as *const Superblock as *const u8,
                consts::SUPERBLOCK_SIZE,
            )
        }
    }
}

impl SuperblockOperations for Superblock {
    fn allocate_block(&mut self) -> io::Result<u64> {
        todo!();
    }

    fn allocate_inode(&mut self) -> io::Result<u64> {
        todo!();
    }

    fn deallocate_block(&mut self, block_num: u64) -> io::Result<()> {
        todo!();
    }

    fn deallocate_inode(&mut self, inode_num: u64) -> io::Result<()> {
        todo!();
    }

    fn get_block_size(&self) -> u64 {
        1_u64 << self.block_shift
    }

    fn get_num_blocks(&self) -> AllocationData {
        todo!();
    }

    fn get_num_inodes(&self) -> AllocationData {
        AllocationData { free: 0, taken: 0 }
    }
}
