//! MzFAT File System.
//!
//!
//! © Stephen Marz
//! 8 June 2026

use crate::{
    bitmap::Bitmap,
    cache::{BlockCache, Tree},
    fs::{FileSystem, InodeOperations, SuperblockOperations},
};
use std::{
    fs::File,
    io::{self, SeekFrom},
};
use superblock::Superblock;

mod consts;
mod entry;
mod mkfs;
mod superblock;

pub struct MzfatFileSystem {
    pub sb: Superblock, // Cached Main Boot Sector (MBS)
    pub fat: Vec<u32>,  // Cached mzFAT file allocation table
    pub bitmap: Bitmap,
    pub tree: Tree,
    pub stream: File,
    pub bcache: BlockCache,
}

impl FileSystem for MzfatFileSystem {
    fn create(&mut self, abs_path: &String, ftype: crate::fs::FileType) -> io::Result<u64> {
        todo!();
    }

    fn get_inode(&mut self, inode_num: u64) -> io::Result<&dyn InodeOperations> {
        todo!();
    }

    fn get_inode_mut(&mut self, inode_num: u64) -> io::Result<&mut dyn crate::fs::InodeOperations> {
        todo!();
    }

    fn get_superblock(&self) -> &dyn SuperblockOperations {
        &self.sb
    }

    fn get_superblock_mut(&mut self) -> &mut dyn SuperblockOperations {
        &mut self.sb
    }

    fn get_tree(&self) -> Option<&Tree> {
        todo!();
    }

    fn get_tree_mut(&mut self) -> Option<&mut Tree> {
        todo!();
    }

    fn link(&mut self, _parent_inode: u64, _abs_path: &String) -> io::Result<u64> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "links are not supported",
        ))
    }

    fn name(&self) -> &str {
        "MzFAT"
    }

    fn read_block(&mut self, block_num: u64, data: &mut [u8]) -> io::Result<()> {
        todo!();
    }

    fn read_file(&mut self, inode: u64, offset: u64, buffer: &mut [u8]) -> io::Result<u64> {
        todo!();
    }

    fn read_symlink(&mut self, inode: u64) -> io::Result<String> {
        todo!();
    }

    fn truncate(&mut self, inode: u64, size: u64) -> io::Result<u64> {
        todo!();
    }

    fn unlink(&mut self, abs_path: &String) -> io::Result<()> {
        todo!();
    }

    fn write_block(&mut self, block_num: u64, data: &[u8]) -> io::Result<()> {
        todo!();
    }

    fn write_file(&mut self, inode: u64, offset: u64, buffer: &[u8]) -> io::Result<u64> {
        todo!();
    }

    fn write_symlink(&mut self, inode: u64, target: &String) -> io::Result<u64> {
        todo!();
    }

    fn write_to_backing(&mut self) -> io::Result<()> {
        todo!();
    }
}

pub fn check_valid<'a, U: io::Read + io::Seek>(stream: &'a mut U) -> io::Result<bool> {
    let mut superblock_bytes = [0_u8; consts::SUPERBLOCK_SIZE];
    stream.seek(SeekFrom::Start(0))?;
    stream.read(&mut superblock_bytes)?;
    let sb = Superblock::from_bytes(&superblock_bytes);
    Ok(sb.magic == consts::MZFAT_MAGIC)
}
