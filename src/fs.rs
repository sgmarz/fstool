//! File System Traits
//!
//! © Stephen Marz
//! 8 June 2026
use crate::cache::Tree;
pub use crate::filetype::FileType;
use std::{fs, io};

#[derive(Debug)]
pub struct AllocationData {
    pub taken: u64,
    pub free: u64,
}
impl AllocationData {
    pub const fn new(taken: u64, free: u64) -> Self {
        Self { taken, free }
    }

    pub fn total(&self) -> u64 {
        self.taken + self.free
    }
}

pub trait MakeFileSystem {
    fn mkfs(stream: &mut fs::File, size: u64) -> io::Result<()>;
}

pub trait SuperblockOperations {
    fn get_block_size(&self) -> u64;
    fn get_num_blocks(&self) -> AllocationData;
    fn get_num_inodes(&self) -> AllocationData;
    fn allocate_inode(&mut self) -> io::Result<u64>;
    fn deallocate_inode(&mut self, inode_num: u64) -> io::Result<()>;
    fn allocate_block(&mut self) -> io::Result<u64>;
    fn deallocate_block(&mut self, block_num: u64) -> io::Result<()>;
}

pub trait InodeOperations {
    fn get_file_type(&self) -> FileType;
    fn set_file_type(&mut self, ft: FileType);
    fn get_mode(&self) -> u16;
    fn set_mode(&mut self, mode: u16);
    fn get_atime(&self) -> u64;
    fn set_atime(&mut self, atime: u64);
    fn get_mtime(&self) -> u64;
    fn set_mtime(&mut self, mtime: u64);
    fn get_ctime(&self) -> u64;
    fn set_ctime(&mut self, ctime: u64);
    fn get_uid(&self) -> u32;
    fn set_uid(&mut self, uid: u32);
    fn get_gid(&self) -> u32;
    fn set_gid(&mut self, gid: u32);
    fn get_nlinks(&self) -> u32;
    fn set_nlinks(&mut self, nlinks: u32);
    fn get_size(&self) -> u64;
    fn set_size(&mut self, size: u64);
    fn get_blocks(&self) -> Vec<u64>;
    fn set_blocks(&mut self, blocks: &[u64]);
    fn get_node(&self) -> (u16, u16);
    fn set_node(&mut self, major: u16, minor: u16);
}

pub trait FileSystem {
    fn name(&self) -> &str;
    fn read_block(&mut self, block_num: u64, data: &mut [u8]) -> io::Result<()>;
    fn write_block(&mut self, block_num: u64, data: &[u8]) -> io::Result<()>;
    fn get_inode(&mut self, inode_num: u64) -> io::Result<&dyn InodeOperations>;
    fn get_inode_mut(&mut self, inode_num: u64) -> io::Result<&mut dyn InodeOperations>;

    fn create(&mut self, abs_path: &String, ftype: FileType) -> io::Result<u64>;
    fn link(&mut self, parent_inode: u64, abs_path: &String) -> io::Result<u64>;

    fn read_symlink(&mut self, inode: u64) -> io::Result<String>;
    fn write_symlink(&mut self, inode: u64, target: &String) -> io::Result<u64>;
    fn read_file(&mut self, inode: u64, offset: u64, buffer: &mut [u8]) -> io::Result<u64>;
    fn write_file(&mut self, inode: u64, offset: u64, buffer: &[u8]) -> io::Result<u64>;
    fn truncate(&mut self, inode: u64, size: u64) -> io::Result<u64>;
    fn unlink(&mut self, abs_path: &String) -> io::Result<()>;

    fn write_to_backing(&mut self) -> io::Result<()>;

    fn get_superblock(&self) -> &dyn SuperblockOperations;
    fn get_superblock_mut(&mut self) -> &mut dyn SuperblockOperations;
    fn get_tree(&self) -> Option<&Tree>;
    fn get_tree_mut(&mut self) -> Option<&mut Tree>;
}
