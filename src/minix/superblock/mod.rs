//! Minix 3 Superblock
//!
//! © Stephen Marz
//! 8 June 2026
use super::MinixFileSystem;
use crate::fs::{AllocationData, SuperblockOperations};
use std::io;

#[repr(C)]
pub struct Superblock {
    pub num_inodes: u32,
    pad0: u16,
    pub imap_blocks: u16,
    pub zmap_blocks: u16,
    pub first_data_zone: u16,
    pub log_zone_size: u16,
    pad1: u16,
    pub max_size: u32,
    pub num_zones: u32,
    pub magic: u16,
    pad2: u16,
    pub block_size: u16,
    pub disk_version: u8,
}

pub mod debug;
pub mod support;

impl SuperblockOperations for MinixFileSystem {
    fn get_num_inodes(&self) -> AllocationData {
        let taken_blocks = self.imap.count_taken() as u64;
        let free_blocks = self.imap.count_free() as u64;
        AllocationData::new(taken_blocks, free_blocks)
    }

    fn get_num_blocks(&self) -> AllocationData {
        let taken_blocks = self.zmap.count_taken() as u64;
        let free_blocks = self.zmap.count_free() as u64;
        AllocationData::new(taken_blocks, free_blocks)
    }

    fn get_block_size(&self) -> u64 {
        self.superblock.block_size as u64
    }

    fn allocate_inode(&mut self) -> io::Result<u64> {
        match self.imap.take_next() {
            Some(t) => Ok(t as u64),
            None => {
                Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "no inodes left to allocate",
                ))
            }
        }
    }

    fn deallocate_inode(&mut self, inode_num: u64) -> io::Result<()> {
        self.imap.clear(inode_num as usize)
    }

    fn allocate_block(&mut self) -> io::Result<u64> {
        match self.zmap.take_next() {
            Some(t) => Ok(self.superblock.first_data_zone as u64 + t as u64),
            None => {
                Err(io::Error::new(
                    io::ErrorKind::OutOfMemory,
                    "no blocks left to allocate",
                ))
            }
        }
    }

    fn deallocate_block(&mut self, block_num: u64) -> io::Result<()> {
        self.zmap
            .clear(block_num as usize - self.superblock.first_data_zone as usize)
    }
}
