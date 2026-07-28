//! Minix 3 Filesystem Inode Implementation
//!
//! © Stephen Marz
//! 8 June 2026
use super::super::consts::*;
use super::Inode;
use crate::{
    filetype::FileType,
    fs::InodeOperations,
    stat::{self, S_IFDIR, S_IFMT},
};
use std::slice::from_raw_parts;

impl Inode {
    pub fn is_dir(&self) -> bool {
        self.mode & S_IFMT == S_IFDIR
    }

    pub fn is_symlink(&self) -> bool {
        self.mode & S_IFMT == stat::S_IFLNK
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { from_raw_parts((self as *const Inode) as *const u8, INODE_BYTES as usize) }
    }
}

impl InodeOperations for Inode {
    fn get_file_type(&self) -> FileType {
        (self.mode & S_IFMT).into()
    }

    fn set_file_type(&mut self, ft: FileType) {
        let old = self.mode & 0o777;
        let t = match ft {
            FileType::BlockDevice => stat::S_IFBLK,
            FileType::CharacterDevice => stat::S_IFCHR,
            FileType::Directory => stat::S_IFDIR,
            FileType::Fifo => stat::S_IFIFO,
            FileType::Regular => stat::S_IFREG,
            FileType::Socket => stat::S_IFSOCK,
            FileType::Symlink => stat::S_IFLNK,
            FileType::Invalid => 0,
        };
        self.mode = old | t;
    }

    fn get_mode(&self) -> u16 {
        self.mode & 0o777
    }

    fn set_mode(&mut self, mode: u16) {
        let t = self.mode & S_IFMT;
        let mode = mode & 0o777;
        self.mode = t | mode;
    }

    fn get_atime(&self) -> u64 {
        self.atime as u64
    }

    fn set_atime(&mut self, atime: u64) {
        self.atime = atime as u32;
    }

    fn get_mtime(&self) -> u64 {
        self.mtime as u64
    }

    fn set_mtime(&mut self, mtime: u64) {
        self.mtime = mtime as u32;
    }

    fn get_ctime(&self) -> u64 {
        self.ctime as u64
    }

    fn set_ctime(&mut self, ctime: u64) {
        self.ctime = ctime as u32;
    }

    fn get_uid(&self) -> u32 {
        self.uid as u32
    }

    fn set_uid(&mut self, uid: u32) {
        self.uid = uid as u16;
    }

    fn get_gid(&self) -> u32 {
        self.gid as u32
    }

    fn set_gid(&mut self, gid: u32) {
        self.gid = gid as u16;
    }

    fn get_nlinks(&self) -> u32 {
        self.nlinks as u32
    }

    fn set_nlinks(&mut self, nlinks: u32) {
        self.nlinks = nlinks as u16;
    }

    fn get_size(&self) -> u64 {
        self.size as u64
    }

    fn set_size(&mut self, size: u64) {
        self.size = size as u32;
    }

    fn get_blocks(&self) -> Vec<u64> {
        self.zones.iter().map(|&zone| zone as u64).collect()
    }

    fn set_blocks(&mut self, blocks: &[u64]) {
        assert!(blocks.len() <= 13);

        for (i, &b) in blocks.iter().enumerate() {
            self.zones[i] = b as u32;
        }
    }

    fn set_node(&mut self, major: u16, minor: u16) {
        // Only block and character devices have nodes. If this is not one of those, do nothing.
        // We could signal a panic, but this might be what we want, so do nothing instead.
        if self.get_file_type() != FileType::BlockDevice
            && self.get_file_type() != FileType::CharacterDevice
        {
            return;
        }
        let mut blocks = self.get_blocks();
        let major = major & 0xFF;
        let minor = minor & 0xFF;
        blocks[0] = ((major << 8) | minor) as u64;
        self.set_blocks(&blocks);
    }

    fn get_node(&self) -> (u16, u16) {
        let blocks = self.get_blocks();
        let node = blocks[0] as u32;
        let major = (node >> 8) as u16;
        let minor = (node & 0xFF) as u16;
        (major, minor)
    }
}
