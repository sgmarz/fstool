//! mzFAT Virtual Inode
//!
//! © Stephen Marz
//! 8 June 2026
use super::consts;
use crate::{
    fs::{FileType, InodeOperations},
    stat,
};

#[repr(C, packed)]
#[derive(Debug, Default)]
pub struct Entry {
    pub mode: u16,
    pub uid: u16,
    pub gid: u16,
    pub fat: u32,
    pub ctime: u32,
    pub atime: u32,
    pub mtime: u32,
    pub size: u64,
    pub name: [u8; consts::DIR_ENTRY_SIZE],
}

impl InodeOperations for Entry {
    fn get_blocks(&self) -> Vec<u64> {
        todo!()
    }

    fn get_file_type(&self) -> FileType {
        match stat::S_IFMT & self.mode {
            stat::S_IFDIR => FileType::Directory,
            stat::S_IFREG => FileType::Regular,
            stat::S_IFBLK => FileType::BlockDevice,
            stat::S_IFCHR => FileType::CharacterDevice,
            _ => FileType::Invalid,
        }
    }

    fn get_mode(&self) -> u16 {
        self.mode & 0o777
    }

    fn get_atime(&self) -> u64 {
        self.atime as u64
    }

    fn get_ctime(&self) -> u64 {
        self.ctime as u64
    }

    fn get_mtime(&self) -> u64 {
        self.mtime as u64
    }

    fn get_nlinks(&self) -> u32 {
        1_u32
    }

    fn get_node(&self) -> (u16, u16) {
        (0, 0)
    }

    fn get_size(&self) -> u64 {
        self.size
    }

    fn get_uid(&self) -> u32 {
        self.uid as u32
    }

    fn get_gid(&self) -> u32 {
        self.gid as u32
    }

    fn set_atime(&mut self, atime: u64) {
        self.atime = atime as u32;
    }

    fn set_ctime(&mut self, ctime: u64) {
        self.ctime = ctime as u32;
    }

    fn set_mtime(&mut self, mtime: u64) {
        self.mtime = mtime as u32;
    }

    fn set_blocks(&mut self, blocks: &[u64]) {
        todo!();
    }

    fn set_file_type(&mut self, ft: FileType) {
        let upper_mode = match ft {
            FileType::Directory => stat::S_IFDIR,
            FileType::BlockDevice => stat::S_IFBLK,
            FileType::CharacterDevice => stat::S_IFCHR,
            _ => stat::S_IFREG,
        };
        let old_mode = self.mode & 0o777;
        self.mode = old_mode | upper_mode;
    }

    fn set_uid(&mut self, uid: u32) {
        self.uid = uid as u16;
    }

    fn set_gid(&mut self, gid: u32) {
        self.gid = gid as u16;
    }

    fn set_mode(&mut self, mode: u16) {
        self.mode = (self.mode & !0o777) | (mode & 0o777);
    }

    fn set_nlinks(&mut self, _nlinks: u32) {
        // NO-OP
    }

    fn set_node(&mut self, _major: u16, _minor: u16) {}

    fn set_size(&mut self, size: u64) {
        self.size = size;
    }
}
