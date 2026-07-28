//! Minix 3 File System
//!
//! © Stephen Marz
//! 8 June 2026
pub use super::Bitmap;
pub use super::DirEntry;
pub use super::Inode;
pub use super::Superblock;
use crate::cache::{BlockCache, Tree};
use std::{
    fs::File,
    io::{self, Seek, SeekFrom, Write},
};
pub use support::check_valid;

pub mod build;
pub mod create;
pub mod implfs;
pub mod support;

pub struct MinixFileSystem {
    pub superblock: Superblock,
    pub imap: Bitmap,
    pub zmap: Bitmap,
    pub inodes: Vec<Inode>,
    pub tree: Tree,
    pub stream: File,
    pub bcache: BlockCache,
}

impl MinixFileSystem {
    pub fn write_back_dirty_blocks(&mut self) -> io::Result<()> {
        self.bcache.for_each_dirty(|(&block_num, cb)| {
            let offset = block_num * self.superblock.block_size as u64;
            let _ = self.stream.seek(SeekFrom::Start(offset));
            let _ = self.stream.write_all(&cb.data);
        });
        self.bcache.clear();
        Ok(())
    }
}
